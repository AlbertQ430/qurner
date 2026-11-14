import { expect, type Page, test } from "@playwright/test";

interface MockBook {
  id: string;
  title: string;
  author: string;
  coverUrl: string | null;
  importedAt: string;
  lastOpenedAt: string | null;
}

const BOOKS: MockBook[] = [
  {
    id: "b1",
    title: "中文书名",
    author: "作者甲",
    coverUrl: "cover://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    importedAt: "2026-11-14T00:00:00Z",
    lastOpenedAt: null,
  },
  {
    id: "b2",
    title: "English Title",
    author: "Author B",
    coverUrl: null,
    importedAt: "2026-11-13T00:00:00Z",
    lastOpenedAt: null,
  },
];

const THOUSAND: MockBook[] = Array.from({ length: 1_000 }, (_, index) => ({
  id: `book-${index}`,
  title: `书 ${index.toString().padStart(4, "0")}`,
  author: `作者 ${index}`,
  coverUrl: null,
  importedAt: "2026-11-14T00:00:00Z",
  lastOpenedAt: null,
}));

interface MockHost {
  __TAURI_INTERNALS__?: {
    invoke: (
      command: string,
      args?: {
        input?: { query?: string; sort?: string; direction?: string };
        bookId?: string;
      },
    ) => Promise<unknown>;
  };
  __listCalls: Array<{ query?: string; sort?: string; direction?: string }>;
  __softRemoveCalls: string[];
  __softRemoveError?: string;
  __books: MockBook[];
}

async function installMock(page: Page, books: MockBook[]): Promise<void> {
  await page.addInitScript((seed: MockBook[]) => {
    const host = window as unknown as MockHost;
    host.__listCalls = [];
    host.__softRemoveCalls = [];
    host.__books = seed.slice();
    host.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        if (command === "listBooks") {
          const input = args?.input ?? {};
          host.__listCalls.push(input);
          let result = host.__books.slice();
          if (input.query) {
            result = result.filter(
              (book) =>
                book.title.includes(input.query as string) ||
                book.author.includes(input.query as string),
            );
          }
          if (input.sort === "title") {
            result.sort((left, right) => left.title.localeCompare(right.title));
          } else if (input.sort === "author") {
            result.sort((left, right) => left.author.localeCompare(right.author));
          }
          if (input.direction === "asc") {
            result.reverse();
          }
          return result;
        }
        if (command === "softRemoveBook") {
          if (host.__softRemoveError) {
            throw { code: host.__softRemoveError, message: "mock error" };
          }
          host.__softRemoveCalls.push(args?.bookId ?? "");
          host.__books = host.__books.filter((book) => book.id !== args?.bookId);
          return null;
        }
        if (command === "selectImportFiles") {
          return [];
        }
        return null;
      },
    };
  }, books);
}

async function openLibrary(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 2, name: "书库" })).toBeVisible();
}

test("书库网格/列表显示封面与占位", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);

  await expect(page.getByText("中文书名")).toBeVisible();
  await expect(page.getByText("作者甲")).toBeVisible();
  await expect(page.locator(".book-grid-row").first()).toBeVisible();

  await page.getByRole("button", { name: "列表视图" }).click();
  await expect(page.locator(".book-list-row").first()).toBeVisible();
  await expect(page.getByText("中文书名")).toBeVisible();
  await expect(page.getByText("English Title")).toBeVisible();
  await expect(page.locator(".book-cover-placeholder").first()).toBeVisible();
});

test("搜索防抖、无匹配与清除", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  await expect(page.getByText("中文书名")).toBeVisible();

  const input = page.getByLabel("搜索书名或作者");
  await input.fill("中文");
  await page.waitForTimeout(80);
  let calls = await page.evaluate(() => (window as unknown as MockHost).__listCalls);
  expect(calls.some((call) => call.query === "中文")).toBe(false);

  await page.waitForTimeout(250);
  calls = await page.evaluate(() => (window as unknown as MockHost).__listCalls);
  expect(calls.some((call) => call.query === "中文")).toBe(true);

  await input.fill("没有这样书");
  await expect(page.getByText("没有匹配的书籍")).toBeVisible();
  await page.getByRole("button", { name: "清除搜索" }).first().click();
  await expect(page.getByText("中文书名")).toBeVisible();
});

test("排序与方向透传后端参数", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);

  await page.getByLabel("排序方式").selectOption("author");
  await expect
    .poll(async () =>
      page.evaluate(() => {
        const calls = (window as unknown as MockHost).__listCalls;
        return calls[calls.length - 1]?.sort;
      }),
    )
    .toBe("author");

  await page.getByRole("button", { name: "升序" }).click();
  await expect
    .poll(async () =>
      page.evaluate(() => {
        const calls = (window as unknown as MockHost).__listCalls;
        return calls[calls.length - 1]?.direction;
      }),
    )
    .toBe("asc");
});

