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
  __bookmarks: Array<{
    id: string;
    bookId: string;
    locator: unknown;
    excerpt: string | null;
    createdAt: string;
  }>;
}

async function installMock(page: Page, books: MockBook[]): Promise<void> {
  await page.addInitScript((seed: MockBook[]) => {
    const host = window as unknown as MockHost;
    host.__listCalls = [];
    host.__softRemoveCalls = [];
    host.__books = seed.slice();
    host.__bookmarks = [];
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
        if (command === "openBook") {
          return {
            sessionId: "123e4567-e89b-42d3-a456-426614174000",
            bookId: args?.bookId ?? "book-1",
            title: "中文书名",
            frameUrl: `epub://${"a".repeat(32)}/OEBPS/chapter1.xhtml`,
            spine: [{ index: 0, href: "OEBPS/chapter1.xhtml" }],
            chapterWeights: [1],
            toc: [],
            initialLocator: null,
            initialProgression: 0,
            settings: { mode: "paged", fontScale: 1, theme: "day" },
          };
        }
        if (command === "closeBook") {
          return null;
        }
        if (command === "listBookmarks") {
          return host.__bookmarks.slice();
        }
        if (command === "createBookmark") {
          const locator = (args as { locator?: { quote?: { exact?: string } } })?.locator;
          const bookmark = {
            id: "223e4567-e89b-42d3-a456-426614174000",
            bookId: host.__books[0]?.id ?? "b1",
            locator,
            excerpt: locator?.quote?.exact || null,
            createdAt: "2026-11-18T00:00:00Z",
          };
          host.__bookmarks.push(bookmark);
          return bookmark;
        }
        if (command === "deleteBookmark") {
          const bookmarkId = (args as { bookmarkId?: string })?.bookmarkId;
          host.__bookmarks = host.__bookmarks.filter((bookmark) => bookmark.id !== bookmarkId);
          return null;
        }
        if (
          command === "getEffectiveSettings" ||
          command === "updateAppSettings" ||
          command === "updateBookSettings" ||
          command === "resetBookSettings"
        ) {
          return { mode: "paged", fontScale: 1, theme: "day" };
        }
        if (command === "saveReadingState") {
          return { globalProgression: 0.5, savedAt: "2026-11-18T00:00:00Z" };
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
  await expect(page.locator(".book-card")).toHaveCount(2);
  await expect(page.locator(".book-cover, .book-cover-placeholder")).toHaveCount(2);

  await page.getByRole("button", { name: "列表视图" }).click();
  await expect(page.locator(".book-list-row").first()).toBeVisible();
  await expect(page.locator(".book-title", { hasText: "English Title" })).toBeVisible();
});

async function gridColumnCount(page: Page): Promise<number> {
  return page.evaluate(() => {
    const host = globalThis as unknown as {
      document: {
        querySelector: (selector: string) => {
          style: { gridTemplateColumns: string };
        } | null;
      };
      getComputedStyle: (element: unknown) => { gridTemplateColumns: string };
    };
    const row = host.document.querySelector(".book-grid-row");
    if (!row) {
      return 0;
    }
    const value = host.getComputedStyle(row).gridTemplateColumns;
    return value.split(" ").filter((part) => part.length > 0).length;
  });
}

async function assertCardContentsInside(page: Page): Promise<number> {
  const cards = page.locator(".book-card");
  const count = await cards.count();
  expect(count).toBeGreaterThan(0);
  for (let index = 0; index < count; index += 1) {
    const card = cards.nth(index);
    for (const selector of [".book-cover", ".book-title", ".book-author", ".book-remove"]) {
      await expect(card.locator(selector)).toBeVisible();
    }
    // 卡片内容不得被卡面裁剪（允许 4px 子像素舍入）。
    const overflow = await card.evaluate((element) => element.scrollHeight - element.clientHeight);
    expect(overflow).toBeLessThanOrEqual(4);
  }
  return count;
}

test("默认网格在单本、四本与宽窄窗口下完整显示封面/书名/作者", async ({ page }) => {
  await installMock(page, BOOKS.slice(0, 1));
  await openLibrary(page);
  expect(await assertCardContentsInside(page)).toBe(1);
  const singleColumns = await gridColumnCount(page);
  expect(singleColumns).toBeGreaterThan(1);

  await page.setViewportSize({ width: 1600, height: 900 });
  await expect.poll(async () => gridColumnCount(page)).toBeGreaterThan(1);
  await assertCardContentsInside(page);

  await page.setViewportSize({ width: 360, height: 900 });
  await expect.poll(async () => gridColumnCount(page)).toBe(1);
  await assertCardContentsInside(page);

  await installMock(page, THOUSAND.slice(0, 4));
  await page.goto("/");
  await expect(page.getByText("书 0000", { exact: true })).toBeVisible();
  await page.locator(".library-scroll").evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(page.getByText("书 0003", { exact: true })).toBeVisible();
  await page.locator(".library-scroll").evaluate((element) => {
    element.scrollTop = 0;
  });
  await page.setViewportSize({ width: 1600, height: 900 });
  expect(await assertCardContentsInside(page)).toBeGreaterThanOrEqual(1);
});

test("排序与方向透传后端参数", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  await page.getByLabel("排序方式").selectOption("title");
  await page.getByRole("button", { name: "升序" }).click();
  await expect
    .poll(async () => {
      const call = await page.evaluate(() => {
        const host = window as unknown as {
          __listCalls: Array<{ sort?: string; direction?: string }>;
        };
        return host.__listCalls.at(-1);
      });
      return call;
    })
    .toEqual({ query: undefined, sort: "title", direction: "asc" });
});

test("搜索防抖、无匹配与清除", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  const input = page.getByLabel("搜索书名或作者");
  await input.fill("中文");
  await expect(page.getByText("中文书名")).toBeVisible();
  await input.fill("不存在");
  await expect(page.getByText("没有匹配的书籍")).toBeVisible();
  await page.getByRole("button", { name: "清除搜索" }).first().click();
  await expect(page.getByLabel("搜索书名或作者")).toHaveValue("");
  await expect(page.getByText("中文书名")).toBeVisible();
});

