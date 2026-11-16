import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useState } from "react";

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
import { type ReaderCommandClient, ReaderShell } from "../features/reader/ReaderShell";
import { queryKeys } from "../query/query-client";

interface AppProps {
  libraryClient?: LibraryCommandClient;
  importCommandClient?: ImportCommandClient;
  importEventSubscriber?: ImportEventSubscriber;
  readerClient?: ReaderCommandClient;
}

export default function App({
  libraryClient = tauriLibraryClient,
  importCommandClient,
  importEventSubscriber,
  readerClient,
}: AppProps = {}) {
  const queryClient = useQueryClient();
  const [activeBookId, setActiveBookId] = useState<string | null>(null);

  const handleBack = useCallback(() => {
    setActiveBookId(null);
    void queryClient.invalidateQueries({ queryKey: queryKeys.books });
  }, [queryClient]);

  return (
    <ImportFlowProvider commandClient={importCommandClient} eventSubscriber={importEventSubscriber}>
      {activeBookId ? (
        <ReaderShell bookId={activeBookId} commandClient={readerClient} onBack={handleBack} />
      ) : (
        <main className="app">
          <header className="app-header">
            <h1>Qurner</h1>
            <p className="app-subtitle">个人电子书阅读器</p>
            <div className="import-actions">
              <ImportButton />
              <ImportStatus />
            </div>
          </header>
          <LibraryPage client={libraryClient} onOpenBook={setActiveBookId} />
          <ImportSummary />
        </main>
      )}
    </ImportFlowProvider>
  );
}
