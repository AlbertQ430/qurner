import { describe, expect, it } from "vitest";

import { loadErrorMessage, removeErrorKind, removeErrorMessage } from "./library-errors";

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

describe("书库错误文案", () => {
  it.each(ALL_CODES)("%s 使用固定中文说明", (code) => {
    assertSafeText(removeErrorMessage(code));
  });

  it("未知 code 与 undefined 使用固定兜底文案", () => {
    assertSafeText(removeErrorMessage("unknown"));
    assertSafeText(removeErrorMessage(undefined));
  });

  it("移除错误分类稳定", () => {
    expect(removeErrorKind("E_BOOK_NOT_FOUND")).toBe("book-not-found");
    expect(removeErrorKind("E_STORAGE")).toBe("storage");
    expect(removeErrorKind("E_INTERNAL")).toBe("other");
    expect(removeErrorKind(undefined)).toBe("other");
  });

  it("加载错误文案区分首次加载与刷新", () => {
    assertSafeText(loadErrorMessage(false));
    assertSafeText(loadErrorMessage(true));
    expect(loadErrorMessage(false)).not.toBe(loadErrorMessage(true));
  });
});
