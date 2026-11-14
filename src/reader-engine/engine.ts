import type {
  ReaderLocationChangedPayload,
  ReaderLocator,
  ReaderMode,
  ReaderPageChangedPayload,
  ReaderSpineRequestPayload,
} from "../bridge/messages";
import {
  buildTextIndex,
  captureQuote,
  restoreLocator,
  sourceOffsetToNormalized,
  type TextIndex,
  type TextPoint,
} from "./locator";

const COLUMN_GAP_PX = 32;
const SCROLL_TURN_RATIO = 0.9;
const SCROLL_DEBOUNCE_MS = 150;
const RESIZE_DEBOUNCE_MS = 100;
const ENGINE_VERSION = "0.2.0";

export interface ReaderEngineCallbacks {
  ready(): void;
  locationChanged(payload: ReaderLocationChangedPayload): void;
  pageChanged(payload: ReaderPageChangedPayload): void;
  spineRequest(payload: ReaderSpineRequestPayload): void;
  error(code: string, message: string): void;
}

export interface ReaderEngine {
  start(
    mode: ReaderMode,
    initialLocator: ReaderLocator | null,
    callbacks: ReaderEngineCallbacks,
  ): void;
  goTo(locator: ReaderLocator): void;
  updateMode(mode: ReaderMode): void;
  captureLocator(): void;
  turnPage(direction: "prev" | "next"): void;
}

const STYLE_ID = "qurner-reader-style";

/** 在阅读 iframe 内创建基于正文 DOM 的分页/滚动引擎。 */
export function createDomReaderEngine(
  root: Document,
  spineIndex: number,
  spineHref: string,
): ReaderEngine {
  return new DomReaderEngine(root, spineIndex, spineHref);
}

class DomReaderEngine implements ReaderEngine {
  private readonly root: Document;
  private readonly spineIndex: number;
  private readonly spineHref: string;
  private mode: ReaderMode = "paged";
  private callbacks: ReaderEngineCallbacks | null = null;
  private pageIndex = 0;
  private pageCount = 1;
  private atStart = true;
  private atEnd = false;
  private boundarySent: "prev" | "next" | null = null;
  private index: TextIndex = { text: "", entries: [] };
  private anchor: number | null = null;
  private lastLocator: ReaderLocator | null = null;
  private scrollTimer: ReturnType<typeof setTimeout> | null = null;
  private resizeTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly onScroll = (): void => {
    if (this.mode !== "scrolling" || !this.callbacks) {
      return;
    }
    if (this.scrollTimer !== null) {
      clearTimeout(this.scrollTimer);
    }
    this.scrollTimer = setTimeout(() => {
      this.scrollTimer = null;
      this.anchor = null;
      this.updatePageState();
      this.emitLocation("scroll");
    }, SCROLL_DEBOUNCE_MS);
  };
  private readonly onResize = (): void => {
    if (!this.callbacks) {
      return;
    }
    if (this.resizeTimer !== null) {
      clearTimeout(this.resizeTimer);
    }
    this.resizeTimer = setTimeout(() => {
      this.resizeTimer = null;
      this.relayout();
    }, RESIZE_DEBOUNCE_MS);
  };

  constructor(root: Document, spineIndex: number, spineHref: string) {
    this.root = root;
    this.spineIndex = spineIndex;
    this.spineHref = spineHref;
  }

  start(
    mode: ReaderMode,
    initialLocator: ReaderLocator | null,
    callbacks: ReaderEngineCallbacks,
  ): void {
    this.mode = mode;
    this.callbacks = callbacks;
    this.installStyle();
    this.applyLayout();
    this.updatePageState();
    if (initialLocator !== null) {
      this.restore(initialLocator);
    } else {
      this.scrollToStart();
    }
    this.updatePageState();
    this.lastLocator = this.capture();
    this.observe();
    this.observeFonts();
    callbacks.ready();
    this.emitLocation("init");
  }

