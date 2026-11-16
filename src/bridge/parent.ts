import {
  PROTOCOL_VERSION,
  parseReaderMessage,
  type ReaderLocationChangedPayload,
  type ReaderLocator,
  type ReaderNavigation,
  type ReaderPageChangedPayload,
  type ReaderSettings,
  type ReaderSpineRequestPayload,
  readerMessageSchema,
} from "./messages";

export type ParentBridgeState = "waiting" | "ready" | "error" | "closed";

export interface ParentBridgeOptions {
  sessionId: string;
  sessionToken: string;
  timeoutMs?: number;
  settings: ReaderSettings;
  spineIndex: number;
  initialLocator?: ReaderLocator | null;
  initialNavigation?: ReaderNavigation;
  onReady?: (engineVersion: string) => void;
  onError?: (code: string, message: string) => void;
  onLocationChanged?: (payload: ReaderLocationChangedPayload) => void;
  onPageChanged?: (payload: ReaderPageChangedPayload) => void;
  onSpineRequest?: (payload: ReaderSpineRequestPayload) => void;
}

const DEFAULT_TIMEOUT_MS = 5_000;

/** 父窗口侧的阅读消息桥。 */
export class ParentBridge {
  private state: ParentBridgeState = "waiting";
  private timeout: ReturnType<typeof setTimeout> | null = null;
  private readonly iframe: HTMLIFrameElement;
  private readonly options: Required<
    Pick<ParentBridgeOptions, "sessionId" | "sessionToken" | "timeoutMs">
  > &
    ParentBridgeOptions;
  private readonly listener: (event: MessageEvent) => void;

  constructor(iframe: HTMLIFrameElement, options: ParentBridgeOptions) {
    this.iframe = iframe;
    this.options = {
      timeoutMs: options.timeoutMs ?? DEFAULT_TIMEOUT_MS,
      ...options,
    };
    this.listener = (event) => this.handleMessage(event);
    window.addEventListener("message", this.listener);
    this.timeout = setTimeout(() => {
      if (this.state === "waiting") {
        this.fail("E_HANDSHAKE_TIMEOUT", "阅读会话握手超时。");
      }
    }, this.options.timeoutMs);
  }

  /** 向当前 iframe 发送一次 init；只在等待状态发送。 */
  start(): void {
    if (this.state !== "waiting") {
      return;
    }
    const target = this.iframe.contentWindow;
    if (!target) {
      this.fail("E_IFRAME_UNAVAILABLE", "阅读 iframe 不可用。");
      return;
    }
    try {
      const message = readerMessageSchema.parse({
        protocolVersion: PROTOCOL_VERSION,
        sessionId: this.options.sessionId,
        sessionToken: this.options.sessionToken,
        type: "reader:init",
        payload: {
          spineIndex: this.options.spineIndex,
          settings: this.options.settings,
          initialLocator: this.options.initialLocator ?? null,
          ...(this.options.initialNavigation
            ? { initialNavigation: this.options.initialNavigation }
            : {}),
        },
      });
      target.postMessage(message, "*");
    } catch {
      this.fail("E_INVALID_STATE", "阅读 iframe 初始化失败。");
    }
  }

  /** 在握手完成后发送翻页请求。 */
  turnPage(direction: "prev" | "next"): boolean {
    return this.sendReady("reader:turn-page", { direction });
  }

  /** 跳转到指定 Locator。 */
  goTo(locator: ReaderLocator): boolean {
    return this.sendReady("reader:go-to", { locator });
  }

  /** 切换阅读模式。 */
  updateSettings(settings: ReaderSettings): boolean {
    return this.sendReady("reader:update-settings", { settings });
  }

  /** 请求捕获当前 Locator；captureRequestId 用于关联迟到的响应。 */
  captureLocator(captureRequestId: number): boolean {
    return this.sendReady("reader:capture-locator", { captureRequestId });
  }

  getState(): ParentBridgeState {
    return this.state;
  }

  /** 停止监听并进入 closed；之后的 ready 不再触发回调。 */
  close(): void {
    if (this.state === "closed") {
      return;
    }
    this.state = "closed";
    window.removeEventListener("message", this.listener);
    this.clearTimeout();
  }

  private sendReady(type: string, payload: unknown): boolean {
    if (this.state !== "ready") {
      return false;
    }
    const target = this.iframe.contentWindow;
    if (!target) {
      return false;
    }
    try {
      target.postMessage(
        readerMessageSchema.parse({
          protocolVersion: PROTOCOL_VERSION,
          sessionId: this.options.sessionId,
          sessionToken: this.options.sessionToken,
          type,
          payload,
        }),
        "*",
      );
      return true;
    } catch {
      return false;
    }
  }

  private fail(code: string, message: string): void {
    if (this.state === "ready" || this.state === "closed") {
      return;
    }
    this.state = "error";
    this.clearTimeout();
    this.options.onError?.(code, message);
  }

  private clearTimeout(): void {
    if (this.timeout !== null) {
      clearTimeout(this.timeout);
      this.timeout = null;
    }
  }

  private handleMessage(event: MessageEvent): void {
    if (this.state === "closed") {
      return;
    }
    if (event.source !== this.iframe.contentWindow) {
      return;
    }
    const message = parseReaderMessage(event.data);
    if (!message) {
      return;
    }
    if (
      message.sessionId !== this.options.sessionId ||
      message.sessionToken !== this.options.sessionToken
    ) {
      return;
    }
    if (message.type === "reader:error") {
      this.options.onError?.(message.payload.code, message.payload.message);
      return;
    }
    if (message.type === "reader:ready" && this.state === "waiting") {
      this.state = "ready";
      this.clearTimeout();
      this.options.onReady?.(message.payload.engineVersion);
      return;
    }
    if (this.state !== "ready") {
      return;
    }
    switch (message.type) {
      case "reader:location-changed":
        this.options.onLocationChanged?.(message.payload);
        break;
      case "reader:page-changed":
        this.options.onPageChanged?.(message.payload);
        break;
      case "reader:spine-request":
        this.options.onSpineRequest?.(message.payload);
        break;
      default:
        break;
    }
  }
}

/** 为阅读 iframe 提供的 sandbox 设置；不得授予 same-origin 或导航能力。 */
export const READER_IFRAME_SANDBOX = "allow-scripts";