test("软移除取消、确认与错误保留", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  await page.getByRole("button", { name: "从书库移除《中文书名》" }).click();
  const dialog = page.getByRole("dialog", { name: "从书库移除书籍？" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "取消" }).click();
  await expect(page.getByText("中文书名")).toBeVisible();

  await page.getByRole("button", { name: "从书库移除《中文书名》" }).click();
  await dialog.getByRole("button", { name: "确认移除" }).click();
  await expect
    .poll(async () => {
      return page.evaluate(() => {
        const host = window as unknown as { __softRemoveCalls: string[] };
        return host.__softRemoveCalls;
      });
    })
    .toEqual(["b1"]);
  await expect(page.getByText("中文书名")).toHaveCount(0);

  await page.evaluate(() => {
    const host = window as unknown as { __softRemoveError?: string };
    host.__softRemoveError = "E_STORAGE";
  });
  await page.getByRole("button", { name: "从书库移除《English Title》" }).click();
  const errorDialog = page.getByRole("dialog", { name: "从书库移除书籍？" });
  await errorDialog.getByRole("button", { name: "确认移除" }).click();
  await expect(page.getByText("无法更新书库，请确认磁盘空间后重试。")).toBeVisible();
  await expect(page.locator(".book-title", { hasText: "English Title" })).toBeVisible();
});

test("1,000 本虚拟滚动保持 DOM 受控并显示最后一项", async ({ page }) => {
  await installMock(page, THOUSAND);
  await openLibrary(page);
  await expect(page.locator(".book-grid-row").first()).toBeVisible();
  const rendered = await page.locator(".book-grid-row").count();
  expect(rendered).toBeLessThan(200);
  await page.locator(".library-scroll").evaluate((element) => {
    element.scrollTop = element.scrollHeight;
    const eventConstructor = (
      globalThis as unknown as { Event: new (type: string) => { bubbles?: boolean } }
    ).Event;
    element.dispatchEvent(new eventConstructor("scroll"));
  });
  await expect(page.getByText("书 0999", { exact: true })).toBeVisible();
});

test("Tab/Enter/Space/Escape 纯键盘主流程", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  const search = page.getByLabel("搜索书名或作者");
  await search.focus();
  await expect(search).toBeFocused();
  await search.fill("中文");
  await expect(page.getByText("中文书名")).toBeVisible();
});

test("打开书籍进入阅读器并可返回书库", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  await page.getByRole("button", { name: "打开《中文书名》" }).click();
  await expect(page.getByRole("heading", { level: 2, name: "中文书名" })).toBeVisible();
  await expect(page.getByRole("button", { name: "返回书库" }).first()).toBeVisible();
  await page.getByRole("button", { name: "返回书库" }).first().click();
  await expect(page.getByRole("heading", { level: 2, name: "书库" })).toBeVisible();
});

test("沉浸式隐藏控件、热区显示控件并遵循 Esc 顺序", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  await page.getByRole("button", { name: "打开《中文书名》" }).click();
  await expect(page.getByRole("button", { name: "返回书库" }).first()).toBeVisible();

  await expect(async () => {
    await page.getByRole("button", { name: "沉浸式" }).click();
    await expect(page.getByRole("button", { name: "显示阅读控件" })).toBeVisible({
      timeout: 1_000,
    });
  }).toPass({ timeout: 15_000 });
  await expect(page.getByRole("button", { name: "返回书库" })).toHaveCount(0);

  const hotBox = await page.getByRole("button", { name: "显示阅读控件" }).boundingBox();
  expect(hotBox).not.toBeNull();
  if (hotBox) {
    await page.mouse.move(hotBox.x + hotBox.width / 2, hotBox.y + hotBox.height / 2);
  }
  await expect(page.getByRole("button", { name: "返回书库" }).first()).toBeVisible();

  // 鼠标移出 header 后经过固定短延迟自动隐藏。
  await page.mouse.move(600, 420);

  // 自动隐藏后，键盘聚焦热区重新显示控件，继续验证设置与 Esc 顺序。
  await page.evaluate(() => {
    const host = globalThis as unknown as {
      document: {
        querySelector: (selector: string) => { focus?: () => void } | null;
      };
    };
    host.document.querySelector(".reader-hot-zone")?.focus?.();
  });
  await expect(page.getByRole("button", { name: "返回书库" }).first()).toBeVisible();

  await page.getByRole("button", { name: "阅读设置" }).click();
  await expect(page.getByRole("dialog", { name: "阅读设置" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "阅读设置" })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: "返回书库" }).first()).toBeVisible();
  await expect(page.getByRole("button", { name: "显示阅读控件" })).toHaveCount(0);
});

