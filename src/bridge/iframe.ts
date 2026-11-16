import type { ReaderEngine, ReaderEngineCallbacks } from "../reader-engine/engine";
import {
  isValidSessionId,
  PROTOCOL_VERSION,
  parseReaderMessage,
  type ReaderMessage,
  readerMessageSchema,
} from "./messages";

export type IframeBridgeState = "waiting" | "ready" | "error" | "closed";

export interface IframeBridgeConfig {
  sessionId: string;
  sessionToken: string;
  /**
   * 预期父窗口 origin；由引擎从 document.referrer 计算。
   * 为空表示 referrer 不可用（例如 packaged no-referrer），
   * 首次来自 window.parent 的合法 init 时学习父 origin。
   */
  parentOrigin: string;
  engineVersion: string;
  timeoutMs?: number;
}

export interface IframeBridgeDeps {
  parentWindow?: Window;
  listenTarget?: Window;
  postToParent?: (message: ReaderMessage, targetOrigin: string) => void;
  onStateChange?: (state: IframeBridgeState) => void;
  engine?: ReaderEngine;
}

const DEFAULT_TIMEOUT_MS = 5_000;

/** iframe 侧的阅读消息桥；只接受来自父窗口的合法信封。 */
export class IframeBridge {
  private state: IframeBridgeState = "waiting";
  private timeout: ReturnType<typeof setTimeout> | null = null;
  private readonly config: Required<IframeBridgeConfig>;
  private readonly parentWindow: Window;
  private readonly listenTarget: Window;
  private readonly postToParent: (message: ReaderMessage, targetOrigin: string) => void;
  private readonly onStateChange?: (state: IframeBridgeState) => void;
  private readonly engine?: ReaderEngine;
  private readonly listener: (event: MessageEvent) => void;
  private parentOrigin: string;

  constructor(config: IframeBridgeConfig, deps: IframeBridgeDeps = {}) {
    if (
      !isValidSessionId(config.sessionId) ||
      !/^[0-9a-f]{32,64}$/.test(config.sessionToken) ||
      config.engineVersion.length === 0
    ) {
      throw new Error("invalid reader iframe config");
    }
    this.config = {
      timeoutMs: config.timeoutMs ?? DEFAULT_TIMEOUT_MS,
      ...config,
    };
    this.parentWindow = deps.parentWindow ?? window.parent;
    this.listenTarget = deps.listenTarget ?? window;
    this.postToParent =
      deps.postToParent ??
      ((message, targetOrigin) => {
        this.parentWindow.postMessage(message, targetOrigin);
      });
    this.onStateChange = deps.onStateChange;
    this.engine = deps.engine;
    this.parentOrigin = config.parentOrigin;
    this.listener = (event) => this.handleMessage(event);
    this.listenTarget.addEventListener("message", this.listener);
    this.timeout = setTimeout(() => {
      if (this.state === "waiting") {
        this.setState("error");
      }
    }, this.config.timeoutMs);
  }

  getState(): IframeBridgeState {
    return this.state;
  }

  close(): void {
    if (this.state === "closed") {
      return;
    }
    this.setState("closed");
    this.listenTarget.removeEventListener("message", this.listener);
    this.clearTimeout();
  }

  private setState(state: IframeBridgeState): void {
    this.state = state;
    this.onStateChange?.(state);
  }

  private clearTimeout(): void {
    if (this.timeout !== null) {
      clearTimeout(this.timeout);
      this.timeout = null;
    }
  }

  private send(type: string, payload: unknown): void {
    try {
      const message = readerMessageSchema.parse({
        protocolVersion: PROTOCOL_VERSION,
        sessionId: this.config.sessionId,
        sessionToken: this.config.sessionToken,
        type,
        payload,
      });
      this.postToParent(message, this.parentOrigin);
    } catch {
      // 校验失败不发送，避免污染既有状态。
    }
  }

  private sendError(code: string, message: string): void {
    this.send("reader:error", { code, message });
  }

  private markReady(): void {
    if (this.state !== "waiting") {
      return;
    }
    this.setState("ready");
    this.clearTimeout();
    this.send("reader:ready", { engineVersion: this.config.engineVersion });
  }

  private callbacks(): ReaderEngineCallbacks {
    return {
      ready: () => this.markReady(),
      locationChanged: (payload) => this.send("reader:location-changed", payload),
      pageChanged: (payload) => this.send("reader:page-changed", payload),
      spineRequest: (payload) => this.send("reader:spine-request", payload),
      error: (code, message) => this.sendError(code, message),
    };
  }

  private handleMessage(event: MessageEvent): void {
    if (this.state === "closed") {
      return;
    }
    if (event.source !== this.parentWindow) {
      return;
    }
    const message = parseReaderMessage(event.data);
    if (!message) {
      return;
    }
    if (
      message.sessionId !== this.config.sessionId ||
      message.sessionToken !== this.config.sessionToken
    ) {
      return;
    }
    if (this.parentOrigin.length === 0) {
      // referrer 不可用（packaged no-referrer）：只允许来自父窗口的合法 init
      // 学习父 origin；其他消息或非法 origin 一律丢弃。
      if (message.type !== "reader:init") {
        return;
      }
      if (event.origin.length === 0 || event.origin === "null") {
        return;
      }
      this.parentOrigin = event.origin;
    } else if (event.origin !== this.parentOrigin) {
      return;
    }

    if (message.type === "reader:init") {
      if (this.state !== "waiting") {
        return;
      }
      try {
        if (this.engine) {
          this.engine.start(
            message.payload.settings,
            message.payload.initialLocator,
            message.payload.initialNavigation ?? null,
            message.payload.spineIndex,
            this.callbacks(),
          );
        } else {
          this.markReady();
        }
      } catch {
        this.setState("error");
      }
      return;
    }

    if (this.state !== "ready") {
      return;
    }

    switch (message.type) {
      case "reader:turn-page":
        this.engine?.turnPage(message.payload.direction);
        break;
      case "reader:go-to":
        this.engine?.goTo(message.payload.locator);
        break;
      case "reader:update-settings":
        this.engine?.applySettings(message.payload.settings);
        break;
      case "reader:capture-locator":
        this.engine?.captureLocator(message.payload.captureRequestId);
        break;
      case "reader:ready":
      case "reader:error":
      case "reader:location-changed":
      case "reader:page-changed":
      case "reader:spine-request":
        break;
    }
  }
}
