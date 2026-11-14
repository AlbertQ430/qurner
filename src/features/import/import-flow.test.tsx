import { type QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ImportEventHandlers } from "../../bridge/commands";
import type {
  ImportCompletedEvent,
  ImportItemResult,
  ImportProgressEvent,
} from "../../bridge/ipc-schemas";
import { createQueryClient, queryKeys } from "../../query/query-client";
import { ImportButton } from "./ImportButton";
import {
  type ImportCommandClient,
  type ImportEventSubscriber,
  ImportFlowProvider,
} from "./ImportFlowProvider";
import { ImportStatus } from "./ImportStatus";
import { ImportSummary } from "./ImportSummary";

const JOB = "123e4567-e89b-42d3-a456-426614174000";
const OTHER_JOB = "00000000-0000-4000-8000-000000000000";
const PATH = "/Users/reader/Books/示例书.epub";

const IMPORTED: ImportItemResult = {
  sourceName: "示例书.epub",
  status: "imported",
  bookId: "book-1",
};

const REACTIVATED: ImportItemResult = {
  sourceName: "旧书.epub",
  status: "reactivated",
  bookId: "book-2",
};

const DUPLICATE: ImportItemResult = {
  sourceName: "重复书.epub",
  status: "duplicate",
};

const FAILED: ImportItemResult = {
  sourceName: "坏书.epub",
  status: "failed",
  error: { code: "E_EPUB_INVALID", message: "raw底层错误" },
};

class FakeCommands implements ImportCommandClient {
  paths: string[] = [PATH];
  selectError: unknown = null;
  startError: unknown = null;
  defer = false;
  startCalls: string[][] = [];
  subscribeCallsAtStart = 0;
  deferredResolve: ((value: { jobId: string }) => void) | null = null;
  events: FakeEvents | null = null;

  async selectImportFiles(): Promise<string[]> {
    if (this.selectError !== null) {
      throw this.selectError;
    }
    return this.paths;
  }

  async startImport(paths: string[]): Promise<{ jobId: string }> {
    this.startCalls.push(paths);
    this.subscribeCallsAtStart = this.events?.subscribeCalls ?? 0;
    if (this.startError !== null) {
      throw this.startError;
    }
    if (this.defer) {
      return new Promise((resolve) => {
        this.deferredResolve = resolve;
      });
    }
    return { jobId: JOB };
  }
}

class FakeEvents implements ImportEventSubscriber {
  handlers: ImportEventHandlers | null = null;
  subscribeCalls = 0;
  unsubscribed = 0;
  subscribeError: unknown = null;

  async subscribe(handlers: ImportEventHandlers): Promise<() => void> {
    this.subscribeCalls += 1;
    if (this.subscribeError !== null) {
      throw this.subscribeError;
    }
    this.handlers = handlers;
    return () => {
      this.unsubscribed += 1;
    };
  }

  progress(event: ImportProgressEvent): void {
    this.handlers?.onProgress?.(event);
  }

  completed(event: ImportCompletedEvent): void {
    this.handlers?.onCompleted?.(event);
  }
}

interface Harness {
  commands: FakeCommands;
  events: FakeEvents;
  queryClient: QueryClient;
  container: HTMLElement;
}

function setup(): Harness {
  const commands = new FakeCommands();
  const events = new FakeEvents();
  commands.events = events;
  const queryClient = createQueryClient();
  const view = render(
    <QueryClientProvider client={queryClient}>
      <ImportFlowProvider
        commandClient={commands}
        eventSubscriber={events}
        queryClient={queryClient}
      >
        <ImportButton />
        <ImportStatus />
        <ImportSummary />
      </ImportFlowProvider>
    </QueryClientProvider>,
  );
  return { commands, events, queryClient, container: view.container };
}

function importButton(): HTMLElement {
  return screen.getByRole("button", { name: "导入 EPUB" });
}

function progress(
  jobId: string,
  completed: number,
  total: number,
  item: ImportItemResult,
): ImportProgressEvent {
  return { jobId, completed, total, item };
}

function completed(jobId: string, items: ImportItemResult[]): ImportCompletedEvent {
  return { jobId, items };
}

