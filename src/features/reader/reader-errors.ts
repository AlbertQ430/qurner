/** 打开书籍失败文案；不渲染 backend message、stack 或路径。 */
export function openErrorMessage(code: string | null): string {
  switch (code) {
    case "E_BOOK_NOT_FOUND":
      return "该书已不在书库。";
    case "E_BOOK_UNAVAILABLE":
      return "原始文件已被移动、删除或无法读取。";
    case "E_SOURCE_CHANGED":
      return "文件内容与书库记录不一致，需要重新导入。";
    case "E_STORAGE":
      return "本地阅读数据无法读取。";
    default:
      return "无法打开该书，请重试。";
  }
}

/** 打开错误是否允许重试；书库/文件错误不提供无意义自动重试。 */
export function openErrorRecoverable(code: string | null): boolean {
  return code === "E_INTERNAL" || code === "E_STORAGE";
}

/** iframe/握手错误文案。 */
export function frameErrorMessage(code: string | null): string {
  if (code === "E_HANDSHAKE_TIMEOUT") {
    return "阅读内容加载超时，请重试。";
  }
  if (code === "E_INVALID_FRAME_URL") {
    return "阅读内容地址无效，请返回书库后重试。";
  }
  return "阅读内容加载失败，请重试。";
}

/** 非法 frame URL 不可通过重试修复。 */
export function frameErrorRecoverable(code: string | null): boolean {
  return code !== "E_INVALID_FRAME_URL";
}

/** 从已验证 frame URL 提取 token；失败返回 null。 */
export function extractSessionToken(frameUrl: string): string | null {
  const match = /^epub:\/\/([0-9a-f]{32,64})\/.+$/.exec(frameUrl);
  return match?.[1] ?? null;
}
