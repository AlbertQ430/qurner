import { describe, expect, it } from "vitest";

import type { ReaderEngineCallbacks } from "./engine";
import { createDomReaderEngine } from "./engine";

function callbacks(): ReaderEngineCallbacks {
  return {
    ready: () => undefined,
    locationChanged: () => undefined,
    pageChanged: () => undefined,
    spineRequest: () => undefined,
    error: () => undefined,
  };
}

function setupDom() {
  const originalCreateRange = document.createRange.bind(document);
  document.createRange = () => {
    const range = originalCreateRange();
    Object.defineProperty(range, "getClientRects", {
      configurable: true,
      value: () => [],
    });
    Object.defineProperty(range, "getBoundingClientRect", {
      configurable: true,
      value: () => ({
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        width: 0,
        height: 0,
      }),
    });
    return range;
  };
  document.body.setAttribute(
    "style",
    "font-size: 18px; background-color: rgb(255, 255, 255); color: rgb(10, 10, 10)",
  );
  document.body.innerHTML = `
    <p id="styled" style="color: rgb(200, 30, 30); font-family: Georgia, serif;">正文</p>
    <img id="img" alt="" style="width: 120px; height: 80px" src="data:image/png;base64,iVBORw0KGgo=" />
  `;
  return document;
}

describe("engine settings", () => {
  it("does not override an external CSS body font size at scale 1", () => {
    const originalCreateRange = document.createRange.bind(document);
    document.createRange = () => {
      const range = originalCreateRange();
      Object.defineProperty(range, "getClientRects", {
        configurable: true,
        value: () => [],
      });
      Object.defineProperty(range, "getBoundingClientRect", {
        configurable: true,
        value: () => ({
          top: 0,
          left: 0,
          right: 0,
          bottom: 0,
          width: 0,
          height: 0,
        }),
      });
      return range;
    };
    document.head.innerHTML = "<style>body { font-size: 20px; }</style>";
    document.body.removeAttribute("style");
    document.body.innerHTML = "<p>正文</p>";
    const engine = createDomReaderEngine(document, 0, "chapter1.xhtml");
    engine.start({ mode: "paged", fontScale: 1, theme: "day" }, null, null, 0, callbacks());
    expect(document.body.style.fontSize).toBe("");

    engine.applySettings({ mode: "paged", fontScale: 1.5, theme: "day" });
    expect(document.body.style.fontSize.startsWith("calc(")).toBe(true);
  });

  it("echoes captureRequestId in capture location payload", () => {
    setupDom();
    const locations: Array<{
      reason: string;
      captureRequestId?: number;
    }> = [];
    const engine = createDomReaderEngine(document, 0, "chapter1.xhtml");
    engine.start({ mode: "paged", fontScale: 1, theme: "day" }, null, null, 0, {
      ...callbacks(),
      locationChanged: (payload) => {
        locations.push({
          reason: payload.reason,
          captureRequestId: payload.captureRequestId,
        });
      },
    });
    engine.captureLocator(7);
    expect(locations.at(-1)).toEqual({
      reason: "capture",
      captureRequestId: 7,
    });
    engine.captureLocator(8);
    expect(locations.at(-1)).toEqual({
      reason: "capture",
      captureRequestId: 8,
    });
  });

  it("enables native text selection through injected style", () => {
    setupDom();
    const bodyBefore = document.body.innerHTML;
    const engine = createDomReaderEngine(document, 0, "chapter1.xhtml");
    engine.start({ mode: "paged", fontScale: 1, theme: "day" }, null, null, 0, callbacks());
    const style = document.getElementById("qurner-reader-style");
    expect(style?.textContent).toContain("user-select: text");
    expect(document.body.innerHTML).toBe(bodyBefore);
  });

  it("applies font scale and theme without rewriting body or images", () => {
    setupDom();
    const engine = createDomReaderEngine(document, 0, "chapter1.xhtml");
    engine.start({ mode: "paged", fontScale: 1, theme: "day" }, null, null, 0, callbacks());
    const bodyBefore = document.body.innerHTML;
    expect(document.documentElement.style.getPropertyValue("--qurner-font-scale")).toBe("1");
    expect(document.documentElement.classList.contains("qurner-night")).toBe(false);

    engine.applySettings({
      mode: "scrolling",
      fontScale: 1.5,
      theme: "night",
    });
    expect(document.documentElement.style.getPropertyValue("--qurner-font-scale")).toBe("1.5");
    expect(document.documentElement.classList.contains("qurner-night")).toBe(true);
    expect(document.documentElement.classList.contains("qurner-paged")).toBe(false);
    expect(document.body.innerHTML).toBe(bodyBefore);
    expect(document.body.style.fontSize).toContain("27px");
    expect(document.body.style.backgroundColor).toBe("rgb(29, 26, 22)");
    expect(document.body.style.color).toBe("rgb(237, 230, 217)");
    const image = document.getElementById("img") as HTMLImageElement;
    expect(image.style.filter).toBe("");
    expect(document.body.style.fontFamily).toBe("");
  });
});
