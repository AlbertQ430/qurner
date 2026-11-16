import { keepPreviousData, useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { INTERNAL_APP_ERROR, toAppError } from "../../bridge/commands";
import type { BookSort, BookSummary, SortDirection } from "../../bridge/ipc-schemas";
import { queryKeys } from "../../query/query-client";
import { BookCover } from "./BookCover";
import { EmptyLibrary } from "./EmptyLibrary";
import type { LibraryCommandClient } from "./library-client";
import { loadErrorMessage, removeErrorKind, removeErrorMessage } from "./library-errors";
import { RemoveBookDialog } from "./RemoveBookDialog";

const DEBOUNCE_MS = 150;
const LIST_ROW_HEIGHT = 118;
const GRID_GAP = 16;
const GRID_CARD_MIN_WIDTH = 180;
/** 卡面内边距 + 边框（box-sizing: border-box）。 */
const GRID_CARD_CHROME = 26;
/** 标题两行 + 作者一行 + 移除按钮 + 间距的安全高度。 */
const GRID_CARD_META_HEIGHT = 130;
/** 网格行底部间距。 */
const GRID_ROW_EXTRA = 16;

/** 由卡宽推导行高：封面 2:3 完整 + 卡面内边距/边框 + 标题作者区域。 */
export function gridRowHeight(cardWidth: number): number {
  const safeWidth = Math.max(GRID_CARD_MIN_WIDTH, cardWidth);
  return Math.ceil(
    GRID_CARD_CHROME +
      (safeWidth - GRID_CARD_CHROME) * 1.5 +
      GRID_CARD_META_HEIGHT +
      GRID_ROW_EXTRA,
  );
}

const SORT_OPTIONS: Array<{ value: BookSort; label: string }> = [
  { value: "importedAt", label: "最近导入" },
  { value: "title", label: "书名" },
  { value: "author", label: "作者" },
];

function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(timer);
  }, [value, delayMs]);
  return debounced;
}

interface LibraryPageProps {
  client: LibraryCommandClient;
  onOpenBook?: (bookId: string) => void;
}

