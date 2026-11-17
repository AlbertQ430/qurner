import { describe, expect, it } from "vitest";

import {
  extractSessionToken,
  frameErrorMessage,
  frameErrorRecoverable,
  openErrorMessage,
  openErrorRecoverable,
} from "./reader-errors";

const ALL_CODES = [
  "E_INVALID_INPUT",
  "E_FILE_NOT_FOUND",
  "E_FILE_UNREADABLE",
  "E_EPUB_INVALID",
  "E_EPUB_ENCRYPTED",
  "E_EPUB_FIXED_LAYOUT",
  "E_EPUB_VERTICAL",
  "E_EPUB_SCRIPT_REQUIRED",
  "E_BOOK_NOT_FOUND",
  "E_BOOK_UNAVAILABLE",
  "E_SOURCE_CHANGED",
  "E_SESSION_INVALID",
  "E_STORAGE",
  "E_INTERNAL",
] as const;

function assertSafeText(text: string) {
  expect(text).toMatch(/[\u4e00-\u9fa5]/);
  expect(text).not.toContain("/");
  expect(text).not.toContain("http");
  expect(text).not.toContain("E_");
}

describe("打开书籍错误文案", () => {
  it.each(ALL_CODES)("%s 使用固定中文说明", (code) => {
    assertSafeText(openErrorMessage(code));
  });

  it("null 与未知 code 使用固定兜底文案", () => {
    assertSafeText(openErrorMessage(null));
    assertSafeText(openErrorMessage("unknown"));
  });

  it("只有内部/存储错误允许自动重试", () => {
    expect(openErrorRecoverable("E_INTERNAL")).toBe(true);
    expect(openErrorRecoverable("E_STORAGE")).toBe(true);
    expect(openErrorRecoverable("E_BOOK_UNAVAILABLE")).toBe(false);
    expect(openErrorRecoverable("E_SOURCE_CHANGED")).toBe(false);
    expect(openErrorRecoverable(null)).toBe(false);
  });

  it("iframe 错误固定文案且非法 URL 不重试", () => {
    assertSafeText(frameErrorMessage("E_HANDSHAKE_TIMEOUT"));
    assertSafeText(frameErrorMessage("E_INVALID_FRAME_URL"));
    assertSafeText(frameErrorMessage(null));
    expect(frameErrorRecoverable("E_INVALID_FRAME_URL")).toBe(false);
    expect(frameErrorRecoverable("E_HANDSHAKE_TIMEOUT")).toBe(true);
  });

  it("只从合法 epub 会话 URL 提取 token", () => {
    expect(extractSessionToken(`epub://${"a".repeat(32)}/OEBPS/ch.xhtml`)).toBe("a".repeat(32));
    expect(extractSessionToken("https://example.com/x")).toBeNull();
    expect(extractSessionToken("epub://short/x")).toBeNull();
  });
});