test("封面：合法 cover 生成图片并在加载失败后回退占位，非法 scheme 不生成图片", async ({
  page,
}) => {
  await installMock(page, [
    BOOKS[0] as MockBook,
    { ...(BOOKS[1] as MockBook), coverUrl: "https://evil.test/cover.png" },
  ]);
  await page.addInitScript(() => {
    const host = globalThis as unknown as {
      __coverImgSeen?: boolean;
      document: {
        addEventListener: (type: string, listener: () => void) => void;
        body: unknown;
        querySelector: (selector: string) => unknown;
      };
      MutationObserver: new (
        callback: (records: unknown[]) => void,
      ) => {
        observe: (target: unknown, options: unknown) => void;
      };
    };
    host.__coverImgSeen = false;
    host.document.addEventListener("DOMContentLoaded", () => {
      const observer = new host.MutationObserver((records) => {
        for (const record of records) {
          const nodes = (record as { addedNodes?: unknown[] }).addedNodes ?? [];
          for (const node of nodes) {
            const element = node as {
              nodeType?: number;
              matches?: (selector: string) => boolean;
              querySelector?: (selector: string) => unknown;
            };
            if (element.nodeType !== 1) {
              continue;
            }
            if (element.matches?.('img[src^="cover://"]')) {
              host.__coverImgSeen = true;
            }
            if (element.querySelector?.('img[src^="cover://"]')) {
              host.__coverImgSeen = true;
            }
          }
        }
      });
      observer.observe(host.document.body, {
        childList: true,
        subtree: true,
        attributes: true,
      });
    });
  });
  await openLibrary(page);
  await expect(page.getByText("中文书名")).toBeVisible();

  await expect(page.locator('img[src^="https"]')).toHaveCount(0);
  await expect
    .poll(async () =>
      page.evaluate(() => (globalThis as unknown as { __coverImgSeen?: boolean }).__coverImgSeen),
    )
    .toBe(true);
  await expect(page.locator('img[src^="cover://"]')).toHaveCount(0);
  await expect(page.locator(".book-cover-placeholder")).toHaveCount(2);
});

test("软移除取消、Escape 与确认", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  await expect(page.getByText("中文书名")).toBeVisible();

  await page.getByRole("button", { name: "从书库移除《中文书名》" }).click();
  await expect(page.getByRole("dialog", { name: "从书库移除书籍？" })).toBeVisible();
  await page.getByRole("button", { name: "取消" }).click();
  await expect(page.getByRole("dialog")).toBeHidden();

  await page.getByRole("button", { name: "从书库移除《中文书名》" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();
  let calls = await page.evaluate(() => (window as unknown as MockHost).__softRemoveCalls);
  expect(calls).toEqual([]);

  await page.getByRole("button", { name: "从书库移除《中文书名》" }).click();
  await page.getByRole("button", { name: "确认移除" }).click();
  await expect(page.getByText("已从书库移除《中文书名》")).toBeVisible();
  await expect(page.getByText("中文书名")).toBeHidden();
  calls = await page.evaluate(() => (window as unknown as MockHost).__softRemoveCalls);
  expect(calls).toEqual(["b1"]);
});

test("软移除错误保留书籍并显示中文原因", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  await page.evaluate(() => {
    (window as unknown as MockHost).__softRemoveError = "E_STORAGE";
  });

  await page.getByRole("button", { name: "从书库移除《中文书名》" }).click();
  await page.getByRole("button", { name: "确认移除" }).click();
  await expect(page.getByText("无法更新书库，请确认磁盘空间后重试。")).toBeVisible();
  await expect(page.getByRole("listitem").filter({ hasText: "中文书名" })).toBeVisible();
});

test("1,000 本书虚拟滚动保持 DOM 受控并显示最后一项", async ({ page }) => {
  await installMock(page, THOUSAND);
  await openLibrary(page);
  await expect(page.getByText("书 0000")).toBeVisible();

  expect(await page.getByRole("listitem").count()).toBeLessThan(120);
  const firstRow = await page.getByRole("listitem").nth(0).boundingBox();
  const secondRow = await page.getByRole("listitem").nth(1).boundingBox();
  expect(firstRow).not.toBeNull();
  expect(secondRow).not.toBeNull();
  if (firstRow && secondRow) {
    expect(secondRow.y).toBeGreaterThanOrEqual(firstRow.y + firstRow.height - 0.5);
  }

  const scroller = page.locator(".library-scroll");
  await scroller.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await page.waitForTimeout(100);
  await expect(page.getByText("书 0999")).toBeVisible();
  expect(await page.getByRole("listitem").count()).toBeLessThan(120);
  const gridContainer = await page.locator(".library-scroll").boundingBox();
  const gridLast = await page.getByRole("listitem").last().boundingBox();
  expect(gridContainer).not.toBeNull();
  expect(gridLast).not.toBeNull();
  if (gridContainer && gridLast) {
    expect(gridLast.y + gridLast.height).toBeLessThanOrEqual(
      gridContainer.y + gridContainer.height + 1,
    );
  }

  await page.getByRole("button", { name: "列表视图" }).click();
  await expect(page.locator(".book-list-row").first()).toBeVisible();
  const listScroller = page.locator(".library-scroll");
  await listScroller.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await page.waitForTimeout(100);
  await expect(page.getByText("书 0999")).toBeVisible();
  expect(await page.getByRole("listitem").count()).toBeLessThan(120);

  const listRows = page.getByRole("listitem");
  const listFirst = await listRows.nth(-2).boundingBox();
  const listSecond = await listRows.nth(-1).boundingBox();
  if (listFirst && listSecond) {
    expect(listSecond.y).toBeGreaterThanOrEqual(listFirst.y + listFirst.height - 0.5);
  }
  const listContainer = await page.locator(".library-scroll").boundingBox();
  const listLast = await page.getByRole("listitem").last().boundingBox();
  if (listContainer && listLast) {
    expect(listLast.y + listLast.height).toBeLessThanOrEqual(
      listContainer.y + listContainer.height + 1,
    );
  }
});