export function LibraryPage({ client, onOpenBook }: LibraryPageProps) {
  const queryClient = useQueryClient();
  const [searchText, setSearchText] = useState("");
  const debouncedSearch = useDebouncedValue(searchText, DEBOUNCE_MS);
  const [sort, setSort] = useState<BookSort>("importedAt");
  const [direction, setDirection] = useState<SortDirection>("desc");
  const [view, setView] = useState<"grid" | "list">("grid");
  const [removingBook, setRemovingBook] = useState<BookSummary | null>(null);
  const [removePending, setRemovePending] = useState(false);
  const [removeErrorCode, setRemoveErrorCode] = useState<string | undefined>();
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [triggerElement, setTriggerElement] = useState<HTMLElement | null>(null);

  const trimmedSearch = debouncedSearch.trim();
  const input = useMemo(
    () => ({
      sort,
      direction,
      ...(trimmedSearch ? { query: trimmedSearch } : {}),
    }),
    [direction, sort, trimmedSearch],
  );

  const query = useQuery({
    queryKey: queryKeys.bookList(input),
    queryFn: () => client.listBooks(input),
    placeholderData: keepPreviousData,
  });

  const books = query.data ?? [];
  const showToolbar = searchText.trim() !== "" || trimmedSearch !== "" || books.length > 0;
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const titleRef = useRef<HTMLHeadingElement | null>(null);
  const initialWidth = typeof window === "undefined" ? 0 : Math.max(320, window.innerWidth - 64);
  const [gridWidth, setGridWidth] = useState(initialWidth);
  const [scrollHeight, setScrollHeight] = useState(0);
  const [columns, setColumns] = useState(
    Math.max(1, Math.floor((initialWidth + GRID_GAP) / (GRID_CARD_MIN_WIDTH + GRID_GAP))),
  );

  useEffect(() => {
    if (view !== "grid" || !showToolbar) {
      return;
    }
    const update = () => {
      const element = scrollRef.current;
      if (!element) {
        return;
      }
      // 首帧 clientWidth 可能为 0；用窗口宽度测量，避免退化为单列超宽卡面。
      const width = element.clientWidth > 0 ? element.clientWidth : window.innerWidth;
      if (width <= 0) {
        return;
      }
      setGridWidth(width);
      setScrollHeight(element.clientHeight);
      setColumns(Math.max(1, Math.floor((width + GRID_GAP) / (GRID_CARD_MIN_WIDTH + GRID_GAP))));
    };
    update();
    window.addEventListener("resize", update);
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(update);
    if (observer && scrollRef.current) {
      observer.observe(scrollRef.current);
    }
    return () => {
      window.removeEventListener("resize", update);
      observer?.disconnect();
    };
  }, [showToolbar, view]);

  const hasData = query.data !== undefined;
  const rows = useMemo(() => {
    if (view === "list" || columns <= 1) {
      return books.map((book) => [book]);
    }
    const grouped: BookSummary[][] = [];
    for (let index = 0; index < books.length; index += columns) {
      grouped.push(books.slice(index, index + columns));
    }
    return grouped;
  }, [books, columns, view]);

  const gridCardWidth =
    gridWidth > 0 ? (gridWidth - GRID_GAP * (columns - 1)) / columns : GRID_CARD_MIN_WIDTH;
  const widthRowHeight = gridRowHeight(gridCardWidth);
  const rowHeight =
    view === "list"
      ? LIST_ROW_HEIGHT
      : scrollHeight > 0
        ? Math.min(widthRowHeight, Math.max(LIST_ROW_HEIGHT, scrollHeight))
        : widthRowHeight;
  const coverWidth =
    view === "grid"
      ? Math.max(
          0,
          Math.min(
            gridCardWidth - GRID_CARD_CHROME,
            Math.floor(
              ((rowHeight - GRID_ROW_EXTRA - GRID_CARD_CHROME - GRID_CARD_META_HEIGHT) * 2) / 3,
            ),
          ),
        )
      : undefined;
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    overscan: 2,
    getItemKey: (index) => rows[index]?.[0]?.id ?? index,
  });

  // biome-ignore lint/correctness/useExhaustiveDependencies: 参数变化时重置滚动位置
  useEffect(() => {
    scrollRef.current?.scrollTo({ top: 0 });
  }, [direction, sort, trimmedSearch]);

  const isInitialLoading = query.isPending && !hasData;
  const isInitialError = query.isError && !hasData;
  const isRefreshError = query.isError && hasData;
  const isEmptyLibrary = hasData && books.length === 0 && trimmedSearch === "";
  const isNoMatch = hasData && books.length === 0 && trimmedSearch !== "";

  useEffect(() => {
    if (!statusMessage) {
      return;
    }
    const timer = setTimeout(() => setStatusMessage(null), 4_000);
    return () => clearTimeout(timer);
  }, [statusMessage]);

  const clearSearch = useCallback(() => {
    setSearchText("");
  }, []);

  const restoreFocus = useCallback(() => {
    if (triggerElement?.isConnected) {
      triggerElement.focus();
      return;
    }
    const nextRemove = document.querySelector<HTMLElement>(".book-remove");
    if (nextRemove) {
      nextRemove.focus();
      return;
    }
    titleRef.current?.focus();
  }, [triggerElement]);

  const openRemove = useCallback((book: BookSummary, trigger: HTMLElement) => {
    setStatusMessage(null);
    setRemoveErrorCode(undefined);
    setRemovePending(false);
    setRemovingBook(book);
    setTriggerElement(trigger);
  }, []);

  const cancelRemove = useCallback(() => {
    if (removePending) {
      return;
    }
    setRemovingBook(null);
    setRemoveErrorCode(undefined);
    restoreFocus();
  }, [removePending, restoreFocus]);

  const confirmRemove = useCallback(async () => {
    if (!removingBook || removePending) {
      return;
    }
    const book = removingBook;
    setRemovePending(true);
    setRemoveErrorCode(undefined);
    try {
      await client.softRemoveBook(book.id);
      await queryClient.invalidateQueries({ queryKey: queryKeys.books });
      // 后端必须真正持久化 removed；若重新查询仍包含该书，不得报告成功。
      const stillPresent = queryClient
        .getQueriesData<BookSummary[]>({ queryKey: queryKeys.books })
        .some(([, data]) => data?.some((candidate) => candidate.id === book.id));
      if (stillPresent) {
        throw INTERNAL_APP_ERROR;
      }
      queryClient.setQueriesData<BookSummary[]>({ queryKey: queryKeys.books }, (previous) =>
        previous?.filter((candidate) => candidate.id !== book.id),
      );
      setStatusMessage(`已从书库移除《${book.title}》`);
      setRemovingBook(null);
      setRemovePending(false);
      // 等 React 提交列表更新后再恢复焦点，避免聚焦到即将卸载的按钮。
      window.setTimeout(restoreFocus, 0);
    } catch (error) {
      const code = toAppError(error).code;
      setRemovePending(false);
      if (removeErrorKind(code) === "book-not-found") {
        setStatusMessage(removeErrorMessage(code));
        setRemovingBook(null);
        void queryClient.invalidateQueries({ queryKey: queryKeys.books });
        window.setTimeout(restoreFocus, 0);
        return;
      }
      setRemoveErrorCode(code);
    }
  }, [client, queryClient, removePending, removingBook, restoreFocus]);

  const renderBook = (
    book: BookSummary,
    layout: "grid" | "list",
    coverWidthDuringRender?: number,
  ) => (
    <div key={book.id} className={layout === "grid" ? "book-card" : "book-row"}>
      <button
        type="button"
        className="book-open"
        aria-label={`打开《${book.title}》`}
        onClick={() => onOpenBook?.(book.id)}
      >
        <BookCover book={book} coverWidth={coverWidthDuringRender} />
        <span className="book-meta">
          <span className="book-title">{book.title}</span>
          <span className="book-author">{book.author}</span>
        </span>
      </button>
      <button
        type="button"
        className="book-remove"
        aria-label={`从书库移除《${book.title}》`}
        onClick={(event) => openRemove(book, event.currentTarget)}
      >
        移除
      </button>
    </div>
  );

  return (
    <section className="library" aria-labelledby="library-title">
      <h2 id="library-title" ref={titleRef} tabIndex={-1}>
        书库
      </h2>

      {isRefreshError ? (
        <div className="library-error" role="alert">
          <p>{loadErrorMessage(true)}</p>
          <button type="button" onClick={() => void query.refetch()}>
            重试
          </button>
        </div>
      ) : null}

      {isInitialLoading ? (
        <p className="library-loading" role="status">
          正在加载书库…
        </p>
      ) : null}

      {isInitialError ? (
        <div className="library-error" role="alert">
          <p>{loadErrorMessage(false)}</p>
          <button type="button" onClick={() => void query.refetch()}>
            重试
          </button>
        </div>
      ) : null}

      {isEmptyLibrary ? <EmptyLibrary /> : null}

      {showToolbar ? (
        <div className="library-toolbar">
          <input
            type="search"
            className="library-search"
            aria-label="搜索书名或作者"
            placeholder="搜索书名或作者"
            value={searchText}
            onChange={(event) => setSearchText(event.target.value)}
          />
          {searchText !== "" ? (
            <button type="button" onClick={clearSearch}>
              清除搜索
            </button>
          ) : null}
          <label className="library-sort">
            排序方式
            <select value={sort} onChange={(event) => setSort(event.target.value as BookSort)}>
              {SORT_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <fieldset className="library-direction">
            <legend className="visually-hidden">排序方向</legend>
            <button
              type="button"
              aria-pressed={direction === "asc"}
              onClick={() => setDirection("asc")}
            >
              升序
            </button>
            <button
              type="button"
              aria-pressed={direction === "desc"}
              onClick={() => setDirection("desc")}
            >
              降序
            </button>
          </fieldset>
          <fieldset className="library-view">
            <legend className="visually-hidden">视图模式</legend>
            <button type="button" aria-pressed={view === "grid"} onClick={() => setView("grid")}>
              网格视图
            </button>
            <button type="button" aria-pressed={view === "list"} onClick={() => setView("list")}>
              列表视图
            </button>
          </fieldset>
        </div>
      ) : null}

      {isNoMatch ? (
        <div className="library-no-match" role="status">
          <p>没有匹配的书籍</p>
          <p>当前搜索：{trimmedSearch}</p>
          <button type="button" onClick={clearSearch}>
            清除搜索
          </button>
        </div>
      ) : null}

      {books.length > 0 ? (
        // biome-ignore lint/a11y/noNoninteractiveTabindex: 虚拟滚动容器需要键盘聚焦与可见焦点
        <div ref={scrollRef} className="library-scroll" tabIndex={0}>
          <ul
            className="library-virtual"
            aria-label="书库书籍"
            style={{ height: rows.length * rowHeight }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const rowBooks = rows[virtualRow.index] ?? [];
              return (
                <li
                  key={virtualRow.key}
                  className={view === "grid" ? "book-grid-row" : "book-list-row"}
                  style={{
                    transform: `translateY(${virtualRow.start}px)`,
                    height: rowHeight,
                    gridTemplateColumns:
                      view === "grid" ? `repeat(${columns}, minmax(0, 1fr))` : undefined,
                  }}
                >
                  {rowBooks.map((book) => renderBook(book, view, coverWidth))}
                </li>
              );
            })}
          </ul>
        </div>
      ) : null}

      {statusMessage ? (
        <p className="library-status" role="status">
          {statusMessage}
        </p>
      ) : null}

      {removingBook ? (
        <RemoveBookDialog
          book={removingBook}
          pending={removePending}
          errorCode={removeErrorCode}
          onConfirm={() => void confirmRemove()}
          onCancel={cancelRemove}
        />
      ) : null}
    </section>
  );
}
