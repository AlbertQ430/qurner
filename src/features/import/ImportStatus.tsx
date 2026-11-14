import { startupErrorMessage } from "./error-text";
import { useImportFlow } from "./ImportFlowProvider";

export function ImportStatus() {
  const { state, startImportFlow } = useImportFlow();

  if (state.phase === "selecting") {
    return (
      <p className="import-status" role="status">
        正在打开文件选择器…
      </p>
    );
  }

  if (state.phase === "importing") {
    if (!state.progress) {
      return (
        <p className="import-status" role="status">
          正在准备导入…
        </p>
      );
    }
    return (
      <div className="import-status">
        <p role="status">
          已完成 {state.progress.completed} / {state.progress.total}
        </p>
        <progress
          aria-label="导入进度"
          max={state.progress.total}
          value={state.progress.completed}
        />
      </div>
    );
  }

  if (state.phase === "error") {
    return (
      <div className="import-error" role="alert">
        <p>{startupErrorMessage(state.errorCode)}</p>
        <button type="button" onClick={() => void startImportFlow()}>
          重试
        </button>
      </div>
    );
  }

  return null;
}