test("Tab/Enter/Space/Escape 纯键盘主流程", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  await expect(page.getByText("中文书名")).toBeVisible();

  const remove = page.getByRole("button", { name: "从书库移除《中文书名》" });
  await remove.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("dialog", { name: "从书库移除书籍？" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();

  await remove.focus();
  await page.keyboard.press(" ");
  await expect(page.getByRole("dialog", { name: "从书库移除书籍？" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();

  await page.getByLabel("搜索书名或作者").focus();
  await page.keyboard.press("Tab");
  await expect(page.getByLabel("排序方式")).toBeFocused();
});

test("1,000 本页面重载与搜索耗时（WebKit + mock IPC，非进程冷启动）", async ({ page }) => {
  await installMock(page, THOUSAND);

  const coldStarts: number[] = [];
  for (let iteration = 0; iteration < 5; iteration += 1) {
    const start = Date.now();
    await page.goto("/");
    await expect(page.getByText("书 0000", { exact: true })).toBeVisible();
    coldStarts.push(Date.now() - start);
  }

  const searchTimes = await page.evaluate(
    async (targets: string[]) => {
      const host = globalThis as unknown as {
        document: {
          querySelector: (selector: string) => {
            value: string;
            dispatchEvent: (event: unknown) => void;
          } | null;
          querySelectorAll: (selector: string) => ArrayLike<{ textContent: string | null }>;
        };
        HTMLInputElement: { prototype: object };
        Object: {
          getOwnPropertyDescriptor: (
            target: object,
            key: string,
          ) => { set?: (value: string) => void } | undefined;
        };
        Event: new (type: string, init?: { bubbles?: boolean }) => unknown;
        performance: { now: () => number };
        requestAnimationFrame: (callback: () => void) => void;
      };
      const input = host.document.querySelector('input[aria-label="搜索书名或作者"]');
      if (!input) {
        throw new Error("missing search input");
      }
      const descriptor = host.Object.getOwnPropertyDescriptor(
        host.HTMLInputElement.prototype,
        "value",
      );
      const setValue = (value: string) => {
        descriptor?.set?.call(input, value);
        input.dispatchEvent(new host.Event("input", { bubbles: true }));
      };
      const waitForTitle = (text: string, start: number): Promise<number> =>
        new Promise((resolve) => {
          const check = () => {
            const found = Array.from(host.document.querySelectorAll("h3")).some(
              (element) => element.textContent === text,
            );
            if (found) {
              resolve(host.performance.now() - start);
              return;
            }
            host.requestAnimationFrame(check);
          };
          check();
        });
      const results: number[] = [];
      for (const target of targets) {
        const start = host.performance.now();
        setValue(target);
        results.push(await waitForTitle(target, start));
      }
      return results;
    },
    Array.from({ length: 20 }, (_, index) => (index % 2 === 0 ? "书 0999" : "书 0998")),
  );

  const sorted = [...searchTimes].sort((left, right) => left - right);
  const p95 = sorted[Math.ceil(0.95 * sorted.length) - 1] ?? Number.POSITIVE_INFINITY;
  const reporter = (globalThis as unknown as { console: { log: (...args: unknown[]) => void } })
    .console;
  reporter.log("M4D_PAGE_RELOAD_MS", JSON.stringify(coldStarts));
  reporter.log(
    "M4D_SEARCH_TIMES_MS",
    JSON.stringify(searchTimes.map((value) => Number(value.toFixed(2)))),
  );
  reporter.log("M4D_SEARCH_P95_MS", p95.toFixed(2));
  expect(Math.max(...coldStarts)).toBeLessThanOrEqual(3_000);
  expect(p95).toBeLessThanOrEqual(200);
});
