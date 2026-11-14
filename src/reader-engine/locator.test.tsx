import { describe, expect, it } from "vitest";

import { buildTextIndex, captureQuote, locateOffset, restoreLocator } from "./locator";

function makeBody(html: string): HTMLElement {
  document.body.innerHTML = html;
  return document.body;
}

describe("text index", () => {
  it("indexes text nodes in DOM order and skips non-rendered content", () => {
    const root = makeBody(
      "<p>第一段</p><script>ignored()</script><p>第二段 <style>.x{}</style>文本</p>",
    );
    const index = buildTextIndex(root);
    expect(index.text).toBe("第一段第二段 文本");
    expect(index.entries.length).toBe(3);
    expect(index.entries[0]?.start).toBe(0);
  });

  it("normalizes CRLF and maps offsets back to source", () => {
    document.body.textContent = "a\r\nb\rc";
    const index = buildTextIndex(document.body);
    expect(index.text).toBe("a\nb\nc");
    const point = locateOffset(index, 3);
    expect(point).not.toBeNull();
    expect(point?.sourceOffset).toBe(4);
  });

  it("keeps UTF-16 offsets for emoji and Chinese", () => {
    const root = makeBody("<p>中文😀后续</p>");
    const index = buildTextIndex(root);
    expect(index.text.length).toBe(6);
    const point = locateOffset(index, 3);
    expect(point?.sourceOffset).toBe(3);
  });
});

describe("quote capture and restore", () => {
  it("captures 32 code units with prefix and suffix", () => {
    const text = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const root = makeBody(`<p>${text}</p>`);
    const index = buildTextIndex(root);
    const quote = captureQuote(index, 10);
    expect(quote.exact).toBe(text.slice(10, 42));
    expect(quote.prefix).toBe(text.slice(0, 10));
    expect(quote.suffix).toBe(text.slice(42, 74));
  });

  it("restores by exact textOffset when quote matches", () => {
    const root = makeBody("<p>章节开头，中间位置文本，结尾。</p>");
    const index = buildTextIndex(root);
    const offset = index.text.indexOf("中间");
    const quote = captureQuote(index, offset);
    const restored = restoreLocator(index, {
      textOffset: offset,
      intraSpineProgression: offset / index.text.length,
      quote,
    });
    expect(restored?.textOffset).toBe(offset);
  });

  it("falls back to quote and context when the offset is stale", () => {
    const root = makeBody("<p>开头 目标文本 中段 目标文本 结尾</p>");
    const index = buildTextIndex(root);
    const quote = captureQuote(index, index.text.indexOf("目标文本"));
    const restored = restoreLocator(index, {
      textOffset: 9_999,
      intraSpineProgression: 0.5,
      quote,
    });
    expect(restored).not.toBeNull();
    expect(restored?.textOffset).toBe(index.text.indexOf("目标文本"));
  });

  it("prefers the ambiguous match closest to intraSpineProgression", () => {
    const root = makeBody("<p>重复片段 A 重复片段 B 重复片段 C</p>");
    const index = buildTextIndex(root);
    const positions = [0, 9, 18]
      .map((from) => index.text.indexOf("重复片段", from))
      .filter((position) => position >= 0);
    const target = positions[2] ?? 0;
    const quote = captureQuote(index, target);
    const restored = restoreLocator(index, {
      textOffset: 0,
      intraSpineProgression: target / index.text.length,
      quote,
    });
    expect(restored?.textOffset).toBe(target);
  });

  it("returns null when quote does not match", () => {
    const root = makeBody("<p>完全不同的正文。</p>");
    const index = buildTextIndex(root);
    expect(
      restoreLocator(index, {
        textOffset: 0,
        intraSpineProgression: 0,
        quote: { exact: "不存在的片段", prefix: "", suffix: "" },
      }),
    ).toBeNull();
  });
});