  turnPage(direction: "prev" | "next"): void {
    if (!this.callbacks) {
      return;
    }
    if (this.mode === "paged") {
      this.turnPaged(direction);
      return;
    }
    const html = this.root.documentElement;
    const atStart = html.scrollTop <= 0;
    const atEnd = html.scrollTop + html.clientHeight >= html.scrollHeight - 1;
    if (direction === "next" && atEnd) {
      this.requestSpine("next");
      return;
    }
    if (direction === "prev" && atStart) {
      this.requestSpine("prev");
      return;
    }
    const delta = html.clientHeight * SCROLL_TURN_RATIO;
    html.scrollTop = html.scrollTop + (direction === "next" ? delta : -delta);
    this.anchor = null;
    this.updatePageState();
    this.emitLocation("turn");
  }

  goTo(locator: ReaderLocator): void {
    if (!this.callbacks) {
      return;
    }
    this.updatePageState();
    this.restore(locator);
    this.updatePageState();
    this.emitLocation("go-to");
  }

  updateMode(mode: ReaderMode): void {
    if (!this.callbacks) {
      return;
    }
    const locator = this.anchor !== null ? this.capture() : (this.lastLocator ?? this.capture());
    this.mode = mode;
    this.applyLayout();
    this.updatePageState();
    const restored = this.restore(locator);
    this.updatePageState();
    if (restored) {
      this.emitLocationWith("layout", locator);
    } else {
      this.emitLocation("layout");
    }
  }

  captureLocator(): void {
    if (!this.callbacks) {
      return;
    }
    this.emitLocation("capture");
  }

  private turnPaged(direction: "prev" | "next"): void {
    this.updatePageState();
    const current = this.currentPageIndex();
    if (direction === "next" && current >= this.pageCount - 1) {
      this.requestSpine("next");
      return;
    }
    if (direction === "prev" && current <= 0) {
      this.requestSpine("prev");
      return;
    }
    const next = Math.max(
      0,
      Math.min(this.pageCount - 1, current + (direction === "next" ? 1 : -1)),
    );
    this.pageIndex = next;
    this.anchor = null;
    this.root.documentElement.scrollLeft = next * this.pageStep();
    this.updatePageState();
    this.emitPage();
    this.emitLocation("turn");
  }

  private requestSpine(direction: "prev" | "next"): void {
    if (this.boundarySent === direction) {
      return;
    }
    this.boundarySent = direction;
    this.callbacks?.spineRequest({ direction, locator: this.capture() });
  }

  private relayout(): void {
    const locator = this.anchor !== null ? this.capture() : (this.lastLocator ?? this.capture());
    this.applyLayout();
    this.updatePageState();
    const restored = this.restore(locator);
    this.updatePageState();
    if (restored) {
      this.emitLocationWith("layout", locator);
    } else {
      this.emitLocation("layout");
    }
  }

  private restore(locator: ReaderLocator): boolean {
    if (locator.spineIndex !== this.spineIndex || locator.spineHref !== this.spineHref) {
      this.scrollToStart();
      this.updatePageState();
      this.callbacks?.error(
        "E_LOCATOR_SPINE_MISMATCH",
        "Locator 属于其他章节，无法在当前章节恢复。",
      );
      return false;
    }
    this.index = buildTextIndex(this.root.body);
    const point = restoreLocator(this.index, locator);
    if (!point) {
      this.scrollToStart();
      this.callbacks?.error("E_LOCATOR_NOT_FOUND", "无法定位到原文本位置。");
      return false;
    }
    this.anchor = point.textOffset;
    this.scrollToPoint(point);
    return true;
  }

