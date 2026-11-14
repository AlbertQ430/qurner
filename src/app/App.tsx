import { ImportButton } from "../features/import/ImportButton";
import {
  type ImportCommandClient,
  type ImportEventSubscriber,
  ImportFlowProvider,
} from "../features/import/ImportFlowProvider";
import { ImportStatus } from "../features/import/ImportStatus";
import { ImportSummary } from "../features/import/ImportSummary";
import { LibraryPage } from "../features/library/LibraryPage";
import { type LibraryCommandClient, tauriLibraryClient } from "../features/library/library-client";

interface AppProps {
  libraryClient?: LibraryCommandClient;
  importCommandClient?: ImportCommandClient;
  importEventSubscriber?: ImportEventSubscriber;
}

export default function App({
  libraryClient = tauriLibraryClient,
  importCommandClient,
  importEventSubscriber,
}: AppProps = {}) {
  return (
    <ImportFlowProvider commandClient={importCommandClient} eventSubscriber={importEventSubscriber}>
      <main className="app">
        <header className="app-header">
          <h1>Qurner</h1>
          <p className="app-subtitle">个人电子书阅读器</p>
          <div className="import-actions">
            <ImportButton />
            <ImportStatus />
          </div>
        </header>
        <LibraryPage client={libraryClient} />
        <ImportSummary />
      </main>
    </ImportFlowProvider>
  );
}
