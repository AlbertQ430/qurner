import type { AppErrorCode } from "../../bridge/ipc-schemas";

/** 选择器/启动流程错误文案。 */
export function startupErrorMessage(code: AppErrorCode | null): string {
  if (code === "E_INVALID_INPUT") {
    return "本次选择无效或超过 1,000 本，请重新选择。";
  }
  return "无法打开文件选择器或启动导入，请重试。";
}

const ITEM_ERROR_MESSAGES: Record<string, string> = {
  E_FILE_NOT_FOUND: "文件已被移动或删除，请重新选择。",
  E_FILE_UNREADABLE: "无法读取文件，请检查文件权限后重新选择。",
  E_EPUB_INVALID: "文件不是有效或完整的 EPUB，请检查文件。",
  E_EPUB_ENCRYPTED: "书籍受 DRM 或加密保护，Qurner 不支持。",
  E_EPUB_FIXED_LAYOUT: "固定版式 EPUB 暂不支持。",
  E_EPUB_VERTICAL: "竖排 EPUB 暂不支持。",
  E_EPUB_SCRIPT_REQUIRED: "书籍依赖脚本，Qurner 出于安全原因不支持。",
  E_SOURCE_CHANGED: "导入期间文件发生变化，请重新选择。",
  E_STORAGE: "无法保存书库数据，请确认磁盘空间后重试。",
};

/** 逐项失败原因；未知 code 使用固定兜底文案。 */
export function itemErrorMessage(code: string): string {
  return ITEM_ERROR_MESSAGES[code] ?? "无法导入该文件，请重试。";
}

export const STATUS_LABELS: Record<string, string> = {
  imported: "已导入",
  reactivated: "已恢复",
  duplicate: "重复",
  failed: "失败",
};