  private scrollToPoint(point: TextPoint): void {
    const range = this.root.createRange();
    const length = point.entry.node.length;
    range.setStart(point.entry.node, Math.min(point.sourceOffset, length));
    range.setEnd(point.entry.node, Math.min(length, point.sourceOffset + 1));
    const rect = range.getBoundingClientRect();
    if (this.mode === "paged") {
      const step = this.pageStep();
      const target = this.root.documentElement.scrollLeft + rect.left;
      const page = Math.max(0, Math.min(this.pageCount - 1, Math.round(target / step)));
      this.pageIndex = page;
      this.root.documentElement.scrollLeft = page * step;
      return;
    }
    const top = rect.top + this.root.documentElement.scrollTop;
    this.root.documentElement.scrollTop = Math.max(0, top);
  }

  private scrollToStart(): void {
    this.pageIndex = 0;
    this.anchor = null;
    this.root.documentElement.scrollLeft = 0;
    this.root.documentElement.scrollTop = 0;
  }

  private capture(): ReaderLocator {
    this.index = buildTextIndex(this.root.body);
    const anchored =
      this.anchor !== null && this.anchor <= this.index.text.length ? this.anchor : null;
    const textOffset = anchored ?? this.visibleTextPoint()?.textOffset ?? 0;
    if (this.anchor === null) {
      this.anchor = textOffset;
    }
    const quote = captureQuote(this.index, textOffset);
    const progression = this.index.text.length > 0 ? textOffset / this.index.text.length : 0;
    const locator: ReaderLocator = {
      version: 1,
      spineIndex: this.spineIndex,
      spineHref: this.spineHref,
      textOffset,
      intraSpineProgression: progression,
      quote,
    };
    this.lastLocator = locator;
    return locator;
  }

  private visibleTextPoint(): TextPoint | null {
    if (this.index.entries.length === 0) {
      return null;
    }
    const html = this.root.documentElement;
    const caretHost = this.root as Document & {
      caretRangeFromPoint?: (x: number, y: number) => Range | null;
    };
    const caret = caretHost.caretRangeFromPoint;
    if (caret) {
      const positions: Array<[number, number]> = [
        [html.clientWidth * 0.5, html.clientHeight * 0.4],
        [html.clientWidth * 0.2, html.clientHeight * 0.2],
        [html.clientWidth * 0.5, html.clientHeight * 0.2],
        [html.clientWidth * 0.2, html.clientHeight * 0.6],
        [html.clientWidth * 0.5, html.clientHeight * 0.6],
      ];
      for (const [x, y] of positions) {
        const range = caret.call(this.root, x, y);
        const node = range?.startContainer;
        if (!node || node.nodeType !== Node.TEXT_NODE) {
          continue;
        }
        const textNode = node as Text;
        const entry = this.index.entries.find((candidate) => candidate.node === textNode);
        if (!entry) {
          continue;
        }
        const sourceOffset = Math.min(range?.startOffset ?? 0, textNode.length);
        return {
          entry,
          sourceOffset,
          textOffset: entry.start + sourceOffsetToNormalized(entry, sourceOffset),
        };
      }
    }

    // 回退：取垂直与当前横向栏都可见的文本节点起点。
    const left = html.scrollLeft;
    const right = left + html.clientWidth;
    const height = html.clientHeight;
    for (const entry of this.index.entries) {
      const range = this.root.createRange();
      range.selectNodeContents(entry.node);
      for (const rect of range.getClientRects()) {
        if (rect.bottom > 0 && rect.top < height && rect.right > left && rect.left < right) {
          return {
            entry,
            sourceOffset: 0,
            textOffset: entry.start,
          };
        }
      }
    }
    return null;
  }

  private installStyle(): void {
    if (this.root.getElementById(STYLE_ID)) {
      return;
    }
    const style = this.root.createElement("style");
    style.id = STYLE_ID;
    style.textContent = `
      html.qurner-paged { overflow-x: auto; overflow-y: hidden; }
      html.qurner-paged body {
        margin: 0;
        padding: 0;
        column-width: var(--qurner-page-width, 100vw);
        column-gap: ${COLUMN_GAP_PX}px;
        column-fill: auto;
        height: var(--qurner-page-height, 100vh);
      }
      html.qurner-scrolling body { margin: 0; padding: 0; }
    `;
    (this.root.head ?? this.root.documentElement).appendChild(style);
  }

