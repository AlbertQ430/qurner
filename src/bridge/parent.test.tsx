import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ParentBridge } from "./parent";

const SESSION_ID = "123e4567-e89b-42d3-a456-426614174000";
const TOKEN = "b".repeat(32);

class FakeWindow {
  postMessage = vi.fn();
}

function fakeIframe(window: FakeWindow | null): HTMLIFrameElement {
  return { contentWindow: window } as unknown as HTMLIFrameElement;
}

function locator(overrides: Record<string, unknown> = {}) {
  return {
    version: 1 as const,
    spineIndex: 0,
    spineHref: "chapter1.xhtml",
    textOffset: 5,
    intraSpineProgression: 0.1,
    quote: { exact: "文本", prefix: "", suffix: "" },
    ...overrides,
  };
}

function pageState(overrides: Record<string, unknown> = {}) {
  return {
    pageIndex: 0,
    pageCount: 1,
    atStart: true,
    atEnd: false,
    ...overrides,
  };
}

function dispatch(source: FakeWindow, data: Record<string, unknown>): void {
  window.dispatchEvent(
    new MessageEvent("message", {
      data,
      source: source as unknown as WindowProxy,
    }),
  );
}

function readyMessage(): Record<string, unknown> {
  return {
    protocolVersion: 1,
    sessionId: SESSION_ID,
    sessionToken: TOKEN,
    type: "reader:ready",
    payload: { engineVersion: "0.2.0" },
  };
}

