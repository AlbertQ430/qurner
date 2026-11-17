import { describe, expect, it } from "vitest";

import { itemErrorMessage, startupErrorMessage } from "./error-text";

/** SDS 9.1 的 14 个稳定错误码。 */
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
  expect(text.length).toBeGreaterThan(0);
  expect(text.length).toBeLessThan(80);
  expect(text).toMatch(/[\u4e00-\u9fa5]/);
  expect(text).not.toContain("/");
  expect(text).not.toContain("\\");
  expect(text).not.toContain("http");
  expect(text).not.toContain("E_");
  expect(text).not.toContain("sqlite");
  expect(text).not.toContain("rusqlite");
}

describe("导入错误文案", () => {
  it.each(ALL_CODES)("%s 使用固定中文说明且不泄漏路径/内部信息", (code) => {
    assertSafeText(itemErrorMessage(code));
  });

  it("未知 code 使用固定兜底文案", () => {
    assertSafeText(itemErrorMessage("E_NOT_A_REAL_CODE"));
    assertSafeText(itemErrorMessage(""));
  });

  it("启动错误对无效选择与其它失败给出不同固定文案", () => {
    assertSafeText(startupErrorMessage("E_INVALID_INPUT"));
    assertSafeText(startupErrorMessage("E_STORAGE"));
    assertSafeText(startupErrorMessage(null));
    expect(startupErrorMessage("E_INVALID_INPUT")).not.toBe(startupErrorMessage("E_STORAGE"));
  });
});
