import { QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import type { BookSummary } from "../../bridge/ipc-schemas";
import { createQueryClient, queryKeys } from "../../query/query-client";
import {
  type ImportCommandClient,
  type ImportEventSubscriber,
  ImportFlowProvider,
} from "../import/ImportFlowProvider";
import { LibraryPage } from "./LibraryPage";
import type { LibraryCommandClient, LibraryListInput } from "./library-client";

const BOOKS: BookSummary[] = [
  {
    id: "b1",
    title: "中文书名",
    author: "作者甲",
    coverUrl: "cover://cover-1",
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
  {
    id: "b3",
    title: "非法封面",
    author: "作者丙",
    coverUrl: "https://evil.test/cover.png",
    importedAt: "2026-11-12T00:00:00Z",
    lastOpenedAt: null,
  },
];

let currentScrollWidth = 1_000;
const scrollToMock = vi.fn();
const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;
const originalClientWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientWidth");
const originalClientHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientHeight");

const originalScrollTo = HTMLElement.prototype.scrollTo;

beforeAll(() => {
  HTMLElement.prototype.scrollTo = scrollToMock;
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(
        private readonly callback: (entries: Array<{ target: Element }>, observer: unknown) => void,
      ) {}
      observe(element: Element) {
        setTimeout(() => {
          this.callback([{ target: element }], this);
        }, 0);
      }
      unobserve() {}
      disconnect() {}
    },
  );
  Object.defineProperty(HTMLElement.prototype, "clientWidth", {
    configurable: true,
    get: () => currentScrollWidth,
  });
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get: () => 600,
  });
  Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
    configurable: true,
    value: () => ({
      width: currentScrollWidth,
      height: 600,
      top: 0,
      left: 0,
      right: currentScrollWidth,
      bottom: 600,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    }),
  });
});

afterAll(() => {
  vi.unstubAllGlobals();
  HTMLElement.prototype.scrollTo = originalScrollTo;
  Object.defineProperty(
    HTMLElement.prototype,
    "getBoundingClientRect",
    originalGetBoundingClientRect,
  );
  if (originalClientWidth) {
    Object.defineProperty(HTMLElement.prototype, "clientWidth", originalClientWidth);
  }
  if (originalClientHeight) {
    Object.defineProperty(HTMLElement.prototype, "clientHeight", originalClientHeight);
  }
});

const fakeImportCommands: ImportCommandClient = {
  selectImportFiles: async () => [],
  startImport: async () => ({ jobId: "123e4567-e89b-42d3-a456-426614174000" }),
};

const fakeImportEvents: ImportEventSubscriber = {
  subscribe: async () => () => undefined,
};

class FakeLibraryClient implements LibraryCommandClient {
  listCalls: LibraryListInput[] = [];
  responses: Array<BookSummary[] | { code: string; message: string } | Error> = [];
  softRemoveCalls: string[] = [];
  softRemoveError: unknown = null;
  softRemoveDeferred = false;
  deferredSoftResolve: (() => void) | null = null;
  deferList = false;
  pendingList: Array<{
    input: LibraryListInput;
    resolve: (books: BookSummary[]) => void;
  }> = [];

  async listBooks(input: LibraryListInput): Promise<BookSummary[]> {
    this.listCalls.push(input);
    if (this.deferList) {
      return new Promise((resolve) => {
        this.pendingList.push({ input, resolve });
      });
    }
    const next = this.responses.shift();
    if (next === undefined) {
      return [];
    }
    if (Array.isArray(next)) {
      return next;
    }
    throw next;
  }

  async softRemoveBook(bookId: string): Promise<void> {
    this.softRemoveCalls.push(bookId);
    if (this.softRemoveDeferred) {
      await new Promise<void>((resolve) => {
        this.deferredSoftResolve = resolve;
      });
      return;
    }
    if (this.softRemoveError !== null) {
      throw this.softRemoveError;
    }
  }
}

function renderLibrary(
  client: FakeLibraryClient,
  queryClient = createQueryClient(),
  onOpenBook?: (bookId: string) => void,
) {
  const view = render(
    <QueryClientProvider client={queryClient}>
      <ImportFlowProvider
        commandClient={fakeImportCommands}
        eventSubscriber={fakeImportEvents}
        queryClient={queryClient}
      >
        <LibraryPage client={client} onOpenBook={onOpenBook} />
      </ImportFlowProvider>
    </QueryClientProvider>,
  );
  return { ...view, queryClient };
}

