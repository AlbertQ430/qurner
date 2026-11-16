import { readFile } from "node:fs/promises";

import { expect, type Page, test } from "@playwright/test";

const TOKEN = "a".repeat(32);
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

test.use({ viewport: { width: 900, height: 700 } });

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

function longBody(): string {
  return Array.from(
    { length: 60 },
    (_, index) =>
      `<p>第 ${index} 段：这是分页测试段落，包含中文和 English 以及 emoji 😀。${"继续填充正文内容，用于产生多栏分页。".repeat(3)}</p>`,
  ).join("");
}

function parentHtml(): string {
  return `<!doctype html>
<html>
  <head><meta charset="utf-8"><title>qurner pagination</title></head>
  <body style="margin: 0">
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

function readerHtml(body: string, spineIndex = 0, spineHref = "chapter1.xhtml"): string {
  return `<!doctype html>
<html>
  <head><meta charset="utf-8"><title>reader</title></head>
  <body>
    ${body}
    <script nonce="${NONCE}" src="${IFRAME_ORIGIN}/reader-engine.js" data-session-id="${SESSION_ID}" data-spine-index="${spineIndex}" data-spine-href="${spineHref}"></script>
  </body>
</html>`;
}

function baseMessage(): Record<string, unknown> {
  return {
    protocolVersion: 1,
    sessionId: SESSION_ID,
    sessionToken: TOKEN,
  };
}

function initMessage(
  mode: "paged" | "scrolling",
  initialLocator: unknown = null,
  spineIndex = 0,
): Record<string, unknown> {
  return {
    ...baseMessage(),
    type: "reader:init",
    payload: { spineIndex, settings: { mode, fontScale: 1, theme: "day" }, initialLocator },
  };
}

function command(type: string, payload: Record<string, unknown>): Record<string, unknown> {
  return { ...baseMessage(), type, payload };
}

function locator(overrides: Record<string, unknown> = {}) {
  return {
    version: 1,
    spineIndex: 0,
    spineHref: "chapter1.xhtml",
    textOffset: 0,
    intraSpineProgression: 0,
    quote: { exact: "", prefix: "", suffix: "" },
    ...overrides,
  };
}

async function setupPage(
  page: Page,
  body: string,
  engine: string,
  spineIndex = 0,
  spineHref = "chapter1.xhtml",
): Promise<string[]> {
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
        body: readerHtml(body, spineIndex, spineHref),
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

async function send(page: Page, message: unknown): Promise<void> {
  await page.evaluate((data) => {
    (
      globalThis as unknown as {
        sendReaderMessage: (data: unknown, origin: string) => void;
      }
    ).sendReaderMessage(data, "*");
  }, message);
}

async function messages(page: Page, type: string): Promise<Array<Record<string, unknown>>> {
  return page.evaluate((kind) => {
    const entries = (
      globalThis as unknown as {
        __messages: Array<{ data?: Record<string, unknown> }>;
      }
    ).__messages;
    return entries
      .map((entry) => entry.data)
      .filter((data): data is Record<string, unknown> => data?.type === kind);
  }, type);
}

async function waitForCount(page: Page, type: string, count: number): Promise<void> {
  await page.waitForFunction(
    ({ kind, expected }) => {
      const entries = (
        globalThis as unknown as {
          __messages: Array<{ data?: { type?: string } }>;
        }
      ).__messages;
      return entries.filter((entry) => entry.data?.type === kind).length >= expected;
    },
    { kind: type, expected: count },
    { timeout: 10_000 },
  );
}

async function waitForReason(page: Page, reason: string, count: number): Promise<void> {
  await page.waitForFunction(
    ({ expectedReason, expected }) => {
      const entries = (
        globalThis as unknown as {
          __messages: Array<{
            data?: { type?: string; payload?: { reason?: string } };
          }>;
        }
      ).__messages;
      return (
        entries.filter(
          (entry) =>
            entry.data?.type === "reader:location-changed" &&
            entry.data.payload?.reason === expectedReason,
        ).length >= expected
      );
    },
    { expectedReason: reason, expected: count },
    { timeout: 10_000 },
  );
}

async function lastMessage(page: Page, type: string): Promise<Record<string, unknown>> {
  const list = await messages(page, type);
  const last = list[list.length - 1];
  expect(last).toBeDefined();
  return (last ?? {}) as Record<string, unknown>;
}

async function lastReason(page: Page, reason: string): Promise<Record<string, unknown>> {
  const list = (await messages(page, "reader:location-changed")).filter(
    (message) => (message.payload as { reason?: string }).reason === reason,
  );
  const last = list[list.length - 1];
  expect(last).toBeDefined();
  return (last?.payload ?? {}) as Record<string, unknown>;
}

async function readerFrame(page: Page) {
  const frame = page.frame({ url: `${IFRAME_ORIGIN}/reader.html` });
  expect(frame).not.toBeNull();
  if (!frame) {
    throw new Error("missing reader frame");
  }
  return frame;
}

test("paged mode paginates, advances and clamps at boundaries", async ({ page }) => {
  const engine = await engineSource();
  await setupPage(page, longBody(), engine);
  await send(page, initMessage("paged"));
  await waitForCount(page, "reader:ready", 1);
  await waitForReason(page, "init", 1);
  const initPayload = (await lastReason(page, "init")) as {
    locator: { textOffset: number };
    mode: string;
    pageIndex: number;
    pageCount: number;
  };
  expect(initPayload.mode).toBe("paged");
  expect(initPayload.pageCount).toBeGreaterThan(1);
  expect(initPayload.pageIndex).toBe(0);

  await send(page, command("reader:capture-locator", { captureRequestId: 1 }));
  await waitForReason(page, "capture", 1);
  const pageZero = (await lastReason(page, "capture")) as {
    locator: { textOffset: number };
    captureRequestId: number;
  };
  expect(pageZero.captureRequestId).toBe(1);

  await send(page, command("reader:turn-page", { direction: "next" }));
  await waitForCount(page, "reader:page-changed", 1);
  const first = (await lastMessage(page, "reader:page-changed")).payload as {
    pageIndex: number;
    pageCount: number;
  };
  expect(first.pageIndex).toBe(1);
  expect(first.pageCount).toBe(initPayload.pageCount);

  await send(page, command("reader:capture-locator", { captureRequestId: 2 }));
  await waitForReason(page, "capture", 2);
  const pageOne = (await lastReason(page, "capture")) as {
    locator: { textOffset: number };
    captureRequestId: number;
  };
  expect(pageOne.captureRequestId).toBe(2);
  expect(pageOne.locator.textOffset).toBeGreaterThan(pageZero.locator.textOffset);

  await send(page, command("reader:turn-page", { direction: "prev" }));
  await waitForCount(page, "reader:page-changed", 2);
  const back = (await lastMessage(page, "reader:page-changed")).payload as {
    pageIndex: number;
  };
  expect(back.pageIndex).toBe(0);

  await send(page, command("reader:turn-page", { direction: "prev" }));
  await waitForCount(page, "reader:spine-request", 1);
  const spine = (await lastMessage(page, "reader:spine-request")).payload as {
    direction: string;
    locator: { spineIndex: number; spineHref: string };
  };
  expect(spine.direction).toBe("prev");
  expect(spine.locator.spineIndex).toBe(0);
  expect(spine.locator.spineHref).toBe("chapter1.xhtml");
  await send(page, command("reader:turn-page", { direction: "prev" }));
  await page.waitForTimeout(200);
  expect((await messages(page, "reader:spine-request")).length).toBe(1);

  // 翻到末页，验证 clamp、无空白尾页与 spine-request(next) 去重。
  let changedCount = 2;
  for (let index = 1; index < initPayload.pageCount; index += 1) {
    await send(page, command("reader:turn-page", { direction: "next" }));
    changedCount += 1;
    await waitForCount(page, "reader:page-changed", changedCount);
  }
  const lastPage = (await lastMessage(page, "reader:page-changed")).payload as {
    pageIndex: number;
    pageCount: number;
  };
  expect(lastPage.pageIndex).toBe(initPayload.pageCount - 1);

  const geometry = await (await readerFrame(page)).evaluate(() => {
    const doc = (
      globalThis as unknown as {
        document: {
          documentElement: { scrollWidth: number; clientWidth: number };
        };
      }
    ).document;
    return {
      scrollWidth: doc.documentElement.scrollWidth,
      clientWidth: doc.documentElement.clientWidth,
    };
  });
  const step = geometry.clientWidth + 32;
  expect(geometry.scrollWidth - (initPayload.pageCount - 1) * step).toBeGreaterThan(0);

  await send(page, command("reader:turn-page", { direction: "next" }));
  await waitForCount(page, "reader:spine-request", 2);
  const nextSpine = (await lastMessage(page, "reader:spine-request")).payload as {
    direction: string;
  };
  expect(nextSpine.direction).toBe("next");
  await send(page, command("reader:turn-page", { direction: "next" }));
  await page.waitForTimeout(200);
  expect((await messages(page, "reader:spine-request")).length).toBe(2);
});

test("scrolling mode turns about one viewport and dedupes boundaries", async ({ page }) => {
  const engine = await engineSource();
  await setupPage(page, longBody(), engine);
  await send(page, initMessage("scrolling"));
  await waitForCount(page, "reader:ready", 1);
  await waitForReason(page, "init", 1);
  const initPayload = (await lastReason(page, "init")) as {
    mode: string;
    pageIndex: number;
    pageCount: number;
  };
  expect(initPayload.mode).toBe("scrolling");
  expect(initPayload.pageIndex).toBe(0);
  expect(initPayload.pageCount).toBe(0);

  const frame = await readerFrame(page);
  const readScroll = () =>
    frame.evaluate(() => {
      const doc = (
        globalThis as unknown as {
          document: {
            documentElement: { scrollTop: number; clientHeight: number };
          };
        }
      ).document;
      return {
        top: doc.documentElement.scrollTop,
        height: doc.documentElement.clientHeight,
      };
    });
  await page.waitForTimeout(300);
  const before = await readScroll();
  await send(page, command("reader:capture-locator", { captureRequestId: 1 }));
  await waitForReason(page, "capture", 1);
  const beforeLocator = (await lastReason(page, "capture")) as {
    locator: { textOffset: number };
    captureRequestId: number;
  };
  expect(beforeLocator.captureRequestId).toBe(1);
  await send(page, command("reader:turn-page", { direction: "next" }));
  await waitForReason(page, "turn", 1);
  const after = await readScroll();
  expect(after.top - before.top).toBeGreaterThan(0);
  expect(after.top - before.top).toBeLessThanOrEqual(after.height * 1.1);
  const turnPayload = (await lastReason(page, "turn")) as {
    locator: { textOffset: number };
  };
  expect(turnPayload.locator.textOffset).toBeGreaterThan(beforeLocator.locator.textOffset);

  await frame.evaluate(() => {
    const doc = (
      globalThis as unknown as {
        document: { documentElement: { scrollTop: number } };
      }
    ).document;
    doc.documentElement.scrollTop = 0;
  });
  await send(page, command("reader:turn-page", { direction: "prev" }));
  await waitForCount(page, "reader:spine-request", 1);
  const spine = (await lastMessage(page, "reader:spine-request")).payload as {
    direction: string;
  };
  expect(spine.direction).toBe("prev");
  await send(page, command("reader:turn-page", { direction: "prev" }));
  await page.waitForTimeout(200);
  expect((await messages(page, "reader:spine-request")).length).toBe(1);

  await frame.evaluate(() => {
    const doc = (
      globalThis as unknown as {
        document: {
          documentElement: { scrollTop: number; scrollHeight: number };
        };
      }
    ).document;
    doc.documentElement.scrollTop = doc.documentElement.scrollHeight;
  });
  await page.waitForTimeout(200);
  await send(page, command("reader:turn-page", { direction: "next" }));
  await waitForCount(page, "reader:spine-request", 2);
  const bottomSpine = (await lastMessage(page, "reader:spine-request")).payload as {
    direction: string;
  };
  expect(bottomSpine.direction).toBe("next");
});

test("resize keeps the same locator quote", async ({ page }) => {
  const engine = await engineSource();
  await setupPage(page, longBody(), engine);
  await send(page, initMessage("paged"));
  await waitForCount(page, "reader:ready", 1);
  await waitForReason(page, "init", 1);
  await send(page, command("reader:turn-page", { direction: "next" }));
  await waitForCount(page, "reader:page-changed", 1);
  await send(page, command("reader:capture-locator", { captureRequestId: 1 }));
  await waitForReason(page, "capture", 1);
  const before = (await lastReason(page, "capture")) as {
    locator: { textOffset: number; quote: { exact: string } };
    captureRequestId: number;
  };
  expect(before.captureRequestId).toBe(1);

  await page.evaluate(() => {
    const doc = (
      globalThis as unknown as {
        document: {
          getElementById: (id: string) => { style: { width: string; height: string } } | null;
        };
      }
    ).document;
    const frame = doc.getElementById("reader");
    if (frame) {
      frame.style.width = "500px";
      frame.style.height = "500px";
    }
  });
  const resizedFrame = await readerFrame(page);
  await resizedFrame.evaluate(() => {
    const host = globalThis as unknown as {
      Event: new (type: string) => unknown;
      dispatchEvent: (event: unknown) => void;
    };
    host.dispatchEvent(new host.Event("resize"));
  });
  await page.waitForTimeout(400);

  await send(page, command("reader:capture-locator", { captureRequestId: 2 }));
  await waitForReason(page, "capture", 2);
  const after = (await lastReason(page, "capture")) as {
    locator: { textOffset: number; quote: { exact: string } };
    captureRequestId: number;
  };
  expect(after.captureRequestId).toBe(2);
  expect(after.locator.quote.exact).toBe(before.locator.quote.exact);
  expect(Math.abs(after.locator.textOffset - before.locator.textOffset)).toBeLessThanOrEqual(2);
});

test("delayed font-ready relayout keeps the locator", async ({ page }) => {
  const engine = await engineSource();
  await setupPage(page, longBody(), engine);
  const frame = await readerFrame(page);
  await frame.evaluate(() => {
    const host = globalThis as unknown as {
      document: { fonts: object };
      Promise: new (executor: (resolve: () => void) => void) => unknown;
      __resolveFontsReady?: () => void;
    };
    let resolveReady: (() => void) | null = null;
    const delayed = new host.Promise((resolve) => {
      resolveReady = resolve;
    });
    Object.defineProperty(host.document.fonts, "ready", {
      configurable: true,
      get: () => delayed,
    });
    host.__resolveFontsReady = () => {
      resolveReady?.();
    };
  });

  await send(page, initMessage("paged"));
  await waitForCount(page, "reader:ready", 1);
  await waitForReason(page, "init", 1);
  const initPayload = (await lastReason(page, "init")) as {
    locator: { textOffset: number };
  };
  expect(
    (await messages(page, "reader:location-changed")).filter(
      (message) => (message.payload as { reason?: string }).reason === "layout",
    ),
  ).toHaveLength(0);

  await frame.evaluate(() => {
    (globalThis as unknown as { __resolveFontsReady?: () => void }).__resolveFontsReady?.();
  });
  await waitForReason(page, "layout", 1);
  const layout = (await lastReason(page, "layout")) as {
    locator: { textOffset: number };
  };
  expect(Math.abs(layout.locator.textOffset - initPayload.locator.textOffset)).toBeLessThanOrEqual(
    2,
  );
});

test("initial locator restores the middle of the chapter", async ({ page }) => {
  const engine = await engineSource();
  await setupPage(page, longBody(), engine);
  await send(page, initMessage("paged"));
  await waitForCount(page, "reader:ready", 1);
  await waitForReason(page, "init", 1);
  await send(page, command("reader:turn-page", { direction: "next" }));
  await waitForCount(page, "reader:page-changed", 1);
  await send(page, command("reader:capture-locator", { captureRequestId: 1 }));
  await waitForReason(page, "capture", 1);
  const captured = (await lastReason(page, "capture")) as {
    locator: { textOffset: number; quote: { exact: string } };
    pageIndex: number;
    captureRequestId: number;
  };
  expect(captured.captureRequestId).toBe(1);
  expect(captured.pageIndex).toBe(1);

  await page.goto(`http://${PARENT_HOST}/parent.html`);
  await page.waitForSelector("iframe#reader");
  await page.waitForFunction(
    () => (globalThis as unknown as { __readerLoaded?: boolean }).__readerLoaded === true,
    undefined,
    { timeout: 10_000 },
  );
  await send(page, initMessage("paged", captured.locator));
  await waitForCount(page, "reader:ready", 1);
  await waitForReason(page, "init", 1);
  await send(page, command("reader:capture-locator", { captureRequestId: 1 }));
  await waitForReason(page, "capture", 1);
  const restored = (await lastReason(page, "capture")) as {
    locator: { textOffset: number; quote: { exact: string } };
    pageIndex: number;
    captureRequestId: number;
  };
  expect(restored.captureRequestId).toBe(1);
  expect(restored.locator.quote.exact).toBe(captured.locator.quote.exact);
  expect(Math.abs(restored.locator.textOffset - captured.locator.textOffset)).toBeLessThanOrEqual(
    2,
  );
  expect(restored.pageIndex).toBeGreaterThan(0);
});