async function clickImport(): Promise<void> {
  await act(async () => {
    fireEvent.click(importButton());
    await new Promise((resolve) => {
      setTimeout(resolve, 0);
    });
  });
}

describe("import flow", () => {
  it("subscribes before startImport and completes a single book", async () => {
    const { commands, events, queryClient } = setup();
    const invalidate = vi.spyOn(queryClient, "invalidateQueries");

    await clickImport();
    await waitFor(() => {
      expect(commands.startCalls).toHaveLength(1);
    });
    expect(commands.startCalls[0]).toEqual([PATH]);
    expect(commands.subscribeCallsAtStart).toBe(1);
    expect(events.subscribeCalls).toBe(1);

    act(() => {
      events.progress(progress(JOB, 1, 1, IMPORTED));
    });
    expect(screen.getByText("已完成 1 / 1")).toBeInTheDocument();

    act(() => {
      events.completed(completed(JOB, [IMPORTED]));
    });

    await waitFor(() => {
      expect(screen.getByRole("dialog")).toBeInTheDocument();
    });
    expect(screen.getAllByText("已导入 1")).toHaveLength(1);
    expect(screen.getByText("示例书.epub")).toBeInTheDocument();
    expect(events.unsubscribed).toBe(1);
    expect(invalidate).toHaveBeenCalledWith({ queryKey: queryKeys.books });
    expect(screen.queryByText("正在准备导入…")).not.toBeInTheDocument();
  });

  it("summarizes mixed batches without exposing paths", async () => {
    const { events, container, queryClient } = setup();
    const consoleError = vi.spyOn(console, "error");
    const consoleWarn = vi.spyOn(console, "warn");
    await clickImport();
    await waitFor(() => {
      expect(events.subscribeCalls).toBe(1);
    });

    act(() => {
      events.completed(completed(JOB, [IMPORTED, REACTIVATED, DUPLICATE, FAILED]));
    });

    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveAccessibleName("导入结果");
    for (const label of ["已导入 1", "已恢复 1", "重复 1", "失败 1"]) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
    expect(screen.getByText("旧书.epub")).toBeInTheDocument();
    expect(screen.getByText("重复书.epub")).toBeInTheDocument();
    expect(screen.getByText("坏书.epub")).toBeInTheDocument();
    expect(screen.getByText(/文件不是有效或完整的 EPUB，请检查文件。/)).toBeInTheDocument();
    expect(screen.queryByText("raw底层错误")).not.toBeInTheDocument();
    expect(container.textContent).not.toContain("/Users/reader/Books");
    expect(container.textContent).not.toContain("示例书.epub/");
    expect(JSON.stringify(consoleError.mock.calls)).not.toContain("/Users/");
    expect(JSON.stringify(consoleWarn.mock.calls)).not.toContain("/Users/");
    expect(JSON.stringify(queryClient.getQueryCache().getAll())).not.toContain("/Users/");
  });

  it("summarizes an all-failed batch and allows another import", async () => {
    const { commands, events } = setup();
    await clickImport();
    await waitFor(() => {
      expect(events.subscribeCalls).toBe(1);
    });

    const failedAgain: ImportItemResult = {
      sourceName: "损坏.epub",
      status: "failed",
      error: { code: "E_EPUB_ENCRYPTED", message: "底层加密错误" },
    };
    act(() => {
      events.completed(completed(JOB, [FAILED, failedAgain]));
    });

    await screen.findByRole("dialog");
    expect(screen.getByText("失败 2")).toBeInTheDocument();
    for (const label of ["已导入 0", "已恢复 0", "重复 0"]) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
    expect(screen.getByText(/文件不是有效或完整的 EPUB，请检查文件。/)).toBeInTheDocument();
    expect(screen.getByText(/书籍受 DRM 或加密保护，Qurner 不支持。/)).toBeInTheDocument();
    expect(screen.queryByText("底层加密错误")).not.toBeInTheDocument();
    expect(screen.queryByText("raw底层错误")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "关闭导入结果" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    await clickImport();
    await waitFor(() => {
      expect(commands.startCalls).toHaveLength(2);
    });
  });

  it("buffers events that arrive before startImport resolves", async () => {
    const { commands, events } = setup();
    commands.defer = true;

    await clickImport();
    await waitFor(() => {
      expect(commands.startCalls).toHaveLength(1);
    });
    expect(screen.getByText("正在准备导入…")).toBeInTheDocument();

    act(() => {
      events.progress(progress(JOB, 1, 2, IMPORTED));
    });
    expect(screen.getByText("正在准备导入…")).toBeInTheDocument();

    await act(async () => {
      commands.deferredResolve?.({ jobId: JOB });
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.getByText("已完成 1 / 2")).toBeInTheDocument();
    });
  });

  it("ignores other jobIds and late events after completion", async () => {
    const { events } = setup();
    await clickImport();
    await waitFor(() => {
      expect(events.subscribeCalls).toBe(1);
    });

    act(() => {
      events.progress(progress(OTHER_JOB, 1, 2, IMPORTED));
      events.completed(completed(OTHER_JOB, [IMPORTED]));
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    act(() => {
      events.completed(completed(JOB, [IMPORTED]));
    });
    await screen.findByRole("dialog");

    act(() => {
      events.progress(progress(JOB, 2, 2, REACTIVATED));
    });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.queryByText("已完成 2 / 2")).not.toBeInTheDocument();
  });

  it("returns to idle on cancel and never starts an import", async () => {
    const { commands } = setup();
    commands.paths = [];

    await clickImport();
    await waitFor(() => {
      expect(importButton()).toBeEnabled();
    });
    expect(commands.startCalls).toHaveLength(0);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByText("正在打开文件选择器…")).not.toBeInTheDocument();
  });

  it("shows a retriable selection error", async () => {
    const { commands } = setup();
    commands.selectError = { code: "E_INVALID_INPUT", message: "raw" };
    await clickImport();
    await waitFor(() => {
      expect(screen.getByText("本次选择无效或超过 1,000 本，请重新选择。")).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
    expect(importButton()).toBeEnabled();
    expect(commands.startCalls).toHaveLength(0);
  });

  it("stops loading when subscription fails without leaking details", async () => {
    const { commands, events, container } = setup();
    events.subscribeError = new Error("secret /Users/reader");
    await clickImport();
    await waitFor(() => {
      expect(screen.getByText("无法打开文件选择器或启动导入，请重试。")).toBeInTheDocument();
    });
    expect(commands.startCalls).toHaveLength(0);
    expect(container.textContent).not.toContain("secret");
  });

  it("stops loading and unsubscribes when startImport fails", async () => {
    const { commands, events, container } = setup();
    commands.startError = new Error("secret /Users/reader");
    await clickImport();
    await waitFor(() => {
      expect(screen.getByText("无法打开文件选择器或启动导入，请重试。")).toBeInTheDocument();
    });
    expect(events.unsubscribed).toBe(1);
    expect(container.textContent).not.toContain("secret");
  });

  it("disables import while busy and recovers after summary close", async () => {
    const { commands, events } = setup();
    commands.defer = true;
    const button = importButton();
    expect(button).toBeEnabled();

    act(() => {
      fireEvent.click(button);
    });
    expect(importButton()).toBeDisabled();
    expect(screen.getByText("正在打开文件选择器…")).toBeInTheDocument();

    await waitFor(() => {
      expect(commands.startCalls).toHaveLength(1);
    });
    await act(async () => {
      commands.deferredResolve?.({ jobId: JOB });
      await Promise.resolve();
    });
    expect(importButton()).toBeDisabled();

    act(() => {
      events.completed(completed(JOB, [IMPORTED]));
    });
    const close = await screen.findByRole("button", { name: "关闭导入结果" });
    expect(importButton()).toBeEnabled();

    fireEvent.click(close);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(importButton()).toBeEnabled();
  });

  it("closes the summary with Escape and allows a second import", async () => {
    const { commands, events } = setup();
    await clickImport();
    await waitFor(() => {
      expect(events.subscribeCalls).toBe(1);
    });
    act(() => {
      events.completed(completed(JOB, [IMPORTED]));
    });
    await screen.findByRole("dialog");

    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    commands.paths = [];
    await clickImport();
    await waitFor(() => {
      expect(importButton()).toBeEnabled();
    });
    expect(commands.startCalls).toHaveLength(1);
  });
});
