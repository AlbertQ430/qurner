import { type QueryClient, useQueryClient } from "@tanstack/react-query";
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";

import {
  commands,
  type ImportEventHandlers,
  subscribeImportEvents,
  tauriEventTransport,
  toAppError,
} from "../../bridge/commands";
import type {
  AppErrorCode,
  ImportCompletedEvent,
  ImportItemResult,
  ImportProgressEvent,
} from "../../bridge/ipc-schemas";
import { queryKeys } from "../../query/query-client";

export interface ImportCommandClient {
  selectImportFiles(): Promise<string[]>;
  startImport(paths: string[]): Promise<{ jobId: string }>;
}

export interface ImportEventSubscriber {
  subscribe(handlers: ImportEventHandlers): Promise<() => void>;
}

export type ImportPhase = "idle" | "selecting" | "importing" | "completed" | "error";

export interface ImportFlowState {
  phase: ImportPhase;
  progress: { completed: number; total: number } | null;
  result: ImportItemResult[] | null;
  errorCode: AppErrorCode | null;
}

interface ImportFlowContextValue {
  state: ImportFlowState;
  startImportFlow: () => Promise<void>;
  dismissSummary: () => void;
}

const INITIAL_STATE: ImportFlowState = {
  phase: "idle",
  progress: null,
  result: null,
  errorCode: null,
};

const defaultCommandClient: ImportCommandClient = {
  selectImportFiles: () => commands.selectImportFiles(),
  startImport: (paths) => commands.startImport(paths),
};

const defaultEventSubscriber: ImportEventSubscriber = {
  subscribe: (handlers) => subscribeImportEvents(tauriEventTransport, handlers),
};

const ImportFlowContext = createContext<ImportFlowContextValue | null>(null);

interface ImportFlowProviderProps {
  children: ReactNode;
  commandClient?: ImportCommandClient;
  eventSubscriber?: ImportEventSubscriber;
  queryClient?: QueryClient;
}

type PendingEvent =
  | { type: "progress"; event: ImportProgressEvent }
  | { type: "completed"; event: ImportCompletedEvent };

export function ImportFlowProvider({
  children,
  commandClient = defaultCommandClient,
  eventSubscriber = defaultEventSubscriber,
  queryClient,
}: ImportFlowProviderProps) {
  const contextQueryClient = useQueryClient();
  const activeQueryClient = queryClient ?? contextQueryClient;
  const [state, setState] = useState<ImportFlowState>(INITIAL_STATE);
  const runningRef = useRef(false);
  const unsubscribeRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    return () => {
      unsubscribeRef.current?.();
      unsubscribeRef.current = null;
    };
  }, []);

  const dismissSummary = useCallback(() => {
    setState(INITIAL_STATE);
  }, []);

  const startImportFlow = useCallback(async () => {
    if (runningRef.current) {
      return;
    }
    runningRef.current = true;
    setState({ phase: "selecting", progress: null, result: null, errorCode: null });

    let closed = false;
    let jobId: string | null = null;
    let unsubscribe: (() => void) | null = null;
    const pending: PendingEvent[] = [];

    const stopSubscriptions = () => {
      closed = true;
      unsubscribe?.();
      unsubscribe = null;
      unsubscribeRef.current = null;
      runningRef.current = false;
    };

    const apply = (type: "progress" | "completed", event: PendingEvent["event"]) => {
      if (closed) {
        return;
      }
      if (jobId === null) {
        pending.push({ type, event } as PendingEvent);
        return;
      }
      if (event.jobId !== jobId) {
        return;
      }
      if (type === "progress") {
        const progressEvent = event as ImportProgressEvent;
        setState((current) => ({
          ...current,
          phase: "importing",
          progress: {
            completed: progressEvent.completed,
            total: progressEvent.total,
          },
        }));
        return;
      }
      const completedEvent = event as ImportCompletedEvent;
      stopSubscriptions();
      setState({
        phase: "completed",
        progress: null,
        result: completedEvent.items,
        errorCode: null,
      });
      void activeQueryClient.invalidateQueries({ queryKey: queryKeys.books });
    };

    try {
      const paths = await commandClient.selectImportFiles();
      if (paths.length === 0) {
        stopSubscriptions();
        setState(INITIAL_STATE);
        return;
      }

      unsubscribe = await eventSubscriber.subscribe({
        onProgress: (event) => apply("progress", event),
        onCompleted: (event) => apply("completed", event),
        onError: () => {
          if (closed) {
            return;
          }
          stopSubscriptions();
          setState({
            phase: "error",
            progress: null,
            result: null,
            errorCode: "E_INTERNAL",
          });
        },
      });
      unsubscribeRef.current = unsubscribe;
      setState((current) => ({ ...current, phase: "importing" }));

      const { jobId: currentJobId } = await commandClient.startImport(paths);
      jobId = currentJobId;
      for (const item of pending.splice(0)) {
        apply(item.type, item.event);
      }
    } catch (error) {
      stopSubscriptions();
      const appError = toAppError(error);
      setState({
        phase: "error",
        progress: null,
        result: null,
        errorCode: appError.code,
      });
    }
  }, [activeQueryClient, commandClient, eventSubscriber]);

  return (
    <ImportFlowContext.Provider value={{ state, startImportFlow, dismissSummary }}>
      {children}
    </ImportFlowContext.Provider>
  );
}

export function useImportFlow(): ImportFlowContextValue {
  const context = useContext(ImportFlowContext);
  if (!context) {
    throw new Error("useImportFlow 必须在 ImportFlowProvider 内使用");
  }
  return context;
}
