import type {
  ReaderLocationChangedPayload,
  ReaderLocator,
  ReaderMode,
  ReaderNavigation,
  ReaderPageChangedPayload,
  ReaderSettings,
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
    settings: ReaderSettings,
    initialLocator: ReaderLocator | null,
    initialNavigation: ReaderNavigation | null,
    logicalSpineIndex: number,
    callbacks: ReaderEngineCallbacks,
  ): void;
  goTo(locator: ReaderLocator): void;
  applySettings(settings: ReaderSettings): void;
  captureLocator(captureRequestId: number): void;
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
  private spineIndex: number;
  private readonly spineHref: string;
  private mode: ReaderMode = "paged";
  private fontScale = 1;
  private theme: ReaderSettings["theme"] = "day";
  private originalInlineBodyFontSize: string | null = null;
  private originalComputedBodyFontSize: string | null = null;
  private callbacks: ReaderEngineCallbacks | null = null;
  private pageIndex = 0;
  private pageCount = 1;
  private atStart = true;
  private atEnd = false;
  private boundarySent: "prev" | "next" | null = null;
  private index: TextIndex = { text: "", entries: [] };
  private anchor: number | null = null;
  private lastLocator: ReaderLocator | null = null;
  private keyboardInstalled = false;
  private pendingFragment: string | null = null;
  private fragmentFallback = false;
  private readyPending = false;
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
    settings: ReaderSettings,
    initialLocator: ReaderLocator | null,
    initialNavigation: ReaderNavigation | null,
    logicalSpineIndex: number,
    callbacks: ReaderEngineCallbacks,
  ): void {
    // 以 init 的逻辑 spineIndex 覆盖协议注入的 index，重复 href 时保证 Locator 正确。
    this.spineIndex = logicalSpineIndex;
    this.mode = settings.mode;
    this.fontScale = settings.fontScale;
    this.theme = settings.theme;
    this.callbacks = callbacks;
    this.installStyle();
    this.applySettingsStyle();
    this.applyLayout();
    this.updatePageState();
    if (initialLocator !== null) {
      this.restore(initialLocator);
    } else if (initialNavigation !== null) {
      this.navigateInitial(initialNavigation);
    } else {
      this.scrollToStart();
    }
    this.updatePageState();
    this.lastLocator = this.capture();
    this.observe();
    const observesFonts = this.observeFonts();
    this.installKeyboard();
    const deferred = initialNavigation?.kind === "fragment" && observesFonts;
    if (deferred) {
      // fragment 需要字体/布局稳定后再定位并 ready。
      this.readyPending = true;
    } else {
      callbacks.ready();
      this.emitLocation("init");
    }
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

  applySettings(settings: ReaderSettings): void {
    if (!this.callbacks) {
      return;
    }
    const locator = this.anchor !== null ? this.capture() : (this.lastLocator ?? this.capture());
    this.mode = settings.mode;
    this.fontScale = settings.fontScale;
    this.theme = settings.theme;
    this.applySettingsStyle();
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

  captureLocator(captureRequestId: number): void {
    if (!this.callbacks) {
      return;
    }
    this.emitLocation("capture", captureRequestId);
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

  private navigateInitial(navigation: ReaderNavigation): void {
    if (navigation.kind === "start") {
      this.scrollToStart();
      return;
    }
    if (navigation.kind === "end") {
      if (this.mode === "paged") {
        this.pageIndex = Math.max(0, this.pageCount - 1);
        this.root.documentElement.scrollLeft = this.pageIndex * this.pageStep();
      } else {
        const html = this.root.documentElement;
        html.scrollTop = html.scrollHeight;
      }
      this.anchor = null;
      return;
    }
    if (!this.positionFragment(navigation.fragment)) {
      // 找不到目标时回到章首；字体就绪后不要再按旧 Locator 恢复。
      this.scrollToStart();
      this.fragmentFallback = true;
      return;
    }
    this.pendingFragment = navigation.fragment;
  }

  private positionFragment(fragment: string): boolean {
    const element = this.findFragmentElement(fragment);
    if (!element) {
      return false;
    }
    const range = this.root.createRange();
    range.selectNodeContents(element);
    const rect = range.getBoundingClientRect();
    if (this.mode === "paged") {
      const target = this.root.documentElement.scrollLeft + rect.left;
      // 目标位于当前列时使用列索引 floor；目标在列后半时四舍五入会误跳到下一页。
      const page = Math.floor(target / this.pageStep());
      this.pageIndex = Math.max(0, Math.min(this.pageCount - 1, page));
      this.root.documentElement.scrollLeft = this.pageIndex * this.pageStep();
    } else {
      this.root.documentElement.scrollTop = this.root.documentElement.scrollTop + rect.top;
    }
    this.anchor = null;
    return true;
  }

  private findFragmentElement(fragment: string): HTMLElement | null {
    const direct = this.root.getElementById(fragment);
    if (direct) {
      return direct;
    }
    try {
      const decoded = decodeURIComponent(fragment);
      if (decoded !== fragment) {
        return this.root.getElementById(decoded);
      }
    } catch {
      return null;
    }
    return null;
  }

  private installKeyboard(): void {
    if (this.keyboardInstalled) {
      return;
    }
    this.keyboardInstalled = true;
    this.root.addEventListener("keydown", this.onKeyDown);
  }

  private readonly onKeyDown = (event: KeyboardEvent): void => {
    if (!this.callbacks || event.defaultPrevented) {
      return;
    }
    if (event.ctrlKey || event.metaKey || event.altKey) {
      return;
    }
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
      return;
    }
    const target = event.target as Element | null;
    if (
      target &&
      ((target as HTMLElement).isContentEditable ||
        target.closest("input, textarea, select, button, a[href], [contenteditable='true']"))
    ) {
      return;
    }
    event.preventDefault();
    this.turnPage(event.key === "ArrowLeft" ? "prev" : "next");
  };

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
      html { background: #f6f3ec; color: #241f19; }
      html.qurner-night { background: #17130f; color: #ede6d9; }
      html, body, body * {
        -webkit-user-select: text !important;
        user-select: text !important;
      }
    `;
    (this.root.head ?? this.root.documentElement).appendChild(style);
  }

  private applySettingsStyle(): void {
    const html = this.root.documentElement;
    const body = this.root.body;
    if (this.originalInlineBodyFontSize === null) {
      this.originalInlineBodyFontSize = body.style.fontSize;
      const view = this.root.defaultView;
      const computed =
        view && typeof view.getComputedStyle === "function"
          ? view.getComputedStyle(body).fontSize
          : "";
      this.originalComputedBodyFontSize = computed.length > 0 ? computed : null;
    }
    html.style.setProperty("--qurner-font-scale", String(this.fontScale));
    html.classList.toggle("qurner-night", this.theme === "night");
    // 字号只覆盖正文基线；缩放为 1 时恢复原书内联值（外部 CSS 字号因此生效），
    // 非 1 时以原书 computed 字号为基准缩放，图片不受影响。
    if (this.fontScale === 1) {
      body.style.fontSize = this.originalInlineBodyFontSize;
    } else {
      const base =
        this.originalComputedBodyFontSize ??
        (this.originalInlineBodyFontSize && this.originalInlineBodyFontSize.length > 0
          ? this.originalInlineBodyFontSize
          : "1em");
      body.style.fontSize = `calc(${base} * ${this.fontScale})`;
    }
    // 常见 EPUB 在 body 上声明白底黑字；主题以内联样式覆盖默认背景/正文色，
    // 元素级显式颜色仍优先于继承。
    body.style.backgroundColor = this.theme === "night" ? "#1d1a16" : "#f6f3ec";
    body.style.color = this.theme === "night" ? "#ede6d9" : "#241f19";
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

  private emitLocation(
    reason: ReaderLocationChangedPayload["reason"],
    captureRequestId?: number,
  ): void {
    const payload: ReaderLocationChangedPayload = {
      reason,
      locator: this.capture(),
      mode: this.mode,
      pageIndex: this.pageIndex,
      pageCount: this.pageCount,
      atStart: this.atStart,
      atEnd: this.atEnd,
      ...(captureRequestId === undefined ? {} : { captureRequestId }),
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

  private observeFonts(): boolean {
    const fonts = this.root.fonts;
    if (!fonts || typeof fonts.ready?.then !== "function") {
      return false;
    }
    fonts.ready
      .then(() => {
        if (!this.callbacks) {
          return;
        }
        if (this.fragmentFallback) {
          this.fragmentFallback = false;
          this.applyLayout();
          this.updatePageState();
        } else {
          this.relayout();
          if (this.pendingFragment !== null) {
            const fragment = this.pendingFragment;
            this.pendingFragment = null;
            this.positionFragment(fragment);
          }
        }
        if (this.readyPending) {
          this.readyPending = false;
          this.callbacks.ready();
          this.emitLocation("init");
        }
      })
      .catch(() => {
        if (this.readyPending) {
          this.readyPending = false;
          this.callbacks?.ready();
          this.emitLocation("init");
        }
      });
    return true;
  }
}

export { ENGINE_VERSION };
