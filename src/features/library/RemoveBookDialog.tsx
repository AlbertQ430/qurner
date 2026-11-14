import { useEffect, useRef } from "react";

import type { BookSummary } from "../../bridge/ipc-schemas";
import { removeErrorMessage } from "./library-errors";

interface RemoveBookDialogProps {
  book: BookSummary;
  pending: boolean;
  errorCode: string | undefined;
  onConfirm: () => void;
  onCancel: () => void;
}

export function RemoveBookDialog({
  book,
  pending,
  errorCode,
  onConfirm,
  onCancel,
}: RemoveBookDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !pending) {
        onCancel();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onCancel, pending]);

  return (
    <div className="remove-book-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="remove-book-title"
        className="remove-book-dialog"
      >
        <h2 id="remove-book-title">从书库移除书籍？</h2>
        <p>要移除《{book.title}》吗？</p>
        <ul>
          <li>原始文件不会被删除或修改。</li>
          <li>阅读进度和书签会保留。</li>
          <li>再次导入同一本书可以恢复。</li>
        </ul>
        {errorCode ? <p role="alert">{removeErrorMessage(errorCode)}</p> : null}
        <div className="remove-book-actions">
          <button ref={cancelRef} type="button" onClick={onCancel} disabled={pending}>
            取消
          </button>
          <button type="button" onClick={onConfirm} disabled={pending}>
            确认移除
          </button>
          <button type="button" onClick={onCancel} disabled={pending} aria-label="关闭移除对话框">
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