test("mode switch preserves the locator and go-to jumps back", async ({ page }) => {
  const engine = await engineSource();
  await setupPage(page, longBody(), engine);
  await send(page, initMessage("paged"));
  await waitForCount(page, "reader:ready", 1);
  await waitForReason(page, "init", 1);
  await send(page, command("reader:turn-page", { direction: "next" }));
  await waitForCount(page, "reader:page-changed", 1);
  await send(page, command("reader:capture-locator", { captureRequestId: 1 }));
  await waitForReason(page, "capture", 1);
  const captured = (await lastReason(page, "capture")) as {
    locator: { textOffset: number; quote: { exact: string } };
    captureRequestId: number;
  };
  expect(captured.captureRequestId).toBe(1);

  await send(
    page,
    command("reader:update-settings", {
      settings: { mode: "scrolling", fontScale: 1, theme: "day" },
    }),
  );
  await page.waitForFunction(
    () => {
      const entries = (
        globalThis as unknown as {
          __messages: Array<{
            data?: {
              type?: string;
              payload?: { reason?: string; mode?: string };
            };
          }>;
        }
      ).__messages;
      return entries.some(
        (entry) =>
          entry.data?.type === "reader:location-changed" &&
          entry.data.payload?.reason === "layout" &&
          entry.data.payload?.mode === "scrolling",
      );
    },
    undefined,
    { timeout: 10_000 },
  );

  await send(page, command("reader:capture-locator", { captureRequestId: 2 }));
  await waitForReason(page, "capture", 2);
  const scrollingCapture = (await lastReason(page, "capture")) as {
    locator: { quote: { exact: string } };
    captureRequestId: number;
  };
  expect(scrollingCapture.captureRequestId).toBe(2);
  expect(scrollingCapture.locator.quote.exact).toBe(captured.locator.quote.exact);

  await send(
    page,
    command("reader:update-settings", { settings: { mode: "paged", fontScale: 1, theme: "day" } }),
  );
  await page.waitForFunction(
    () => {
      const entries = (
        globalThis as unknown as {
          __messages: Array<{
            data?: {
              type?: string;
              payload?: { reason?: string; mode?: string };
            };
          }>;
        }
      ).__messages;
      return (
        entries.filter(
          (entry) =>
            entry.data?.type === "reader:location-changed" &&
            entry.data.payload?.reason === "layout" &&
            entry.data.payload?.mode === "paged",
        ).length >= 1
      );
    },
    undefined,
    { timeout: 10_000 },
  );
  await send(page, command("reader:capture-locator", { captureRequestId: 3 }));
  await waitForReason(page, "capture", 3);
  const pagedCapture = (await lastReason(page, "capture")) as {
    locator: { quote: { exact: string } };
    captureRequestId: number;
  };
  expect(pagedCapture.captureRequestId).toBe(3);
  expect(pagedCapture.locator.quote.exact).toBe(captured.locator.quote.exact);

  const target = locator({
    textOffset: Math.max(0, captured.locator.textOffset - 200),
    intraSpineProgression: 0,
    quote: captured.locator.quote,
  });
  await send(page, command("reader:go-to", { locator: target }));
  await waitForReason(page, "go-to", 1);
});