describe("parent bridge", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("sends init with mode and initial locator", () => {
    const target = new FakeWindow();
    const bridge = new ParentBridge(fakeIframe(target), {
      sessionId: SESSION_ID,
      sessionToken: TOKEN,
      spineIndex: 0,
      settings: { mode: "scrolling", fontScale: 1, theme: "day" },
      initialLocator: locator(),
    });

    bridge.start();
    expect(target.postMessage).toHaveBeenCalledTimes(1);
    const [message, origin] = target.postMessage.mock.calls[0] as [Record<string, unknown>, string];
    expect(origin).toBe("*");
    expect(message.type).toBe("reader:init");
    expect(message.payload).toEqual({
      spineIndex: 0,
      settings: { mode: "scrolling", fontScale: 1, theme: "day" },
      initialLocator: locator(),
    });

    bridge.close();
  });

  it("sends commands only after ready and stops after close", () => {
    const target = new FakeWindow();
    const bridge = new ParentBridge(fakeIframe(target), {
      sessionId: SESSION_ID,
      sessionToken: TOKEN,
      spineIndex: 0,
      settings: { mode: "paged", fontScale: 1, theme: "day" },
    });

    expect(bridge.goTo(locator())).toBe(false);
    expect(bridge.updateSettings({ mode: "paged", fontScale: 1, theme: "day" })).toBe(false);
    expect(bridge.captureLocator(1)).toBe(false);
    expect(bridge.turnPage("next")).toBe(false);
    expect(target.postMessage).not.toHaveBeenCalled();

    dispatch(target, readyMessage());
    expect(bridge.getState()).toBe("ready");

    expect(bridge.goTo(locator({ textOffset: 9 }))).toBe(true);
    expect(bridge.updateSettings({ mode: "scrolling", fontScale: 1, theme: "day" })).toBe(true);
    expect(bridge.captureLocator(1)).toBe(true);
    expect(bridge.turnPage("prev")).toBe(true);
    const types = target.postMessage.mock.calls.map((call) => (call[0] as { type: string }).type);
    expect(types).toEqual([
      "reader:go-to",
      "reader:update-settings",
      "reader:capture-locator",
      "reader:turn-page",
    ]);
    const captureMessage = target.postMessage.mock.calls.find(
      (call) => (call[0] as { type: string }).type === "reader:capture-locator",
    )?.[0] as { payload?: { captureRequestId?: number } } | undefined;
    expect(captureMessage?.payload).toEqual({ captureRequestId: 1 });

    bridge.close();
    target.postMessage.mockClear();
    expect(bridge.turnPage("next")).toBe(false);
    expect(target.postMessage).not.toHaveBeenCalled();
  });

  it("forwards location, page, spine and error messages once", () => {
    const target = new FakeWindow();
    const onLocationChanged = vi.fn();
    const onPageChanged = vi.fn();
    const onSpineRequest = vi.fn();
    const onError = vi.fn();
    const bridge = new ParentBridge(fakeIframe(target), {
      sessionId: SESSION_ID,
      sessionToken: TOKEN,
      spineIndex: 0,
      settings: { mode: "paged", fontScale: 1, theme: "day" },
      onLocationChanged,
      onPageChanged,
      onSpineRequest,
      onError,
    });
    dispatch(target, readyMessage());

    dispatch(target, {
      protocolVersion: 1,
      sessionId: SESSION_ID,
      sessionToken: TOKEN,
      type: "reader:location-changed",
      payload: {
        reason: "turn",
        locator: locator(),
        mode: "paged",
        ...pageState({ pageIndex: 1, pageCount: 3, atStart: false }),
      },
    });
    dispatch(target, {
      protocolVersion: 1,
      sessionId: SESSION_ID,
      sessionToken: TOKEN,
      type: "reader:page-changed",
      payload: pageState({ pageIndex: 1, pageCount: 3, atStart: false }),
    });
    dispatch(target, {
      protocolVersion: 1,
      sessionId: SESSION_ID,
      sessionToken: TOKEN,
      type: "reader:spine-request",
      payload: { direction: "next", locator: locator() },
    });
    dispatch(target, {
      protocolVersion: 1,
      sessionId: SESSION_ID,
      sessionToken: TOKEN,
      type: "reader:error",
      payload: { code: "E_LOCATOR_NOT_FOUND", message: "无法定位到原文本位置。" },
    });

    expect(onLocationChanged).toHaveBeenCalledTimes(1);
    expect(onPageChanged).toHaveBeenCalledTimes(1);
    expect(onSpineRequest).toHaveBeenCalledTimes(1);
    expect(onError).toHaveBeenCalledWith("E_LOCATOR_NOT_FOUND", "无法定位到原文本位置。");

    bridge.close();
  });

  it("ignores ready from wrong source/token and duplicate ready", () => {
    const target = new FakeWindow();
    const onReady = vi.fn();
    const bridge = new ParentBridge(fakeIframe(target), {
      sessionId: SESSION_ID,
      sessionToken: TOKEN,
      spineIndex: 0,
      settings: { mode: "paged", fontScale: 1, theme: "day" },
      onReady,
    });

    const stranger = new FakeWindow();
    dispatch(stranger, readyMessage());
    dispatch(target, {
      ...readyMessage(),
      sessionToken: "c".repeat(32),
    });
    expect(onReady).not.toHaveBeenCalled();

    dispatch(target, readyMessage());
    dispatch(target, readyMessage());
    expect(onReady).toHaveBeenCalledTimes(1);

    bridge.close();
  });

  it("times out and ignores messages after close", () => {
    const target = new FakeWindow();
    const onError = vi.fn();
    const bridge = new ParentBridge(fakeIframe(target), {
      sessionId: SESSION_ID,
      sessionToken: TOKEN,
      spineIndex: 0,
      settings: { mode: "paged", fontScale: 1, theme: "day" },
      onError,
      timeoutMs: 1_000,
    });

    bridge.start();
    vi.advanceTimersByTime(1_001);
    expect(bridge.getState()).toBe("error");
    expect(onError).toHaveBeenCalledWith("E_HANDSHAKE_TIMEOUT", "阅读会话握手超时。");

    bridge.close();
    dispatch(target, readyMessage());
    expect(bridge.getState()).toBe("closed");
  });
});
