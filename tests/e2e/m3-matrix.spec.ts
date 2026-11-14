import { readFile } from "node:fs/promises";

import { expect, type Page, test } from "@playwright/test";

declare const Buffer: { from: (value: Uint8Array) => unknown };

const TOKEN = "a".repeat(32);
const SESSION_ID = "123e4567-e89b-42d3-a456-426614174000";
const PARENT_HOST = "qurner.test";
const IFRAME_HOST = `${TOKEN}.test`;
const IFRAME_ORIGIN = `http://${IFRAME_HOST}`;
const NONCE = "0123456789abcdef0123456789abcdef";
const STYLE_CSS =
  'p.styled { text-indent: 2em; }\n@font-face { font-family: "QurnerTest"; src: url("font.ttf"); }\np.font { font-family: "QurnerTest", serif; }\n';
const COVER_PNG = Uint8Array.from([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
  0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
  0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
  0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
  0x42, 0x60, 0x82,
]);
const FONT_TTF = Uint8Array.from([
  0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
  0x00, 0x00, 0x00, 0x00,
]);

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

function parentHtml(): string {
  return `<!doctype html>
<html>
  <head><meta charset="utf-8"><title>qurner m3 matrix</title></head>
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

function readerHtml(body: string, spineIndex: number, spineHref: string): string {
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
  return { protocolVersion: 1, sessionId: SESSION_ID, sessionToken: TOKEN };
}

function initMessage(mode: "paged" | "scrolling"): Record<string, unknown> {
  return { ...baseMessage(), type: "reader:init", payload: { mode, initialLocator: null } };
}

function command(type: string, payload: Record<string, unknown>): Record<string, unknown> {
  return { ...baseMessage(), type, payload };
}

interface FixturePage {
  id: string;
  body: string;
  spineIndex: number;
  spineHref: string;
  style?: boolean;
  image?: boolean;
  font?: boolean;
}

const FIXTURES: FixturePage[] = [
  { id: "M3-F01", body: "<p>这是第一章。</p>", spineIndex: 0, spineHref: "chapter1.xhtml" },
  { id: "M3-F02", body: "<p>这是第一章。</p>", spineIndex: 0, spineHref: "chapter1.xhtml" },
  {
    id: "M3-F03",
    body: '<h1 id="s1">第一节</h1><p>第一节正文。</p><h2 id="s2">第二节</h2><p>第二节正文。</p>',
    spineIndex: 0,
    spineHref: "chapter1.xhtml",
  },
  { id: "M3-F04", body: "<p>没有元数据。</p>", spineIndex: 0, spineHref: "chapter1.xhtml" },
  {
    id: "M3-F05",
    body: '<link rel="stylesheet" href="style.css"/><p class="styled">带样式的正文。</p><img id="cover" src="cover.png" alt="封面"/><p class="font">字体文本。</p>',
    spineIndex: 0,
    spineHref: "chapter1.xhtml",
    style: true,
    image: true,
    font: true,
  },
  {
    id: "M3-F06",
    body: "<p>这是中文第一章。</p><p>This is chapter two.</p><p>😀 emoji 混排。</p>",
    spineIndex: 0,
    spineHref: "chapter1.xhtml",
  },
];

interface PageResources {
  requests: string[];
  responses: Array<{ url: string; status: number }>;
}

async function setupPage(page: Page, fixture: FixturePage, engine: string): Promise<PageResources> {
  const resources: PageResources = { requests: [], responses: [] };
  page.on("request", (request) => resources.requests.push(request.url()));
  page.on("response", (response) =>
    resources.responses.push({ url: response.url(), status: response.status() }),
  );
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
        body: readerHtml(fixture.body, fixture.spineIndex, fixture.spineHref),
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
    if (url.hostname === IFRAME_HOST && url.pathname === "/style.css") {
      await route.fulfill({
        status: 200,
        headers: { "Content-Type": "text/css; charset=utf-8", "Cache-Control": "no-store" },
        body: STYLE_CSS,
      });
      return;
    }
    if (url.hostname === IFRAME_HOST && url.pathname === "/cover.png") {
      await route.fulfill({
        status: 200,
        headers: { "Content-Type": "image/png", "Cache-Control": "no-store" },
        body: Buffer.from(COVER_PNG) as never,
      });
      return;
    }
    if (url.hostname === IFRAME_HOST && url.pathname === "/font.ttf") {
      await route.fulfill({
        status: 200,
        headers: { "Content-Type": "font/ttf", "Cache-Control": "no-store" },
        body: Buffer.from(FONT_TTF) as never,
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
  return resources;
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

async function lastReason(page: Page, reason: string): Promise<Record<string, unknown>> {
  const list = (await messages(page, "reader:location-changed")).filter(
    (message) => (message.payload as { reason?: string }).reason === reason,
  );
  const last = list[list.length - 1];
  expect(last).toBeDefined();
  return (last?.payload ?? {}) as Record<string, unknown>;
}

for (const fixture of FIXTURES) {
  test(`${fixture.id} loads in WebKit with engine init and assets`, async ({ page }) => {
    const engine = await engineSource();
    const resources = await setupPage(page, fixture, engine);
    await send(page, initMessage("paged"));
    await waitForCount(page, "reader:ready", 1);
    await waitForReason(page, "init", 1);
    const initPayload = (await lastReason(page, "init")) as {
      mode: string;
      pageIndex: number;
      pageCount: number;
      locator: { spineIndex: number; spineHref: string };
    };
    expect(initPayload.mode).toBe("paged");
    expect(initPayload.pageIndex).toBe(0);
    expect(initPayload.locator.spineIndex).toBe(fixture.spineIndex);
    expect(initPayload.locator.spineHref).toBe(fixture.spineHref);

    if (fixture.style) {
      const styled = await page.frame({ url: `${IFRAME_ORIGIN}/reader.html` })?.evaluate(() => {
        const host = globalThis as unknown as {
          document: { querySelector: (selector: string) => unknown | null };
          getComputedStyle: (element: unknown) => { textIndent: string };
        };
        const element = host.document.querySelector(".styled");
        return element ? host.getComputedStyle(element).textIndent : null;
      });
      expect(styled).toBe("32px");
      expect(resources.responses.some((response) => response.url.endsWith("/style.css"))).toBe(
        true,
      );
      const imageLoaded = await page
        .frame({ url: `${IFRAME_ORIGIN}/reader.html` })
        ?.evaluate(() => {
          const host = globalThis as unknown as {
            document: {
              getElementById: (id: string) => {
                complete: boolean;
                naturalWidth: number;
              } | null;
            };
          };
          const image = host.document.getElementById("cover");
          return image ? { complete: image.complete, naturalWidth: image.naturalWidth } : null;
        });
      expect(imageLoaded).not.toBeNull();
      if (imageLoaded) {
        expect(imageLoaded.complete).toBe(true);
        expect(imageLoaded.naturalWidth).toBeGreaterThanOrEqual(1);
      }
      expect(resources.responses.some((response) => response.url.endsWith("/cover.png"))).toBe(
        true,
      );
    }
    if (fixture.font) {
      expect(resources.requests.some((url) => url.endsWith("/font.ttf"))).toBe(true);
      const fontResponse = resources.responses.find((response) =>
        response.url.endsWith("/font.ttf"),
      );
      expect(fontResponse?.status).toBe(200);
    }

    // 无出站网络请求：只有父页与同源 iframe 资源。
    const external = resources.requests.filter((url) => {
      const host = parseUrl(url).hostname;
      return host !== PARENT_HOST && host !== IFRAME_HOST;
    });
    expect(external).toEqual([]);
    (globalThis as unknown as { console: { log: (...args: unknown[]) => void } }).console.log(
      `M3D_WEBKIT ${fixture.id} ok ready=1 requests=${resources.requests.length}`,
    );
  });
}

test("paged turn-page p95 stays within 150 ms (20 interactions)", async ({ page }) => {
  const engine = await engineSource();
  const longBody = Array.from(
    { length: 60 },
    (_, index) =>
      `<p>第 ${index} 段：分页性能测试正文，包含中文和 English 以及 emoji 😀。${"继续填充正文内容。".repeat(6)}</p>`,
  ).join("");
  const fixture: FixturePage = {
    id: "M3-PERF",
    body: longBody,
    spineIndex: 0,
    spineHref: "chapter1.xhtml",
  };
  const resources = await setupPage(page, fixture, engine);
  await send(page, initMessage("paged"));
  await waitForCount(page, "reader:ready", 1);
  await waitForReason(page, "init", 1);

  // 先向后翻 3 页，避免测量落到第 0 页或末页边界。
  for (let index = 1; index <= 3; index += 1) {
    await send(page, command("reader:turn-page", { direction: "next" }));
    await waitForCount(page, "reader:page-changed", index);
  }
  const pageCount = ((await lastReason(page, "init")) as { pageCount: number }).pageCount;
  expect(pageCount).toBeGreaterThan(5);

  const timings = await page.evaluate(
    async ({ count, sessionId, token }) => {
      const host = globalThis as unknown as {
        __messages: Array<{ data?: { type?: string } }>;
        document: {
          getElementById: (id: string) => {
            contentWindow: { postMessage: (data: unknown, origin: string) => void };
          } | null;
        };
        performance: { now: () => number };
        setTimeout: (callback: () => void, milliseconds: number) => unknown;
      };
      const frame = host.document.getElementById("reader");
      if (!frame) {
        throw new Error("missing reader frame");
      }
      const target = frame.contentWindow;
      const base = {
        protocolVersion: 1,
        sessionId,
        sessionToken: token,
      };
      const waitForPageChanged = (from: number) =>
        new Promise<void>((resolve) => {
          const check = () => {
            for (let index = from; index < host.__messages.length; index += 1) {
              if (host.__messages[index]?.data?.type === "reader:page-changed") {
                resolve();
                return;
              }
            }
            host.setTimeout(check, 1);
          };
          check();
        });
      const results: number[] = [];
      for (let index = 0; index < count; index += 1) {
        const from = host.__messages.length;
        const direction = index % 2 === 0 ? "next" : "prev";
        const start = host.performance.now();
        target.postMessage({ ...base, type: "reader:turn-page", payload: { direction } }, "*");
        await waitForPageChanged(from);
        results.push(host.performance.now() - start);
      }
      return results;
    },
    { count: 20, sessionId: SESSION_ID, token: TOKEN },
  );

  expect(timings).toHaveLength(20);
  const sorted = [...timings].sort((left, right) => left - right);
  const p95 = sorted[Math.ceil(0.95 * sorted.length) - 1] ?? Number.POSITIVE_INFINITY;
  const p50 = sorted[Math.floor(sorted.length / 2)] ?? Number.POSITIVE_INFINITY;
  const printer = (globalThis as unknown as { console: { log: (...args: unknown[]) => void } })
    .console;
  printer.log("M3D_TURN_TIMINGS", JSON.stringify(timings.map((value) => Number(value.toFixed(2)))));
  printer.log("M3D_TURN_P50", p50.toFixed(2), "M3D_TURN_P95", p95.toFixed(2));
  expect(p95).toBeLessThanOrEqual(150);
  await waitForCount(page, "reader:page-changed", 23);
  expect((await messages(page, "reader:page-changed")).length).toBe(23);
  const external = resources.requests.filter((url) => {
    const host = parseUrl(url).hostname;
    return host !== PARENT_HOST && host !== IFRAME_HOST;
  });
  expect(external).toEqual([]);
});
