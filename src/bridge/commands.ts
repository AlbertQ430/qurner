import { invoke } from "@tauri-apps/api/core";
import { listen, type Event as TauriEvent, type UnlistenFn } from "@tauri-apps/api/event";

import {
  type AppError,
  appErrorSchema,
  type BookSummary,
  bookSummaryListSchema,
  type ImportCompletedEvent,
  type ImportProgressEvent,
  importCompletedEventSchema,
  importProgressEventSchema,
  type ListBooksInput,
  listBooksInputSchema,
  selectImportFilesOutputSchema,
  softRemoveBookInputSchema,
  startImportInputSchema,
  startImportOutputSchema,
} from "./ipc-schemas";

/** 命令名与 Rust `#[tauri::command(rename = ...)]` 一致。 */
export const COMMAND_NAMES = {
  startImport: "startImport",
  listBooks: "listBooks",
  softRemoveBook: "softRemoveBook",
  selectImportFiles: "selectImportFiles",
} as const;

/** 导入事件名。 */
export const IMPORT_EVENT_NAMES = {
  progress: "import-progress",
  completed: "import-completed",
} as const;

export interface CommandTransport {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

export interface EventTransport {
  listen<T>(event: string, handler: (event: T) => void): Promise<UnlistenFn>;
}

/** 固定内部错误，不携带底层细节。 */
export const INTERNAL_APP_ERROR: AppError = {
  code: "E_INTERNAL",
  message: "内部错误",
};

/** 本地输入错误；不包含调用方路径或底层信息。 */
export const INVALID_INPUT_APP_ERROR: AppError = {
  code: "E_INVALID_INPUT",
  message: "输入无效",
};

/** 把任意 rejection 转成稳定 AppError；普通 Error 与 schema 不匹配统一 E_INTERNAL。 */
export function toAppError(error: unknown): AppError {
  const parsed = appErrorSchema.safeParse(error);
  return parsed.success ? parsed.data : INTERNAL_APP_ERROR;
}

/** 默认 Tauri 命令 transport。 */
export const tauriCommandTransport: CommandTransport = {
  invoke: <T>(command: string, args?: Record<string, unknown>) => invoke<T>(command, args),
};

/** 默认 Tauri 事件 transport；向桥暴露 payload 而非 Tauri Event 包装。 */
export const tauriEventTransport: EventTransport = {
  listen: <T>(event: string, handler: (event: T) => void) =>
    listen<T>(event, (tauriEvent: TauriEvent<T>) => handler(tauriEvent.payload)),
};

export interface CommandBridge {
  startImport(paths: string[]): Promise<{ jobId: string }>;
  listBooks(input: ListBooksInput["input"]): Promise<BookSummary[]>;
  softRemoveBook(bookId: string): Promise<void>;
  selectImportFiles(): Promise<string[]>;
}

async function invokeValidated<T>(
  transport: CommandTransport,
  command: string,
  args: Record<string, unknown> | undefined,
  parseResult: (value: unknown) => T,
): Promise<T> {
  try {
    const result = await transport.invoke<unknown>(command, args);
    try {
      return parseResult(result);
    } catch {
      throw INTERNAL_APP_ERROR;
    }
  } catch (error) {
    throw toAppError(error);
  }
}

/** 创建可注入 transport 的命令桥。 */
export function createCommandBridge(transport: CommandTransport): CommandBridge {
  return {
    async startImport(paths) {
      const parsed = startImportInputSchema.safeParse({ paths });
      if (!parsed.success) {
        throw INVALID_INPUT_APP_ERROR;
      }
      return invokeValidated(
        transport,
        COMMAND_NAMES.startImport,
        {
          paths: parsed.data.paths,
        },
        (value) => startImportOutputSchema.parse(value),
      ).then((result) => ({ jobId: result.jobId }));
    },

    async listBooks(input) {
      const parsed = listBooksInputSchema.safeParse({ input });
      if (!parsed.success) {
        throw INVALID_INPUT_APP_ERROR;
      }
      return invokeValidated(
        transport,
        COMMAND_NAMES.listBooks,
        {
          input: parsed.data.input,
        },
        (value) => bookSummaryListSchema.parse(value),
      );
    },

    async selectImportFiles() {
      return invokeValidated(transport, COMMAND_NAMES.selectImportFiles, undefined, (value) =>
        selectImportFilesOutputSchema.parse(value),
      );
    },

    async softRemoveBook(bookId) {
      const parsed = softRemoveBookInputSchema.safeParse({ bookId });
      if (!parsed.success) {
        throw INVALID_INPUT_APP_ERROR;
      }
      await invokeValidated(
        transport,
        COMMAND_NAMES.softRemoveBook,
        { bookId: parsed.data.bookId },
        (value) => {
          if (value !== null) {
            throw INTERNAL_APP_ERROR;
          }
          return undefined;
        },
      );
    },
  };
}

/** 默认命令桥（真实 Tauri transport）。 */
export const commands = createCommandBridge(tauriCommandTransport);

export interface ImportEventHandlers {
  onProgress?: (event: ImportProgressEvent) => void;
  onCompleted?: (event: ImportCompletedEvent) => void;
  onError?: (error: AppError) => void;
}

/** 订阅导入进度与完成事件；返回幂等取消函数。 */
export async function subscribeImportEvents(
  transport: EventTransport,
  handlers: ImportEventHandlers,
): Promise<() => void> {
  const unlisteners: UnlistenFn[] = [];
  let closed = false;

  const handleProgress = (raw: unknown): void => {
    if (closed) {
      return;
    }
    const parsed = importProgressEventSchema.safeParse(raw);
    if (parsed.success) {
      handlers.onProgress?.(parsed.data);
    } else {
      handlers.onError?.(INTERNAL_APP_ERROR);
    }
  };
  const handleCompleted = (raw: unknown): void => {
    if (closed) {
      return;
    }
    const parsed = importCompletedEventSchema.safeParse(raw);
    if (parsed.success) {
      handlers.onCompleted?.(parsed.data);
    } else {
      handlers.onError?.(INTERNAL_APP_ERROR);
    }
  };

  try {
    unlisteners.push(await transport.listen<unknown>(IMPORT_EVENT_NAMES.progress, handleProgress));
    unlisteners.push(
      await transport.listen<unknown>(IMPORT_EVENT_NAMES.completed, handleCompleted),
    );
  } catch (error) {
    for (const unlisten of unlisteners) {
      unlisten();
    }
    throw toAppError(error);
  }

  return () => {
    if (closed) {
      return;
    }
    closed = true;
    for (const unlisten of unlisteners) {
      unlisten();
    }
  };
}
