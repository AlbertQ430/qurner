/** 书库操作错误文案；不渲染底层 message、stack 或路径。 */
export function loadErrorMessage(isRefresh: boolean): string {
  return isRefresh ? "刷新书库失败，正在显示上次结果。" : "无法加载书库，请稍后重试。";
}

export type RemoveErrorKind = "book-not-found" | "storage" | "other";

export function removeErrorKind(code: string | undefined): RemoveErrorKind {
  if (code === "E_BOOK_NOT_FOUND") {
    return "book-not-found";
  }
  if (code === "E_STORAGE") {
    return "storage";
  }
  return "other";
}

export function removeErrorMessage(code: string | undefined): string {
  switch (removeErrorKind(code)) {
    case "book-not-found":
      return "该书已不在书库，正在刷新列表。";
    case "storage":
      return "无法更新书库，请确认磁盘空间后重试。";
    default:
      return "无法从书库移除该书，请重试。";
  }
}
