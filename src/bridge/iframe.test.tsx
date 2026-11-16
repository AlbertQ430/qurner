import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ReaderEngine, ReaderEngineCallbacks } from "../reader-engine/engine";
import { IframeBridge } from "./iframe";

const SESSION_ID = "123e4567-e89b-42d3-a456-426614174000";
const TOKEN = "b".repeat(32);
const PARENT_ORIGIN = "http://parent.test";

class FakeParent {
  postMessage = vi.fn();
}

class FakeEngine implements ReaderEngine {
  start = vi.fn(
    (
      _settings: unknown,
      _locator: unknown,
      _navigation: unknown,
      _spineIndex: unknown,
      callbacks: ReaderEngineCallbacks,
    ) => {
      this.callbacks = callbacks;
      callbacks.ready();
    },
  );
  goTo = vi.fn();
  applySettings = vi.fn();
  captureLocator = vi.fn();
  turnPage = vi.fn();
  callbacks: ReaderEngineCallbacks | null = null;
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

function createBridge(engine?: FakeEngine) {
  const postToParent = vi.fn();
  const parent = new FakeParent();
  const bridge = new IframeBridge(
    {
      sessionId: SESSION_ID,
      sessionToken: TOKEN,
      parentOrigin: PARENT_ORIGIN,
      engineVersion: "0.2.0",
    },
    {
      parentWindow: parent as unknown as Window,
      listenTarget: window,
      postToParent,
      engine,
    },
  );
  return { bridge, postToParent, parent, engine };
}

function dispatch(source: unknown, data: unknown, origin = PARENT_ORIGIN): void {
  window.dispatchEvent(
    new MessageEvent("message", {
      data,
      source: source as unknown as WindowProxy,
      origin,
    }),
  );
}

function initMessage(
  mode = "paged",
  locatorValue: unknown = null,
  token = TOKEN,
  sessionId = SESSION_ID,
  initialNavigation?: unknown,
  spineIndex = 0,
): Record<string, unknown> {
  return {
    protocolVersion: 1,
    sessionId,
    sessionToken: token,
    type: "reader:init",
    payload: {
      spineIndex,
      settings: { mode, fontScale: 1, theme: "day" },
      initialLocator: locatorValue,
      ...(initialNavigation === undefined ? {} : { initialNavigation }),
    },
  };
}

function commandMessage(type: string, payload: Record<string, unknown>): Record<string, unknown> {
  return {
    protocolVersion: 1,
    sessionId: SESSION_ID,
    sessionToken: TOKEN,
    type,
    payload,
  };
}

describe("iframe bridge", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("learns parent origin from a valid init when referrer is unavailable", () => {
    const engine = new FakeEngine();
    const parent = new FakeParent();
    const postToParent = vi.fn();
    const bridge = new IframeBridge(
      {
        sessionId: SESSION_ID,
        sessionToken: TOKEN,
        parentOrigin: "",
        engineVersion: "0.2.0",
      },
      {
        parentWindow: parent as unknown as Window,
        listenTarget: window,
        postToParent,
        engine,
      },
    );
    dispatch(parent, initMessage("scrolling", locator()), "http://packaged-parent.test");
    expect(bridge.getState()).toBe("ready");
    expect(postToParent).toHaveBeenCalledTimes(1);
    expect(postToParent.mock.calls[0]?.[1]).toBe("http://packaged-parent.test");

    dispatch(parent, commandMessage("reader:turn-page", { direction: "next" }), "http://evil.test");
    expect(engine.turnPage).not.toHaveBeenCalled();
    dispatch(
      parent,
      commandMessage("reader:turn-page", { direction: "next" }),
      "http://packaged-parent.test",
    );
    expect(engine.turnPage).toHaveBeenCalledWith("next");
  });

  it("does not learn the parent origin from non-init or invalid messages", () => {
    const engine = new FakeEngine();
    const parent = new FakeParent();
    const postToParent = vi.fn();
    const bridge = new IframeBridge(
      {
        sessionId: SESSION_ID,
        sessionToken: TOKEN,
        parentOrigin: "",
        engineVersion: "0.2.0",
      },
      {
        parentWindow: parent as unknown as Window,
        listenTarget: window,
        postToParent,
        engine,
      },
    );
    dispatch(parent, commandMessage("reader:turn-page", { direction: "next" }), "http://evil.test");
    dispatch(parent, initMessage("paged", null, "c".repeat(32)), "http://evil.test");
    expect(bridge.getState()).toBe("waiting");
    expect(postToParent).not.toHaveBeenCalled();
    dispatch(parent, initMessage("scrolling", locator()), "http://legit-parent.test");
    expect(bridge.getState()).toBe("ready");
    expect(postToParent.mock.calls[0]?.[1]).toBe("http://legit-parent.test");
  });

  it("starts the engine and sends a single ready", () => {
    const engine = new FakeEngine();
    const { bridge, postToParent, parent } = createBridge(engine);
    dispatch(parent, initMessage("scrolling", locator()));

    expect(engine.start).toHaveBeenCalledTimes(1);
    expect(engine.start.mock.calls[0]?.[0]).toEqual({
      mode: "scrolling",
      fontScale: 1,
      theme: "day",
    });
    expect(engine.start.mock.calls[0]?.[1]).toEqual(locator());
    expect(engine.start.mock.calls[0]?.[2]).toBeNull();
    expect(engine.start.mock.calls[0]?.[3]).toBe(0);
    expect(bridge.getState()).toBe("ready");
    expect(postToParent).toHaveBeenCalledTimes(1);
    const [message, origin] = postToParent.mock.calls[0] as [Record<string, unknown>, string];
    expect(origin).toBe(PARENT_ORIGIN);
    expect(message.type).toBe("reader:ready");
    expect(message.payload).toEqual({ engineVersion: "0.2.0" });

    dispatch(parent, initMessage());
    expect(engine.start).toHaveBeenCalledTimes(1);
    expect(postToParent).toHaveBeenCalledTimes(1);

    bridge.close();
  });

  it("dispatches commands to the engine only after ready", () => {
    const engine = new FakeEngine();
    const { bridge, parent } = createBridge(engine);

    dispatch(parent, commandMessage("reader:turn-page", { direction: "next" }));
    dispatch(parent, commandMessage("reader:go-to", { locator: locator() }));
    dispatch(
      parent,
      commandMessage("reader:update-settings", {
        settings: { mode: "paged", fontScale: 1, theme: "day" },
      }),
    );
    dispatch(parent, commandMessage("reader:capture-locator", { captureRequestId: 7 }));
    expect(engine.turnPage).not.toHaveBeenCalled();
    expect(engine.goTo).not.toHaveBeenCalled();
    expect(engine.applySettings).not.toHaveBeenCalled();
    expect(engine.captureLocator).not.toHaveBeenCalled();

    dispatch(parent, initMessage());
    dispatch(parent, commandMessage("reader:turn-page", { direction: "prev" }));
    dispatch(parent, commandMessage("reader:go-to", { locator: locator({ textOffset: 9 }) }));
    dispatch(
      parent,
      commandMessage("reader:update-settings", {
        settings: { mode: "scrolling", fontScale: 1.2, theme: "night" },
      }),
    );
    dispatch(parent, commandMessage("reader:capture-locator", { captureRequestId: 7 }));

    expect(engine.captureLocator).toHaveBeenCalledWith(7);
    expect(engine.turnPage).toHaveBeenCalledWith("prev");
    expect(engine.goTo).toHaveBeenCalledWith(locator({ textOffset: 9 }));
    expect(engine.applySettings).toHaveBeenCalledWith({
      mode: "scrolling",
      fontScale: 1.2,
      theme: "night",
    });
    expect(engine.captureLocator).toHaveBeenCalledTimes(1);

    bridge.close();
  });

  it("sends engine callbacks as messages", () => {
    const engine = new FakeEngine();
    const { bridge, postToParent, parent } = createBridge(engine);
    dispatch(parent, initMessage());
    postToParent.mockClear();

    engine.callbacks?.locationChanged({
      reason: "turn",
      locator: locator(),
      mode: "paged",
      pageIndex: 1,
      pageCount: 3,
      atStart: false,
      atEnd: false,
    });
    engine.callbacks?.pageChanged({
      pageIndex: 1,
      pageCount: 3,
      atStart: false,
      atEnd: false,
    });
    engine.callbacks?.spineRequest({ direction: "next", locator: locator() });
    engine.callbacks?.error("E_LOCATOR_NOT_FOUND", "无法定位到原文本位置。");

    const types = postToParent.mock.calls.map((call) => (call[0] as { type: string }).type);
    expect(types).toEqual([
      "reader:location-changed",
      "reader:page-changed",
      "reader:spine-request",
      "reader:error",
    ]);

    bridge.close();
  });

  it("ignores wrong window, origin, token or session", () => {
    const engine = new FakeEngine();
    const { bridge, postToParent, parent } = createBridge(engine);
    const stranger = new FakeParent();

    dispatch(stranger, initMessage());
    dispatch(parent, initMessage(), "https://evil.test");
    dispatch(parent, initMessage("paged", null, "c".repeat(32)));
    dispatch(parent, initMessage("paged", null, TOKEN, "00000000-0000-4000-8000-000000000000"));

    expect(engine.start).not.toHaveBeenCalled();
    expect(bridge.getState()).toBe("waiting");
    expect(postToParent).not.toHaveBeenCalled();

    bridge.close();
  });

  it("times out without a valid init and ignores messages after close", () => {
    const engine = new FakeEngine();
    const { bridge, postToParent, parent } = createBridge(engine);
    vi.advanceTimersByTime(5_001);
    expect(bridge.getState()).toBe("error");

    bridge.close();
    dispatch(parent, initMessage());
    expect(engine.start).not.toHaveBeenCalled();
    expect(postToParent).not.toHaveBeenCalled();
    expect(bridge.getState()).toBe("closed");
  });
});