test("missing and cross-spine locators report distinct errors", async ({ page }) => {
  const engine = await engineSource();
  await setupPage(page, longBody(), engine, 1, "chapter2.xhtml");
  await send(page, initMessage("paged", null, 1));
  await waitForCount(page, "reader:ready", 1);
  await waitForReason(page, "init", 1);
  const initPayload = (await lastReason(page, "init")) as {
    locator: { spineIndex: number; spineHref: string; textOffset: number };
  };
  expect(initPayload.locator.spineIndex).toBe(1);
  expect(initPayload.locator.spineHref).toBe("chapter2.xhtml");

  await send(page, command("reader:turn-page", { direction: "next" }));
  await waitForCount(page, "reader:page-changed", 1);

  const crossSpine = locator({
    spineIndex: 0,
    spineHref: "chapter1.xhtml",
    textOffset: 10,
    quote: { exact: "跨章节", prefix: "", suffix: "" },
  });
  await send(page, command("reader:go-to", { locator: crossSpine }));
  await waitForCount(page, "reader:error", 1);
  const spineError = (await lastMessage(page, "reader:error")).payload as {
    code: string;
  };
  expect(spineError.code).toBe("E_LOCATOR_SPINE_MISMATCH");
  await waitForReason(page, "go-to", 1);
  const crossPosition = (await lastReason(page, "go-to")) as {
    locator: { textOffset: number };
    pageIndex: number;
  };
  expect(crossPosition.pageIndex).toBe(0);
  expect(crossPosition.locator.textOffset).toBeLessThanOrEqual(initPayload.locator.textOffset + 2);

  const missing = locator({
    spineIndex: 1,
    spineHref: "chapter2.xhtml",
    textOffset: 9_007_199_254_740_990,
    intraSpineProgression: 0.5,
    quote: { exact: "完全不存在的片段", prefix: "", suffix: "" },
  });
  await send(page, command("reader:go-to", { locator: missing }));
  await waitForCount(page, "reader:error", 2);
  const textError = (await lastMessage(page, "reader:error")).payload as {
    code: string;
  };
  expect(textError.code).toBe("E_LOCATOR_NOT_FOUND");
  await waitForReason(page, "go-to", 2);
  const missingPosition = (await lastReason(page, "go-to")) as {
    locator: { textOffset: number };
    pageIndex: number;
  };
  expect(missingPosition.pageIndex).toBe(0);
  expect(missingPosition.locator.textOffset).toBeLessThanOrEqual(
    initPayload.locator.textOffset + 2,
  );
  await page.waitForTimeout(200);
  expect((await messages(page, "reader:error")).length).toBe(2);
});
