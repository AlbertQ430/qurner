import { useEffect } from "react";

import type { ImportItemResult, ImportStatus } from "../../bridge/ipc-schemas";
import { itemErrorMessage, STATUS_LABELS } from "./error-text";
import { useImportFlow } from "./ImportFlowProvider";

const COUNTS: Array<{ status: ImportStatus; label: string }> = [
  { status: "imported", label: "已导入" },
  { status: "reactivated", label: "已恢复" },
  { status: "duplicate", label: "重复" },
  { status: "failed", label: "失败" },
];

function itemSuffix(item: ImportItemResult): string {
  if (item.status !== "failed" || !item.error) {
    return "";
  }
  return `：${itemErrorMessage(item.error.code)}`;
}

export function ImportSummary() {
  const { state, dismissSummary } = useImportFlow();
  const open = state.phase === "completed" && state.result !== null;

  useEffect(() => {
    if (!open) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        dismissSummary();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [dismissSummary, open]);

  if (!open || !state.result) {
    return null;
  }

  return (
    <div className="import-summary-backdrop">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-summary-title"
        className="import-summary"
      >
        <h2 id="import-summary-title">导入结果</h2>
        <ul className="import-summary-counts">
          {COUNTS.map(({ status, label }) => (
            <li key={status}>
              {label} {state.result?.filter((item) => item.status === status).length}
            </li>
          ))}
        </ul>
        <ul className="import-summary-items">
          {state.result.map((item) => (
            <li
              key={`${item.sourceName}|${item.status}|${item.bookId ?? ""}|${item.error?.code ?? ""}`}
            >
              <span className="import-summary-name">{item.sourceName}</span>{" "}
              <span className="import-summary-status">{STATUS_LABELS[item.status]}</span>
              {itemSuffix(item)}
            </li>
          ))}
        </ul>
        <button type="button" onClick={dismissSummary}>
          关闭导入结果
        </button>
      </div>
    </div>
  );
}
