import { readFile } from "node:fs/promises";

import { expect, type Frame, type Page, test } from "@playwright/test";

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

function parseUrl(raw: string): { hostname: string; pathname: string } {
  const rest = raw.replace(/^[a-z]+:\/\//, "");
  const slash = rest.indexOf("/");
  const host = slash === -1 ? rest : rest.slice(0, slash);
  const path = slash === -1 ? "/" : rest.slice(slash);
  return { hostname: host, pathname: path.split("?")[0] ?? "/" };
}

function bodyHtml(): string {
  const paragraphs = Array.from(
    { length: 40 },
    (_, index) =>
      `<p>第 ${index} 段：阅读导航测试段落，包含中文与 English。${"继续填充正文内容。".repeat(4)}</p>`,
  ).join("");
  return `
    <h2 id="start">开始</h2>
    <p id="styled" style="color: rgb(200, 30, 30); font-family: Georgia, serif;">特殊颜色文本</p>
    <p><img id="inline-image" alt="" style="width: 120px; height: 80px" src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==" /></p>
    <p><span style="display:inline-block;width:60%"></span><span id="inline-target">内联目标</span></p>
    ${paragraphs}
    <p><input id="t-input" aria-label="输入框" /></p>
    <p><textarea id="t-textarea" aria-label="文本域"></textarea></p>
    <p><select id="t-select" aria-label="下拉"><option>1</option><option>2</option></select></p>
    <p><a id="t-link" href="#start">链接</a></p>
    <div id="t-editable" contenteditable="true">可编辑</div>
    <h2 id="middle">中间</h2>
    ${paragraphs}
    <h2 id="end">末尾</h2>
    ${paragraphs}
    ${paragraphs}
    <h2 id="late">靠后</h2>
    <button id="focus-button" type="button">按钮</button>
  `;
}

function parentHtml(noReferrer = false): string {
  const referrer = noReferrer ? ' referrerpolicy="no-referrer"' : "";
  return `<!doctype html>
<html>
  <head><meta charset="utf-8"><title>qurner navigation</title></head>
  <body style="margin: 0">
    <iframe
      id="reader"
      sandbox="allow-scripts"${referrer}
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

function readerHtml(): string {
  return `<!doctype html>
<html>
  <head>
    <meta charset="utf-8"><title>reader</title>
    <style>body { background-color: rgb(255, 255, 255); color: rgb(10, 10, 10); } #styled { font-size: 22px; }</style>
  </head>
  <body tabindex="0">
    ${bodyHtml()}
    <script nonce="${NONCE}" src="${IFRAME_ORIGIN}/reader-engine.js" data-session-id="${SESSION_ID}" data-spine-index="0" data-spine-href="chapter1.xhtml"></script>
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
  initialNavigation?: unknown,
  spineIndex = 0,
): Record<string, unknown> {
  return {
    ...baseMessage(),
    type: "reader:init",
    payload: {
      spineIndex,
      settings: { mode, fontScale: 1, theme: "day" },
      initialLocator,
      ...(initialNavigation === undefined ? {} : { initialNavigation }),
    },
  };
}

async function setupPage(page: Page, options: { noReferrer?: boolean } = {}): Promise<void> {
  const engine = await readFile("src-tauri/resources/reader-engine.js", "utf8");
  await page.route("**/*", async (route) => {
    const url = parseUrl(route.request().url());
    if (url.hostname === PARENT_HOST && url.pathname === "/parent.html") {
      await route.fulfill({
        status: 200,
        headers: { "Content-Type": "text/html; charset=utf-8" },
        body: parentHtml(options.noReferrer === true),
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
        body: readerHtml(),
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
}

function command(type: string, payload: Record<string, unknown>): Record<string, unknown> {
  return { ...baseMessage(), type, payload };
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

function readerFrame(page: Page): Frame {
  const frame = page.frames().find((candidate) => candidate.url().includes("reader.html"));
  if (!frame) {
    throw new Error("missing reader frame");
  }
  return frame;
}

interface ReaderRect {
  top: number;
  left: number;
  width: number;
  height: number;
}

interface ReaderDocument {
  body: {
    innerHTML: string;
    scrollTop: number;
    style: { fontSize: string; backgroundColor: string; color: string };
  };
  documentElement: {
    scrollTop: number;
    scrollLeft: number;
    scrollHeight: number;
    scrollWidth: number;
    clientHeight: number;
    clientWidth: number;
    style: { getPropertyValue(name: string): string };
  };
  style: { getPropertyValue(name: string): string };
  getElementById(id: string): {
    getBoundingClientRect(): ReaderRect;
    getAttribute(name: string): string | null;
  } | null;
}

async function elementGeometry(
  frame: Frame,
  id: string,
): Promise<{ rect: ReaderRect; viewportWidth: number; viewportHeight: number } | null> {
  return frame.evaluate((elementId) => {
    const doc = (globalThis as unknown as { document: ReaderDocument }).document;
    const element = doc.getElementById(elementId);
    if (!element) {
      return null;
    }
    return {
      rect: element.getBoundingClientRect(),
      viewportWidth: doc.documentElement.clientWidth,
      viewportHeight: doc.documentElement.clientHeight,
    };
  }, id);
}

async function initLocation(page: Page): Promise<{
  pageIndex: number;
  pageCount: number;
} | null> {
  return page.evaluate(() => {
    const entries = (
      globalThis as unknown as {
        __messages: Array<{ data?: { type?: string; payload?: unknown } }>;
      }
    ).__messages;
    const found = entries
      .filter((entry) => entry.data?.type === "reader:location-changed")
      .map(
        (entry) =>
          entry.data?.payload as {
            reason?: string;
            pageIndex: number;
            pageCount: number;
          },
      )
      .filter((payload) => payload.reason === "init")
      .at(0);
    return found ?? null;
  });
}

async function scrollTop(frame: Frame): Promise<number> {
  return frame.evaluate(
    () =>
      (globalThis as unknown as { document: ReaderDocument }).document.documentElement.scrollTop,
  );
}

async function focusBody(frame: Frame): Promise<void> {
  await frame.locator("body").click({ position: { x: 10, y: 10 }, force: true });
}

/** 在 WebKit iframe 内派发可冒泡 keydown；返回是否被 preventDefault。 */
async function pressKey(
  frame: Frame,
  key: string,
  options: {
    target?: string;
    ctrlKey?: boolean;
    metaKey?: boolean;
    altKey?: boolean;
  } = {},
): Promise<boolean> {
  return frame.evaluate(
    ({ key: value, target, ctrlKey, metaKey, altKey }) => {
      const win = globalThis as unknown as {
        document: {
          body: { dispatchEvent: (event: unknown) => boolean };
          querySelector: (
            selector: string,
          ) => { dispatchEvent: (event: unknown) => boolean } | null;
        };
        KeyboardEvent: new (
          type: string,
          init: {
            key: string;
            bubbles: boolean;
            cancelable: boolean;
            ctrlKey?: boolean;
            metaKey?: boolean;
            altKey?: boolean;
          },
        ) => { defaultPrevented: boolean };
      };
      const element = target ? win.document.querySelector(target) : win.document.body;
      if (!element) {
        throw new Error("missing key target");
      }
      const event = new win.KeyboardEvent("keydown", {
        key: value,
        bubbles: true,
        cancelable: true,
        ctrlKey,
        metaKey,
        altKey,
      });
      element.dispatchEvent(event);
      return event.defaultPrevented;
    },
    {
      key,
      target: options.target,
      ctrlKey: options.ctrlKey,
      metaKey: options.metaKey,
      altKey: options.altKey,
    },
  );
}

test("no-referrer 跨 origin iframe 仍完成握手（打包回归）", async ({ page }) => {
  await setupPage(page, { noReferrer: true });
  const frame = readerFrame(page);
  // 真实打包环境条件：跨 origin iframe 且父 referrer 被 no-referrer 策略清空。
  expect(
    await frame.evaluate(() => {
      const win = globalThis as unknown as {
        document: { referrer: string };
      };
      return win.document.referrer;
    }),
  ).toBe("");

  await send(page, initMessage("scrolling", null));
  await waitForCount(page, "reader:ready", 1);
  await waitForCount(page, "reader:location-changed", 1);

  // 握手后可正常交互（翻页事件回到父页面）。
  await send(page, command("reader:turn-page", { direction: "next" }));
  await waitForCount(page, "reader:location-changed", 3);
});

test("initialNavigation=fragment 定位到 fragment 元素", async ({ page }) => {
  await setupPage(page);
  await send(page, initMessage("scrolling", null, { kind: "fragment", fragment: "middle" }));
  await waitForCount(page, "reader:ready", 1);
  const frame = readerFrame(page);
  const position = await frame.evaluate(() => {
    const doc = (globalThis as unknown as { document: ReaderDocument }).document;
    const element = doc.getElementById("middle");
    if (!element) return -1;
    return element.getBoundingClientRect().top + doc.documentElement.scrollTop;
  });
  expect(position).toBeGreaterThan(0);
  expect(await scrollTop(frame)).toBeGreaterThan(0);
  expect(await scrollTop(frame)).toBeLessThan(position + 5);
});

test("分页模式 fragment 定位到包含目标的页", async ({ page }) => {
  await setupPage(page);
  await send(page, initMessage("paged", null, { kind: "fragment", fragment: "middle" }));
  await waitForCount(page, "reader:ready", 1);
  const frame = readerFrame(page);
  const init = await initLocation(page);
  expect(init?.pageCount ?? 0).toBeGreaterThan(1);
  expect(init?.pageIndex ?? 0).toBeGreaterThan(0);
  const geometry = await elementGeometry(frame, "middle");
  expect(geometry).not.toBeNull();
  expect(geometry?.rect.left).toBeGreaterThanOrEqual(0);
  expect(geometry?.rect.left).toBeLessThan(geometry?.viewportWidth ?? 0);
  expect(geometry?.rect.top).toBeGreaterThanOrEqual(0);
  expect(geometry?.rect.top).toBeLessThan(geometry?.viewportHeight ?? 0);
});

test("分页模式 inline 非列首 fragment 定位到当前页", async ({ page }) => {
  await setupPage(page);
  await send(page, initMessage("paged", null, { kind: "fragment", fragment: "inline-target" }));
  await waitForCount(page, "reader:ready", 1);
  const frame = readerFrame(page);
  const init = await initLocation(page);
  expect(init?.pageIndex).toBe(0);
  expect(init?.pageCount ?? 0).toBeGreaterThan(1);
  const geometry = await elementGeometry(frame, "inline-target");
  expect(geometry).not.toBeNull();
  const width = geometry?.viewportWidth ?? 0;
  expect(geometry?.rect.left).toBeGreaterThan(width * 0.3);
  expect(geometry?.rect.left).toBeLessThan(width);
  expect(geometry?.rect.top).toBeGreaterThanOrEqual(0);
  expect(geometry?.rect.top).toBeLessThan(geometry?.viewportHeight ?? 0);
});

test("分页模式 fragment 在末页附近仍定位到目标页", async ({ page }) => {
  await setupPage(page);
  await send(page, initMessage("paged", null, { kind: "fragment", fragment: "late" }));
  await waitForCount(page, "reader:ready", 1);
  const frame = readerFrame(page);
  const init = await initLocation(page);
  expect(init?.pageCount ?? 0).toBeGreaterThan(1);
  expect(init?.pageIndex ?? 0).toBeGreaterThanOrEqual((init?.pageCount ?? 1) - 2);
  const geometry = await elementGeometry(frame, "late");
  expect(geometry).not.toBeNull();
  expect(geometry?.rect.left).toBeGreaterThanOrEqual(0);
  expect(geometry?.rect.left).toBeLessThan(geometry?.viewportWidth ?? 0);
  expect(geometry?.rect.top).toBeGreaterThanOrEqual(0);
  expect(geometry?.rect.top).toBeLessThan(geometry?.viewportHeight ?? 0);
});

test("fragment 不存在时回到章节起点并正常 ready", async ({ page }) => {
  await setupPage(page);
  await send(page, initMessage("paged", null, { kind: "fragment", fragment: "missing-id" }));
  await waitForCount(page, "reader:ready", 1);
  const init = await initLocation(page);
  expect(init?.pageIndex).toBe(0);
  const errors = await page.evaluate(() => {
    const entries = (
      globalThis as unknown as {
        __messages: Array<{ data?: { type?: string } }>;
      }
    ).__messages;
    return entries.filter((entry) => entry.data?.type === "reader:error").length;
  });
  expect(errors).toBe(0);
});

test("initialNavigation=end 定位到章节末尾", async ({ page }) => {
  await setupPage(page);
  await send(page, initMessage("scrolling", null, { kind: "end" }));
  await waitForCount(page, "reader:ready", 1);
  const frame = readerFrame(page);
  await expect
    .poll(async () => {
      const max = await frame.evaluate(() => {
        const doc = (globalThis as unknown as { document: ReaderDocument }).document;
        return doc.documentElement.scrollHeight - doc.documentElement.clientHeight;
      });
      return (await scrollTop(frame)) >= max - 2;
    })
    .toBe(true);
});

test("滚动模式左右键滚动一屏，上下键保留原生滚动", async ({ page }) => {
  await setupPage(page);
  await send(page, initMessage("scrolling", null, { kind: "start" }));
  await waitForCount(page, "reader:ready", 1);
  const frame = readerFrame(page);
  await focusBody(frame);
  const before = await scrollTop(frame);

  await pressKey(frame, "ArrowRight");
  await expect.poll(() => scrollTop(frame)).toBeGreaterThan(before + 100);
  const afterRight = await scrollTop(frame);
  await pressKey(frame, "ArrowLeft");
  await expect.poll(() => scrollTop(frame)).toBeLessThan(afterRight - 100);

  const afterLeft = await scrollTop(frame);
  expect(afterLeft).toBeGreaterThanOrEqual(0);

  const downPrevented = await pressKey(frame, "ArrowDown");
  expect(downPrevented, "上下键不得被拦截").toBe(false);
  await page.waitForTimeout(100);
  const spineRequests = await page.evaluate(() => {
    const entries = (
      globalThis as unknown as {
        __messages: Array<{ data?: { type?: string } }>;
      }
    ).__messages;
    return entries.filter((entry) => entry.data?.type === "reader:spine-request").length;
  });
  expect(spineRequests).toBe(0);

  const beforeCtrl = await scrollTop(frame);
  const ctrlPrevented = await pressKey(frame, "ArrowRight", { ctrlKey: true });
  expect(ctrlPrevented).toBe(false);
  await page.waitForTimeout(100);
  expect(await scrollTop(frame)).toBe(beforeCtrl);
});

test("update-settings 即时应用字号与主题且保留图片和特殊颜色", async ({ page }) => {
  await setupPage(page);
  await send(page, initMessage("paged", null, { kind: "start" }));
  await waitForCount(page, "reader:ready", 1);
  const frame = readerFrame(page);
  await waitForCount(page, "reader:location-changed", 1);
  const before = await frame.evaluate(() => {
    const win = globalThis as unknown as {
      document: ReaderDocument & {
        body: { innerHTML: string };
      };
      getComputedStyle: (element: unknown) => {
        fontSize: string;
        color: string;
        fontFamily: string;
        filter: string;
        backgroundColor: string;
      };
    };
    const doc = win.document;
    const styled = doc.getElementById("styled");
    const image = doc.getElementById("inline-image");
    return {
      fontSize: win.getComputedStyle(styled).fontSize,
      color: win.getComputedStyle(styled).color,
      family: win.getComputedStyle(styled).fontFamily,
      imageWidth: image?.getBoundingClientRect().width ?? 0,
      imageStyle: image?.getAttribute("style") ?? null,
      imageFilter: win.getComputedStyle(image).filter,
      bodyHtml: doc.body.innerHTML,
      background: win.getComputedStyle(doc.documentElement).backgroundColor,
      bodyBackground: win.getComputedStyle(doc.body).backgroundColor,
      bodyColor: win.getComputedStyle(doc.body).color,
      scale: doc.documentElement.style.getPropertyValue("--qurner-font-scale"),
    };
  });

  const inlineFont = () =>
    frame.evaluate(() => {
      const win = globalThis as unknown as {
        document: ReaderDocument;
        getComputedStyle: (element: unknown) => { fontSize: string };
      };
      const element = win.document.getElementById("inline-target");
      return win.getComputedStyle(element).fontSize;
    });
  const inlineBefore = await inlineFont();

  const initPayload = await page.evaluate(() => {
    const entries = (
      globalThis as unknown as {
        __messages: Array<{ data?: { type?: string; payload?: unknown } }>;
      }
    ).__messages;
    return entries
      .filter((entry) => entry.data?.type === "reader:location-changed")
      .map(
        (entry) =>
          entry.data?.payload as {
            reason?: string;
            locator: { textOffset: number; quote: { exact: string } };
          },
      )
      .filter((payload) => payload.reason === "init")
      .at(0);
  });

  await send(
    page,
    command("reader:update-settings", {
      settings: { mode: "paged", fontScale: 1.5, theme: "night" },
    }),
  );
  await waitForCount(page, "reader:location-changed", 3);

  const after = await frame.evaluate(() => {
    const win = globalThis as unknown as {
      document: ReaderDocument & {
        body: { innerHTML: string };
      };
      getComputedStyle: (element: unknown) => {
        fontSize: string;
        color: string;
        fontFamily: string;
        filter: string;
        backgroundColor: string;
      };
    };
    const doc = win.document;
    const styled = doc.getElementById("styled");
    const image = doc.getElementById("inline-image");
    return {
      fontSize: win.getComputedStyle(styled).fontSize,
      color: win.getComputedStyle(styled).color,
      family: win.getComputedStyle(styled).fontFamily,
      imageWidth: image?.getBoundingClientRect().width ?? 0,
      imageStyle: image?.getAttribute("style") ?? null,
      imageFilter: win.getComputedStyle(image).filter,
      bodyHtml: doc.body.innerHTML,
      background: win.getComputedStyle(doc.documentElement).backgroundColor,
      bodyBackground: win.getComputedStyle(doc.body).backgroundColor,
      bodyColor: win.getComputedStyle(doc.body).color,
      scale: doc.documentElement.style.getPropertyValue("--qurner-font-scale"),
    };
  });

  expect(after.scale).toBe("1.5");
  const inlineAfter = await inlineFont();
  expect(Number.parseFloat(inlineAfter)).toBeGreaterThan(Number.parseFloat(inlineBefore));
  expect(after.color).toBe(before.color);
  expect(after.family).toBe(before.family);
  expect(after.imageWidth).toBe(before.imageWidth);
  expect(after.imageStyle).toBe(before.imageStyle);
  expect(after.imageFilter).toBe("none");
  expect(after.bodyHtml).toBe(before.bodyHtml);
  expect(after.background).not.toBe(before.background);
  expect(before.background).not.toBe("rgba(0, 0, 0, 0)");
  expect(after.bodyBackground).not.toBe(before.bodyBackground);
  expect(before.bodyBackground).toBe("rgb(246, 243, 236)");
  expect(after.bodyColor).not.toBe(before.bodyColor);

  const layout = await page.evaluate(() => {
    const entries = (
      globalThis as unknown as {
        __messages: Array<{ data?: { type?: string; payload?: unknown } }>;
      }
    ).__messages;
    return entries
      .filter((entry) => entry.data?.type === "reader:location-changed")
      .map(
        (entry) =>
          entry.data?.payload as {
            reason?: string;
            locator: { textOffset: number; quote: { exact: string } };
          },
      )
      .filter((payload) => payload.reason === "layout")
      .at(-1);
  });
  expect(
    Math.abs((layout?.locator.textOffset ?? 0) - (initPayload?.locator.textOffset ?? 0)),
  ).toBeLessThanOrEqual(2);
});

test("正文允许选择且 Cmd/Ctrl+C 触发复制事件（剪贴板内容人工验证）", async ({ page }) => {
  await setupPage(page);
  await send(page, initMessage("scrolling", null, { kind: "start" }));
  await waitForCount(page, "reader:ready", 1);
  const frame = readerFrame(page);

  const selection = await frame.evaluate(() => {
    const win = globalThis as unknown as {
      document: {
        body: { innerHTML: string };
        getElementById: (id: string) => { textContent: string | null } | null;
        createRange: () => {
          selectNodeContents: (node: unknown) => void;
        };
      };
      getComputedStyle: (element: unknown) => {
        userSelect: string;
        webkitUserSelect?: string;
      };
      getSelection: () => {
        removeAllRanges: () => void;
        addRange: (range: unknown) => void;
        toString: () => string;
      };
    };
    const body = win.document.getElementById("styled");
    const range = win.document.createRange();
    range.selectNodeContents(body);
    const selected = win.getSelection();
    selected.removeAllRanges();
    selected.addRange(range);
    const style = win.getComputedStyle(win.document.body);
    return {
      text: selected.toString(),
      userSelect: style.userSelect,
      webkitUserSelect: style.webkitUserSelect ?? "",
      bodyHtml: win.document.body.innerHTML,
    };
  });
  expect(selection.text).toContain("特殊颜色文本");
  expect(selection.userSelect === "text" || selection.webkitUserSelect === "text").toBe(true);

  // 聚焦 iframe 文档后重新选择并监听 copy 事件，证明 Cmd/Ctrl+C 到达阅读文档。
  await frame.locator("#styled").click();
  await frame.evaluate(() => {
    const win = globalThis as unknown as {
      document: {
        getElementById: (id: string) => { textContent: string | null } | null;
        createRange: () => { selectNodeContents: (node: unknown) => void };
        addEventListener: (type: string, listener: () => void) => void;
      };
      getSelection: () => {
        removeAllRanges: () => void;
        addRange: (range: unknown) => void;
      };
      __copyEvents?: number;
    };
    win.__copyEvents = 0;
    win.document.addEventListener("copy", () => {
      win.__copyEvents = (win.__copyEvents ?? 0) + 1;
    });
    const range = win.document.createRange();
    range.selectNodeContents(win.document.getElementById("styled"));
    const selected = win.getSelection();
    selected.removeAllRanges();
    selected.addRange(range);
  });
  await page.keyboard.press("Control+C");
  await page.keyboard.press("Meta+C");
  const copyEvents = await frame.evaluate(() => {
    const win = globalThis as unknown as { __copyEvents?: number };
    return win.__copyEvents ?? 0;
  });
  expect(copyEvents).toBeGreaterThan(0);

  // WebKit 测试环境通常不能读取剪贴板；内容需人工 GUI 验证，不在此断言。
  const clipboard = await frame.evaluate(async () => {
    try {
      const win = globalThis as unknown as {
        navigator?: { clipboard?: { readText: () => Promise<string> } };
      };
      if (!win.navigator?.clipboard?.readText) {
        return null;
      }
      return await win.navigator.clipboard.readText();
    } catch {
      return null;
    }
  });
  if (clipboard !== null) {
    expect(clipboard).toContain("特殊颜色文本");
  }

  const after = await frame.evaluate(() => {
    const win = globalThis as unknown as {
      document: { body: { innerHTML: string } };
    };
    return win.document.body.innerHTML;
  });
  expect(after).toBe(selection.bodyHtml);
});

test("Meta/Alt 组合键不翻页", async ({ page }) => {
  await setupPage(page);
  await send(page, initMessage("scrolling", null, { kind: "start" }));
  await waitForCount(page, "reader:ready", 1);
  const frame = readerFrame(page);
  await pressKey(frame, "ArrowRight");
  await page.waitForTimeout(150);
  const before = await scrollTop(frame);
  expect(before).toBeGreaterThan(0);

  await pressKey(frame, "ArrowRight", { metaKey: true });
  await page.waitForTimeout(100);
  expect(await scrollTop(frame)).toBe(before);

  await pressKey(frame, "ArrowLeft", { altKey: true });
  await page.waitForTimeout(100);
  expect(await scrollTop(frame)).toBe(before);
});

test("分页模式左右键翻页并跳过交互目标", async ({ page }) => {
  await setupPage(page);
  await send(page, initMessage("paged", null, { kind: "start" }));
  await waitForCount(page, "reader:ready", 1);
  await waitForCount(page, "reader:location-changed", 1);
  const frame = readerFrame(page);
  await focusBody(frame);
  const initial = await page.evaluate(() => {
    const entries = (
      globalThis as unknown as {
        __messages: Array<{ data?: { type?: string; payload?: unknown } }>;
      }
    ).__messages;
    const found = entries.find((entry) => entry.data?.type === "reader:location-changed");
    return found?.data?.payload as { pageIndex: number; pageCount: number } | undefined;
  });

  expect(initial?.pageCount ?? 0).toBeGreaterThan(1);

  await pressKey(frame, "ArrowRight");
  await expect
    .poll(async () => {
      const entries = await page.evaluate(() => {
        const list = (
          globalThis as unknown as {
            __messages: Array<{ data?: { type?: string; payload?: unknown } }>;
          }
        ).__messages;
        return list
          .filter((entry) => entry.data?.type === "reader:page-changed")
          .map((entry) => entry.data?.payload as { pageIndex: number });
      });
      return entries.at(-1)?.pageIndex ?? 0;
    })
    .toBeGreaterThan(0);

  await frame.locator("#focus-button").focus();
  const beforeButton = await page.evaluate(() => {
    const list = (
      globalThis as unknown as {
        __messages: Array<{ data?: { type?: string; payload?: unknown } }>;
      }
    ).__messages;
    return list
      .filter((entry) => entry.data?.type === "reader:page-changed")
      .map((entry) => entry.data?.payload as { pageIndex: number });
  });
  const buttonPrevented = await pressKey(frame, "ArrowRight", {
    target: "#focus-button",
  });
  expect(buttonPrevented).toBe(false);
  await page.waitForTimeout(100);
  const afterButton = await page.evaluate(() => {
    const list = (
      globalThis as unknown as {
        __messages: Array<{ data?: { type?: string; payload?: unknown } }>;
      }
    ).__messages;
    return list
      .filter((entry) => entry.data?.type === "reader:page-changed")
      .map((entry) => entry.data?.payload as { pageIndex: number });
  });
  expect(afterButton.length).toBe(beforeButton.length);
  expect(afterButton.at(-1)?.pageIndex).toBe(beforeButton.at(-1)?.pageIndex);
});