  private applyLayout(): void {
    const html = this.root.documentElement;
    html.classList.toggle("qurner-paged", this.mode === "paged");
    html.classList.toggle("qurner-scrolling", this.mode === "scrolling");
    html.style.setProperty("--qurner-page-width", `${html.clientWidth}px`);
    html.style.setProperty("--qurner-page-height", `${html.clientHeight}px`);
  }

  private pageStep(): number {
    return this.root.documentElement.clientWidth + COLUMN_GAP_PX;
  }

  private currentPageIndex(): number {
    const step = this.pageStep();
    if (step <= 0) {
      return 0;
    }
    return Math.max(
      0,
      Math.min(
        Math.max(0, this.pageCount - 1),
        Math.round(this.root.documentElement.scrollLeft / step),
      ),
    );
  }

  private updatePageState(): void {
    const html = this.root.documentElement;
    if (this.mode === "paged") {
      const maxScroll = Math.max(0, html.scrollWidth - html.clientWidth);
      this.pageCount = maxScroll <= 0 ? 1 : Math.round(maxScroll / this.pageStep()) + 1;
      this.pageIndex = this.currentPageIndex();
      this.atStart = this.pageIndex <= 0;
      this.atEnd = this.pageIndex >= this.pageCount - 1;
    } else {
      this.pageIndex = 0;
      this.pageCount = 0;
      this.atStart = html.scrollTop <= 0;
      this.atEnd = html.scrollTop + html.clientHeight >= html.scrollHeight - 1;
    }
    if (!this.atStart && !this.atEnd) {
      this.boundarySent = null;
    }
    if (this.atStart && this.boundarySent === "prev") {
      // 仍在顶部：保持边界去重。
    } else if (this.atEnd && this.boundarySent === "next") {
      // 仍在底部：保持边界去重。
    } else if ((!this.atStart && !this.atEnd) || this.boundarySent === null) {
      // 离开边界后允许再次发送。
    }
  }

  private emitLocationWith(
    reason: ReaderLocationChangedPayload["reason"],
    locator: ReaderLocator,
  ): void {
    const payload: ReaderLocationChangedPayload = {
      reason,
      locator,
      mode: this.mode,
      pageIndex: this.pageIndex,
      pageCount: this.pageCount,
      atStart: this.atStart,
      atEnd: this.atEnd,
    };
    this.callbacks?.locationChanged(payload);
  }

  private emitLocation(reason: ReaderLocationChangedPayload["reason"]): void {
    const payload: ReaderLocationChangedPayload = {
      reason,
      locator: this.capture(),
      mode: this.mode,
      pageIndex: this.pageIndex,
      pageCount: this.pageCount,
      atStart: this.atStart,
      atEnd: this.atEnd,
    };
    this.callbacks?.locationChanged(payload);
  }

  private emitPage(): void {
    this.callbacks?.pageChanged({
      pageIndex: this.pageIndex,
      pageCount: this.pageCount,
      atStart: this.atStart,
      atEnd: this.atEnd,
    });
  }

  private observe(): void {
    this.root.defaultView?.addEventListener("scroll", this.onScroll);
    this.root.defaultView?.addEventListener("resize", this.onResize);
    if (typeof ResizeObserver !== "undefined") {
      const observer = new ResizeObserver(() => this.onResize());
      observer.observe(this.root.documentElement);
    }
  }

  private observeFonts(): void {
    const fonts = this.root.fonts;
    if (!fonts || typeof fonts.ready?.then !== "function") {
      return;
    }
    fonts.ready
      .then(() => {
        if (this.callbacks) {
          this.relayout();
        }
      })
      .catch(() => {
        // 字体加载失败时保持当前布局。
      });
  }
}

export { ENGINE_VERSION };
