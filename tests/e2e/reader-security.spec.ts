import { readFile } from "node:fs/promises";

import { expect, type Page, test } from "@playwright/test";

const TOKEN = "a".repeat(32);
const WRONG_TOKEN = "b".repeat(32);
const SESSION_ID = "123e4567-e89b-42d3-a456-426614174000";
const PARENT_HOST = "qurner.test";
const IFRAME_HOST = `${TOKEN}.test`;
const IFRAME_ORIGIN = `http://${IFRAME_HOST}`;
const NONCE = "0123456789abcdef0123456789abcdef";

const CSP = [
  "default-src 'self' data: blob: epub:",
  `script-src 'nonce-${NONCE}'`,
  "style-src 'self' 'unsafe-inline' epub:",
  "img-src 'self' data: blob: epub:",
  "font-src 'self' data: epub:",
  "connect-src 'none'",
  "frame-src 'none'",
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
].join("; ");

async function engineSource(): Promise<string> {
  return readFile("src-tauri/resources/reader-engine.js", "utf8");
}

function parseUrl(raw: string): { hostname: string; pathname: string } {
  const rest = raw.replace(/^[a-z]+:\/\//, "");
  const slash = rest.indexOf("/");
  const host = slash === -1 ? rest : rest.slice(0, slash);
  const path = slash === -1 ? "/" : rest.slice(slash);
  return { hostname: host, pathname: path.split("?")[0] ?? "/" };
}

function parentHtml(): string {
  return `<!doctype html>
<html>
  <head><meta charset="utf-8"><title>qurner e2e</title></head>
  <body>
    <iframe
      id="reader"
      sandbox="allow-scripts"
      src="${IFRAME_ORIGIN}/reader.html"
      style="width: 600px; height: 400px"
      onload="window.__readerLoaded = true"
    ></iframe>
    <script>
      window.__messages = [];
      window.addEventListener("message", (event) => {
        const frame = document.getElementById("reader");
        if (!frame || event.source !== frame.contentWindow) return;
        window.__messages.push({ origin: event.origin, data: event.data });
      });
      window.sendReaderMessage = (data, origin) => {
        const frame = document.getElementById("reader");
        frame.contentWindow.postMessage(data, origin);
      };
    </script>
  </body>
</html>`;
}

function readerHtml(body: string): string {
  return `<!doctype html>
<html>
  <head><meta charset="utf-8"><title>reader</title></head>
  <body>
    ${body}
    <script nonce="${NONCE}" src="${IFRAME_ORIGIN}/reader-engine.js" data-session-id="${SESSION_ID}" data-spine-index="0" data-spine-href="chapter1.xhtml"></script>
  </body>
</html>`;
}

function validInit(token = TOKEN) {
  return {
    protocolVersion: 1,
    sessionId: SESSION_ID,
    sessionToken: token,
    type: "reader:init",
    payload: { mode: "paged", initialLocator: null },
  };
}

async function setupPage(page: Page, body: string, engine: string): Promise<string[]> {
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  await page.route("**/*", async (route) => {
    const url = parseUrl(route.request().url());
    if (url.hostname === PARENT_HOST && url.pathname === "/parent.html") {
      await route.fulfill({
        status: 200,
        headers: { "Content-Type": "text/html; charset=utf-8" },
        body: parentHtml(),
      });
      return;
    }
    if (url.hostname === IFRAME_HOST && url.pathname === "/reader.html") {
      await route.fulfill({
        status: 200,
        headers: {
          "Content-Type": "text/html; charset=utf-8",
          "Content-Security-Policy": CSP,
          "X-Content-Type-Options": "nosniff",
          "Cache-Control": "no-store",
        },
        body: readerHtml(body),
      });
      return;
    }
    if (url.hostname === IFRAME_HOST && url.pathname === "/reader-engine.js") {
      await route.fulfill({
        status: 200,
        headers: {
          "Content-Type": "text/javascript; charset=utf-8",
          "X-Content-Type-Options": "nosniff",
          "Cache-Control": "no-store",
        },
        body: engine,
      });
      return;
    }
    await route.fulfill({ status: 404, body: "not found" });
  });

  await page.goto(`http://${PARENT_HOST}/parent.html`);
  await page.waitForSelector("iframe#reader");
  await page.waitForFunction(
    () => (globalThis as unknown as { __readerLoaded?: boolean }).__readerLoaded === true,
    undefined,
    { timeout: 10_000 },
  );
  return requests;
}

async function initializeReader(page: Page, token = TOKEN): Promise<void> {
  await page.evaluate(
    ({ message, origin }) => {
      (
        globalThis as unknown as {
          sendReaderMessage: (data: unknown, target: string) => void;
        }
      ).sendReaderMessage(message, origin);
    },
    { message: validInit(token), origin: "*" },
  );
}

async function waitForReady(page: Page): Promise<void> {
  await page.waitForFunction(
    () =>
      (
        globalThis as unknown as {
          __messages: Array<{ data?: { type?: string } }>;
        }
      ).__messages.some((entry) => entry.data?.type === "reader:ready"),
    undefined,
    { timeout: 10_000 },
  );
}

async function readerMessages(page: Page): Promise<Array<{ data?: { type?: string } }>> {
  return page.evaluate(
    () =>
      (
        globalThis as unknown as {
          __messages: Array<{ data?: { type?: string } }>;
        }
      ).__messages,
  );
}

test("trusted engine completes the handshake", async ({ page }) => {
  const engine = await engineSource();
  await setupPage(page, "<p>正文</p>", engine);
  await initializeReader(page);
  await waitForReady(page);

  const messages = await readerMessages(page);
  const ready = messages.filter((entry) => entry.data?.type === "reader:ready");
  expect(ready).toHaveLength(1);
  const data = ready[0]?.data as {
    sessionId: string;
    payload: { engineVersion: string };
  };
  expect(data.sessionId).toBe(SESSION_ID);
  expect(data.payload).toEqual({ engineVersion: "0.2.0" });
});

test("obfuscated links cannot navigate or execute", async ({ page }) => {
  const engine = await engineSource();
  const body = [
    '<a id="tab-top" href="h\tttp://evil.test/tab" target="_top">tab</a>',
    '<a id="lf-js" href="java\nscript:window.pwned = 1">lf</a>',
  ].join("\n");
  const requests = await setupPage(page, body, engine);
  await initializeReader(page);
  await waitForReady(page);

  const parentUrl = page.url();
  const frame = page.frame({ url: `${IFRAME_ORIGIN}/reader.html` });
  expect(frame).not.toBeNull();
  await frame?.click("#tab-top");
  await frame?.click("#lf-js");
  await page.waitForTimeout(300);

  expect(page.url()).toBe(parentUrl);
  const pwned = await frame?.evaluate(() => (globalThis as unknown as { pwned?: unknown }).pwned);
  expect(pwned).toBeUndefined();
  expect(requests.filter((url) => url.includes("evil.test"))).toEqual([]);
});

test("blocks inline and remote scripts, images and styles", async ({ page }) => {
  const engine = await engineSource();
  const hostile = [
    "<p>正文</p>",
    "<script>window.pwned = 1;</script>",
    '<script src="http://evil.test/pwn.js"></script>',
    '<img src="http://evil.test/pwn.png" alt="x">',
    '<link rel="stylesheet" href="http://evil.test/pwn.css">',
  ].join("\n");
  const requests = await setupPage(page, hostile, engine);
  await initializeReader(page);
  await waitForReady(page);

  const frame = page.frame({ url: `${IFRAME_ORIGIN}/reader.html` });
  expect(frame).not.toBeNull();
  const pwned = await frame?.evaluate(() => (globalThis as unknown as { pwned?: unknown }).pwned);
  expect(pwned).toBeUndefined();
  expect(requests.filter((url) => url.includes("evil.test"))).toEqual([]);
});

test("external links and forms do not navigate", async ({ page }) => {
  const engine = await engineSource();
  const body = [
    '<a id="external" data-qurner-external="blocked">外部链接</a>',
    '<form action="http://evil.test/submit" method="post">',
    '<button id="submit" type="submit">提交</button>',
    "</form>",
  ].join("\n");
  const requests = await setupPage(page, body, engine);
  await initializeReader(page);
  await waitForReady(page);

  const parentUrl = page.url();
  const frame = page.frame({ url: `${IFRAME_ORIGIN}/reader.html` });
  expect(frame).not.toBeNull();
  await frame?.click("#external");
  await frame?.click("#submit");
  await page.waitForTimeout(300);
  expect(page.url()).toBe(parentUrl);
  expect(requests.filter((url) => url.includes("evil.test"))).toEqual([]);
});

test("forged messages are ignored and do not break the handshake", async ({ page }) => {
  const engine = await engineSource();
  await setupPage(page, "<p>正文</p>", engine);

  await initializeReader(page, WRONG_TOKEN);
  await page.waitForTimeout(500);
  const forged = await readerMessages(page);
  expect(forged.some((entry) => entry.data?.type === "reader:ready")).toBe(false);

  await initializeReader(page);
  await waitForReady(page);
  const messages = await readerMessages(page);
  expect(messages.filter((entry) => entry.data?.type === "reader:ready")).toHaveLength(1);
});