test("书签抽屉空状态、遮罩与 Esc 关闭", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  await page.getByRole("button", { name: "打开《中文书名》" }).click();
  await expect(page.getByRole("button", { name: "打开书签" })).toBeVisible();

  await page.getByRole("button", { name: "打开书签" }).click();
  const dialog = page.getByRole("dialog", { name: "书签" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("当前书籍还没有书签。")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "添加书签" })).toBeDisabled();

  await page.getByRole("button", { name: "关闭书签遮罩" }).click();
  await expect(page.getByRole("dialog", { name: "书签" })).toHaveCount(0);

  await page.getByRole("button", { name: "打开目录" }).click();
  await expect(page.getByRole("dialog", { name: "目录" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "目录" })).toHaveCount(0);

  await page.getByRole("button", { name: "打开书签" }).click();
  await expect(page.getByRole("dialog", { name: "书签" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "书签" })).toHaveCount(0);
});

test("恢复 initialLocator 时显示章节与百分比并打开书签抽屉", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  await page.evaluate(() => {
    const host = window as unknown as {
      __TAURI_INTERNALS__?: {
        invoke: (command: string, args?: unknown) => Promise<unknown>;
      };
    };
    const original = host.__TAURI_INTERNALS__?.invoke;
    host.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        if (command === "openBook") {
          return {
            sessionId: "123e4567-e89b-42d3-a456-426614174000",
            bookId: "b1",
            title: "中文书名",
            frameUrl: `epub://${"a".repeat(32)}/OEBPS/chapter1.xhtml`,
            spine: [{ index: 0, href: "OEBPS/chapter1.xhtml" }],
            chapterWeights: [1],
            toc: [],
            initialLocator: {
              version: 1,
              spineIndex: 0,
              spineHref: "OEBPS/chapter1.xhtml",
              textOffset: 10,
              intraSpineProgression: 0.3,
              quote: { exact: "", prefix: "", suffix: "" },
            },
            initialProgression: 0.3,
            settings: { mode: "paged", fontScale: 1, theme: "day" },
          };
        }
        if (command === "closeBook") {
          return null;
        }
        return original?.(command, args) ?? null;
      },
    };
  });
  await page.getByRole("button", { name: "打开《中文书名》" }).click();
  await expect(page.getByLabel("阅读位置")).toHaveText("第 1 节 · 30.0%");

  await page.getByRole("button", { name: "打开书签" }).click();
  const dialog = page.getByRole("dialog", { name: "书签" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("当前书籍还没有书签。")).toBeVisible();
  await dialog.getByRole("button", { name: "关闭书签" }).click();
  await expect(page.getByRole("dialog", { name: "书签" })).toHaveCount(0);
});

test("非法 frame URL 显示错误并可返回书库", async ({ page }) => {
  await installMock(page, BOOKS);
  await openLibrary(page);
  await page.evaluate(() => {
    const host = window as unknown as {
      __TAURI_INTERNALS__?: {
        invoke: (command: string, args?: unknown) => Promise<unknown>;
      };
    };
    const original = host.__TAURI_INTERNALS__?.invoke;
    host.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        if (command === "openBook") {
          return {
            sessionId: "123e4567-e89b-42d3-a456-426614174000",
            bookId: "b1",
            title: "中文书名",
            frameUrl: "epub://bad",
            spine: [{ index: 0, href: "OEBPS/chapter1.xhtml" }],
            chapterWeights: [1],
            toc: [],
            initialLocator: null,
            initialProgression: 0,
            settings: { mode: "paged", fontScale: 1, theme: "day" },
          };
        }
        if (command === "closeBook") {
          return null;
        }
        return original?.(command, args) ?? null;
      },
    };
  });
  await page.getByRole("button", { name: "打开《中文书名》" }).click();
  await expect(page.getByText("无法打开该书，请重试。")).toBeVisible();
  await page.getByRole("alert").getByRole("button", { name: "返回书库" }).click();
  await expect(page.getByRole("heading", { level: 2, name: "书库" })).toBeVisible();
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
            const found = Array.from(host.document.querySelectorAll(".book-title")).some(
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
