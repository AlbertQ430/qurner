import { QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import type { ImportCommandClient } from "../features/import/ImportFlowProvider";
import type { LibraryCommandClient } from "../features/library/library-client";
import type { ReaderCommandClient } from "../features/reader/ReaderShell";
import { createQueryClient } from "../query/query-client";
import App from "./App";

const libraryClient: LibraryCommandClient = {
  listBooks: async () => [],
  softRemoveBook: async () => undefined,
};

const importCommandClient: ImportCommandClient = {
  selectImportFiles: async () => [],
  startImport: async () => ({ jobId: "123e4567-e89b-42d3-a456-426614174000" }),
};

const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;
const originalClientWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientWidth");
const originalClientHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientHeight");
const originalScrollTo = HTMLElement.prototype.scrollTo;

beforeAll(() => {
  HTMLElement.prototype.scrollTo = () => undefined;
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
    get: () => 1000,
  });
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get: () => 600,
  });
  Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
    configurable: true,
    value: () => ({
      width: 1000,
      height: 600,
      top: 0,
      left: 0,
      right: 1000,
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

const BOOK = {
  id: "book-1",
  title: "示例书",
  author: "作者",
  coverUrl: null,
  importedAt: "2026-11-15T00:00:00Z",
  lastOpenedAt: null,
};

function fakeReaderClient(calls: string[]): ReaderCommandClient {
  return {
    openBook: async (bookId) => {
      calls.push(`open:${bookId}`);
      return {
        sessionId: "123e4567-e89b-42d3-a456-426614174000",
        bookId,
        title: BOOK.title,
        frameUrl: `epub://${"a".repeat(32)}/OEBPS/chapter1.xhtml`,
        spine: [{ index: 0, href: "OEBPS/chapter1.xhtml" }],
        chapterWeights: [1],
        toc: [],
        initialLocator: null,
        initialProgression: 0,
        settings: { mode: "paged", fontScale: 1, theme: "day" },
      };
    },
    closeBook: async (sessionId) => {
      calls.push(`close:${sessionId}`);
    },
    getEffectiveSettings: async () => ({
      mode: "paged",
      fontScale: 1,
      theme: "day",
    }),
    updateAppSettings: async (patch) => ({
      mode: "paged",
      fontScale: 1,
      theme: "day",
      ...patch,
    }),
    updateBookSettings: async (_bookId, _patch) => ({
      mode: "paged",
      fontScale: 1,
      theme: "day",
    }),
    resetBookSettings: async () => ({
      mode: "paged",
      fontScale: 1,
      theme: "day",
    }),
    saveReadingState: async () => ({
      globalProgression: 0.5,
      savedAt: "2026-11-17T00:00:00Z",
    }),
    listBookmarks: async () => [],
    createBookmark: async () => ({
      id: "123e4567-e89b-42d3-a456-426614174000",
      bookId: "book-1",
      locator: {
        version: 1,
        spineIndex: 0,
        spineHref: "OEBPS/chapter1.xhtml",
        textOffset: 0,
        intraSpineProgression: 0,
        quote: { exact: "", prefix: "", suffix: "" },
      },
      excerpt: null,
      createdAt: "2026-11-18T00:00:00Z",
    }),
    deleteBookmark: async () => undefined,
  };
}

describe("App", () => {
  it("从书库进入阅读器并返回书库", async () => {
    const calls: string[] = [];
    let listCalls = 0;
    const withBooks: LibraryCommandClient = {
      listBooks: async () => {
        listCalls += 1;
        return [BOOK];
      },
      softRemoveBook: async () => undefined,
    };
    render(
      <QueryClientProvider client={createQueryClient()}>
        <App
          libraryClient={withBooks}
          importCommandClient={importCommandClient}
          readerClient={fakeReaderClient(calls)}
        />
      </QueryClientProvider>,
    );

    await waitFor(() => {
      expect(screen.getByText("示例书")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "打开《示例书》" }));
    await waitFor(() => {
      expect(screen.getByRole("heading", { level: 2, name: "示例书" })).toBeInTheDocument();
    });
    expect(calls).toEqual(["open:book-1"]);

    fireEvent.click(screen.getByRole("button", { name: "返回书库" }));
    await waitFor(() => {
      expect(screen.getByRole("heading", { level: 2, name: "书库" })).toBeInTheDocument();
    });
    expect(calls).toEqual(["open:book-1", "close:123e4567-e89b-42d3-a456-426614174000"]);
    expect(listCalls).toBeGreaterThan(1);
  });

  it("渲染 Qurner 标题、书库区域与导入入口", async () => {
    render(
      <QueryClientProvider client={createQueryClient()}>
        <App libraryClient={libraryClient} importCommandClient={importCommandClient} />
      </QueryClientProvider>,
    );

    expect(screen.getByRole("heading", { level: 1, name: "Qurner" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "书库" })).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText("书库还是空的")).toBeInTheDocument();
    });
    expect(screen.getByText("稍后可以通过导入添加书籍。")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "导入 EPUB" })).toHaveLength(2);
  });
});
