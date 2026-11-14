import { useImportFlow } from "./ImportFlowProvider";

export function ImportButton() {
  const { state, startImportFlow } = useImportFlow();
  const busy = state.phase === "selecting" || state.phase === "importing";

  return (
    <button
      type="button"
      className="import-button"
      onClick={() => void startImportFlow()}
      disabled={busy}
    >
      导入 EPUB
    </button>
  );
}