function list(): HTMLElement {
  return screen.getByRole("list", { name: "书库书籍" });
}

function firstRow(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".book-grid-row, .book-list-row");
}

describe("library page", () => {
  it("loads with default importedAt desc and shows loading before data", async () => {
    const client = new FakeLibraryClient();
    let resolveFirst: ((books: BookSummary[]) => void) | null = null;
    client.listBooks = (input) => {
      client.listCalls.push(input);
      return new Promise((resolve) => {
        resolveFirst = resolve;
      });
    };
    renderLibrary(client);

    expect(client.listCalls[0]).toEqual({
      sort: "importedAt",
      direction: "desc",
    });
    expect(screen.getByText("正在加载书库…")).toBeInTheDocument();
    expect(screen.queryByText("书库还是空的")).not.toBeInTheDocument();

    await act(async () => {
      resolveFirst?.([]);
      await Promise.resolve();
    });
    await screen.findByText("书库还是空的");
    expect(screen.queryByText("正在加载书库…")).not.toBeInTheDocument();
  });

  it("shows only the import entry when the library is empty", async () => {
    const client = new FakeLibraryClient();
    client.responses = [[]];
    renderLibrary(client);

    await screen.findByText("书库还是空的");
    expect(screen.getByText("稍后可以通过导入添加书籍。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "导入 EPUB" })).toBeInTheDocument();
    expect(screen.queryByLabelText("搜索书名或作者")).not.toBeInTheDocument();
  });

  it("defaults to grid and switches view without refetching", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS];
    renderLibrary(client);

    await screen.findByText("中文书名");
    expect(firstRow()).not.toBeNull();
    expect(document.querySelector(".book-grid-row")).not.toBeNull();
    expect(screen.getByText("作者甲")).toBeInTheDocument();
    expect(screen.getByText("English Title")).toBeInTheDocument();

    const calls = client.listCalls.length;
    fireEvent.click(screen.getByRole("button", { name: "列表视图" }));
    await waitFor(() => {
      expect(document.querySelector(".book-list-row")).not.toBeNull();
    });
    expect(client.listCalls).toHaveLength(calls);
    expect(screen.getByText("中文书名")).toBeInTheDocument();
    expect(screen.getByText("作者甲")).toBeInTheDocument();
    expect(screen.getByAltText("《中文书名》封面")).toHaveAttribute("src", "cover://cover-1");
    expect(screen.queryByAltText("《English Title》封面")).not.toBeInTheDocument();
    expect(document.querySelectorAll(".book-cover-placeholder").length).toBeGreaterThanOrEqual(2);
  });

  it("virtualizes 1,000 books with controlled DOM in grid and list", async () => {
    const client = new FakeLibraryClient();
    const thousand = Array.from({ length: 1_000 }, (_, index) => ({
      id: `book-${index}`,
      title: `书 ${index.toString().padStart(4, "0")}`,
      author: `作者 ${index}`,
      coverUrl: null,
      importedAt: "2026-11-14T00:00:00Z",
      lastOpenedAt: null,
    }));
    client.responses = [thousand];
    renderLibrary(client);
    await screen.findByText("书 0000");

    expect(list()).toBeInTheDocument();
    const rendered = screen.getAllByRole("listitem").length;
    expect(rendered).toBeGreaterThan(0);
    expect(rendered).toBeLessThan(120);

    const scroller = document.querySelector<HTMLElement>(".library-scroll");
    expect(scroller).not.toBeNull();
    if (!scroller) {
      throw new Error("missing scroll container");
    }
    scroller.scrollTop = 1_000_000;
    fireEvent.scroll(scroller);
    await screen.findByText("书 0999");
    expect(screen.getAllByRole("listitem").length).toBeLessThan(120);

    fireEvent.click(screen.getByRole("button", { name: "列表视图" }));
    await waitFor(() => {
      expect(document.querySelector(".book-list-row")).not.toBeNull();
    });
    scroller.scrollTop = 1_000_000;
    fireEvent.scroll(scroller);
    await screen.findByText("书 0999");
    expect(screen.getAllByRole("listitem").length).toBeLessThan(120);

    currentScrollWidth = 500;
    fireEvent(window, new Event("resize"));
    await waitFor(() => {
      const row = document.querySelector<HTMLElement>(".book-list-row");
      expect(row).not.toBeNull();
    });
    expect(screen.getAllByRole("listitem").length).toBeLessThan(120);
    currentScrollWidth = 1_000;
  });

  it("resets scroll position when query parameters change", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS, BOOKS];
    renderLibrary(client);
    await screen.findByText("中文书名");
    scrollToMock.mockClear();
    fireEvent.change(screen.getByLabelText("排序方式"), {
      target: { value: "title" },
    });
    await waitFor(() => {
      expect(scrollToMock).toHaveBeenCalledWith({ top: 0 });
    });
  });

  it("opens a book from the card action without removing it", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS];
    const onOpenBook = vi.fn();
    renderLibrary(client, createQueryClient(), onOpenBook);
    await screen.findByText("中文书名");

    const openButton = screen.getByRole("button", {
      name: "打开《中文书名》",
    });
    expect(openButton.tagName).toBe("BUTTON");
    fireEvent.click(openButton);
    expect(onOpenBook).toHaveBeenCalledWith("b1");
    expect(client.softRemoveCalls).toHaveLength(0);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "从书库移除《中文书名》" }));
    await screen.findByRole("dialog");
    expect(onOpenBook).toHaveBeenCalledTimes(1);
  });

  it("debounces consecutive input and only requests the final trimmed value", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS, [], BOOKS];
    renderLibrary(client);
    await screen.findByText("中文书名");

    const search = screen.getByLabelText("搜索书名或作者");
    const requestsBeforeTyping = client.listCalls.length;

    fireEvent.change(search, { target: { value: "中" } });
    await new Promise((resolve) => {
      setTimeout(resolve, 80);
    });
    expect(client.listCalls).toHaveLength(requestsBeforeTyping);

    fireEvent.change(search, { target: { value: "中文" } });
    await new Promise((resolve) => {
      setTimeout(resolve, 80);
    });
    expect(client.listCalls).toHaveLength(requestsBeforeTyping);

    fireEvent.change(search, { target: { value: "  中文  " } });
    await waitFor(
      () => {
        expect(client.listCalls).toHaveLength(requestsBeforeTyping + 1);
      },
      { timeout: 1_000 },
    );
    expect(client.listCalls.slice(requestsBeforeTyping)).toEqual([
      { sort: "importedAt", direction: "desc", query: "中文" },
    ]);
    expect(screen.getAllByRole("button", { name: "清除搜索" }).length).toBeGreaterThan(0);

    fireEvent.click(screen.getAllByRole("button", { name: "清除搜索" })[0]);
    await waitFor(() => {
      expect(client.listCalls.at(-1)?.query).toBeUndefined();
    });
  });

  it("passes exact sort and direction parameters", async () => {
    const client = new FakeLibraryClient();
    client.responses = Array.from({ length: 8 }, () => BOOKS);
    renderLibrary(client);
    await screen.findByText("中文书名");

    fireEvent.change(screen.getByLabelText("排序方式"), {
      target: { value: "author" },
    });
    await waitFor(() => {
      expect(client.listCalls.at(-1)).toEqual({
        sort: "author",
        direction: "desc",
      });
    });

    fireEvent.click(screen.getByRole("button", { name: "升序" }));
    await waitFor(() => {
      expect(client.listCalls.at(-1)).toEqual({
        sort: "author",
        direction: "asc",
      });
    });

    fireEvent.change(screen.getByLabelText("排序方式"), {
      target: { value: "title" },
    });
    await waitFor(() => {
      expect(client.listCalls.at(-1)).toEqual({
        sort: "title",
        direction: "asc",
      });
    });

    fireEvent.click(screen.getByRole("button", { name: "降序" }));
    await waitFor(() => {
      expect(client.listCalls.at(-1)).toEqual({
        sort: "title",
        direction: "desc",
      });
    });
  });

  it("keeps previous data while new parameters load", async () => {
    const client = new FakeLibraryClient();
    client.deferList = true;
    renderLibrary(client);
    await waitFor(() => {
      expect(client.pendingList).toHaveLength(1);
    });
    await act(async () => {
      client.pendingList[0]?.resolve(BOOKS);
    });
    await screen.findByText("中文书名");

    fireEvent.change(screen.getByLabelText("排序方式"), {
      target: { value: "title" },
    });
    await waitFor(() => {
      expect(client.pendingList).toHaveLength(2);
    });
    expect(screen.getByText("中文书名")).toBeInTheDocument();
    expect(screen.getByText("English Title")).toBeInTheDocument();

    await act(async () => {
      client.pendingList[1]?.resolve([BOOKS[1] as BookSummary]);
    });
    await screen.findByText("English Title");
    expect(screen.queryByText("中文书名")).not.toBeInTheDocument();
  });

  it("does not let old requests override newer parameters", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS];
    renderLibrary(client);
    await screen.findByText("中文书名");

    client.deferList = true;
    const search = screen.getByLabelText("搜索书名或作者");
    fireEvent.change(search, { target: { value: "a" } });
    await waitFor(
      () => {
        expect(client.pendingList).toHaveLength(1);
      },
      { timeout: 1_000 },
    );
    fireEvent.change(search, { target: { value: "ab" } });
    await waitFor(
      () => {
        expect(client.pendingList).toHaveLength(2);
      },
      { timeout: 1_000 },
    );

    await act(async () => {
      client.pendingList[1]?.resolve([BOOKS[1] as BookSummary]);
    });
    await screen.findByText("English Title");

    await act(async () => {
      client.pendingList[0]?.resolve([BOOKS[2] as BookSummary]);
    });
    expect(screen.getByText("English Title")).toBeInTheDocument();
    expect(screen.queryByText("非法封面")).not.toBeInTheDocument();
  });

  it("shows a distinct no-match state and clears the search", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS, [], BOOKS];
    renderLibrary(client);
    await screen.findByText("中文书名");

    fireEvent.change(screen.getByLabelText("搜索书名或作者"), {
      target: { value: "不存在" },
    });
    await screen.findByText("没有匹配的书籍");
    expect(screen.getByText("当前搜索：不存在")).toBeInTheDocument();
    expect(screen.queryByText("书库还是空的")).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "清除搜索" }).length).toBeGreaterThan(0);

    fireEvent.click(screen.getAllByRole("button", { name: "清除搜索" })[0]);
    await screen.findByText("中文书名");
    expect(screen.queryByText("没有匹配的书籍")).not.toBeInTheDocument();
  });

  it("retries initial load failures without exposing raw details", async () => {
    const client = new FakeLibraryClient();
    client.responses = [{ code: "E_STORAGE", message: "raw /Users/reader/docs" }, []];
    renderLibrary(client);

    await screen.findByText("无法加载书库，请稍后重试。");
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("raw");
    expect(document.body.textContent).not.toContain("/Users/reader");

    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await screen.findByText("书库还是空的");
  });

  it("keeps the previous list and shows a non-blocking refresh error", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS, { code: "E_STORAGE", message: "raw" }];
    const { queryClient } = renderLibrary(client);
    await screen.findByText("中文书名");

    await act(async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.books });
    });
    await screen.findByText("刷新书库失败，正在显示上次结果。");
    expect(screen.getByText("中文书名")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("raw");
  });

  it("renders a cover only for cover:// urls and falls back on failure", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS];
    renderLibrary(client);
    await screen.findByText("中文书名");

    const cover = screen.getByAltText("《中文书名》封面");
    expect(cover).toHaveAttribute("src", "cover://cover-1");
    expect(screen.queryByAltText("《English Title》封面")).not.toBeInTheDocument();
    expect(screen.queryByAltText("《非法封面》封面")).not.toBeInTheDocument();
    expect(document.querySelectorAll(".book-cover-placeholder").length).toBeGreaterThanOrEqual(2);

    fireEvent.error(cover);
    expect(screen.queryByAltText("《中文书名》封面")).not.toBeInTheDocument();
    expect(document.querySelectorAll(".book-cover-placeholder").length).toBeGreaterThanOrEqual(3);
  });

  it("opens the removal dialog with keyboard focus and cancel/Escape do not call the command", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS];
    renderLibrary(client);
    await screen.findByText("中文书名");

    const removeButton = screen.getByRole("button", {
      name: "从书库移除《中文书名》",
    });
    fireEvent.click(removeButton);
    const dialog = await screen.findByRole("dialog", {
      name: "从书库移除书籍？",
    });
    expect(dialog).toHaveTextContent("原始文件不会被删除或修改。");
    expect(dialog).toHaveTextContent("阅读进度和书签会保留。");
    expect(dialog).toHaveTextContent("再次导入同一本书可以恢复。");
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "取消" }));

    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(client.softRemoveCalls).toHaveLength(0);
    expect(document.activeElement).toBe(removeButton);

    fireEvent.click(removeButton);
    await screen.findByRole("dialog");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(client.softRemoveCalls).toHaveLength(0);
  });

  it("confirms removal, disables while pending and removes the book", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS, BOOKS.filter((book) => book.id !== "b1")];
    client.softRemoveDeferred = true;
    renderLibrary(client);
    await screen.findByText("中文书名");

    fireEvent.click(screen.getByRole("button", { name: "从书库移除《中文书名》" }));
    await screen.findByRole("dialog");
    const confirm = screen.getByRole("button", { name: "确认移除" });
    fireEvent.click(confirm);
    expect(confirm).toBeDisabled();
    expect(screen.getByRole("button", { name: "取消" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "关闭移除对话框" })).toBeDisabled();
    expect(client.softRemoveCalls).toEqual(["b1"]);

    await act(async () => {
      client.deferredSoftResolve?.();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    expect(screen.getByText("已从书库移除《中文书名》")).toBeInTheDocument();
    expect(screen.queryByText("中文书名")).not.toBeInTheDocument();
    await waitFor(() => {
      expect(document.activeElement).toHaveAttribute("aria-label", "从书库移除《English Title》");
    });
  });

  it("does not report success when the book survives the refetch", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS, BOOKS];
    renderLibrary(client);
    await screen.findByText("中文书名");

    fireEvent.click(screen.getByRole("button", { name: "从书库移除《中文书名》" }));
    await screen.findByRole("dialog");
    fireEvent.click(screen.getByRole("button", { name: "确认移除" }));
    await screen.findByText("无法从书库移除该书，请重试。");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("中文书名")).toBeInTheDocument();
    expect(screen.queryByText("已从书库移除《中文书名》")).not.toBeInTheDocument();
  });

  it("keeps the dialog and book on storage errors without leaking messages", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS];
    client.softRemoveError = { code: "E_STORAGE", message: "raw /Users/reader" };
    const consoleError = vi.spyOn(console, "error");
    renderLibrary(client);
    await screen.findByText("中文书名");

    fireEvent.click(screen.getByRole("button", { name: "从书库移除《中文书名》" }));
    await screen.findByRole("dialog");
    fireEvent.click(screen.getByRole("button", { name: "确认移除" }));
    await screen.findByText("无法更新书库，请确认磁盘空间后重试。");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("中文书名")).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("raw");
    expect(document.body.textContent).not.toContain("/Users/reader");
    expect(JSON.stringify(consoleError.mock.calls)).not.toContain("/Users/reader");
  });

  it("refreshes when the book is already gone", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS, BOOKS];
    client.softRemoveError = { code: "E_BOOK_NOT_FOUND", message: "raw" };
    renderLibrary(client);
    await screen.findByText("中文书名");

    fireEvent.click(screen.getByRole("button", { name: "从书库移除《中文书名》" }));
    await screen.findByRole("dialog");
    fireEvent.click(screen.getByRole("button", { name: "确认移除" }));
    await screen.findByText("该书已不在书库，正在刷新列表。");
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    expect(client.listCalls.length).toBeGreaterThan(1);
  });

  it("shows a fallback removal error for unknown failures", async () => {
    const client = new FakeLibraryClient();
    client.responses = [BOOKS];
    client.softRemoveError = new Error("secret /Users/reader");
    renderLibrary(client);
    await screen.findByText("中文书名");

    fireEvent.click(screen.getByRole("button", { name: "从书库移除《中文书名》" }));
    await screen.findByRole("dialog");
    fireEvent.click(screen.getByRole("button", { name: "确认移除" }));
    await screen.findByText("无法从书库移除该书，请重试。");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("中文书名")).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("secret");
    expect(document.body.textContent).not.toContain("/Users/reader");
  });
});
