import { describe, expect, it } from "vitest";

import {
  COMMAND_NAMES,
  createCommandBridge,
  IMPORT_EVENT_NAMES,
  INTERNAL_APP_ERROR,
  subscribeImportEvents,
  toAppError,
} from "./commands";
import {
  APP_ERROR_CODES,
  type AppError,
  bookSummarySchema,
  importCompletedEventSchema,
  importItemResultSchema,
  importProgressEventSchema,
  listBooksInputSchema,
  softRemoveBookInputSchema,
  startImportInputSchema,
} from "./ipc-schemas";

const JOB_ID = "123e4567-e89b-42d3-a456-426614174000";

class FakeCommandTransport {
  calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
  responses: unknown[] = [];
  failure: unknown = null;

  async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    this.calls.push({ command, args });
    if (this.failure !== null) {
      throw this.failure;
    }
    return this.responses.shift() as T;
  }
}

class FakeEventTransport {
  listeners = new Map<string, Array<(event: unknown) => void>>();
  unlistenCalls: string[] = [];
  failSecond = false;

  async listen<T>(event: string, handler: (event: T) => void): Promise<() => void> {
    if (this.failSecond && this.listeners.size === 1) {
      throw new Error("listen failed");
    }
    const handlers = this.listeners.get(event) ?? [];
    handlers.push(handler as (event: unknown) => void);
    this.listeners.set(event, handlers);
    return () => {
      this.unlistenCalls.push(event);
      const current = this.listeners.get(event) ?? [];
      this.listeners.set(
        event,
        current.filter((candidate) => candidate !== handler),
      );
    };
  }

  emit(event: string, payload: unknown): void {
    for (const handler of this.listeners.get(event) ?? []) {
      handler(payload);
    }
  }
}

function bridge(): {
  transport: FakeCommandTransport;
  client: ReturnType<typeof createCommandBridge>;
} {
  const transport = new FakeCommandTransport();
  return { transport, client: createCommandBridge(transport) };
}

const BOOK = {
  id: "book-1",
  title: "示例书",
  author: "作者",
  coverUrl: null,
  importedAt: "2026-11-14T00:00:00Z",
  lastOpenedAt: null,
};

const IMPORTED_ITEM = {
  sourceName: "示例书.epub",
  status: "imported" as const,
  bookId: "book-1",
};

const FAILED_ITEM = {
  sourceName: "坏书.epub",
  status: "failed" as const,
  error: { code: "E_EPUB_INVALID" as const, message: "EPUB 无法解析" },
};

describe("command bridge", () => {
  it("startImport invokes with camelCase args and parses jobId", async () => {
    const { transport, client } = bridge();
    transport.responses.push({ jobId: JOB_ID });
    await expect(client.startImport(["/tmp/a.epub", " /tmp/b.epub "])).resolves.toEqual({
      jobId: JOB_ID,
    });
    expect(transport.calls).toHaveLength(1);
    expect(transport.calls[0]?.command).toBe(COMMAND_NAMES.startImport);
    expect(transport.calls[0]?.args).toEqual({
      paths: ["/tmp/a.epub", " /tmp/b.epub "],
    });
  });

  it("startImport accepts 1 and 1,000 paths", async () => {
    for (const count of [1, 1_000]) {
      const { transport, client } = bridge();
      transport.responses.push({ jobId: JOB_ID });
      const paths = Array.from({ length: count }, (_, index) => `/tmp/${index}.epub`);
      await client.startImport(paths);
      expect(transport.calls[0]?.args).toEqual({ paths });
    }
  });

  it("startImport rejects invalid input without invoking", async () => {
    for (const paths of [
      [],
      ["   "],
      ["\t\n"],
      Array.from({ length: 1_001 }, (_, index) => `/tmp/${index}.epub`),
      ["/tmp/a.epub", 42],
    ]) {
      const { transport, client } = bridge();
      await expect(client.startImport(paths as unknown as string[])).rejects.toEqual({
        code: "E_INVALID_INPUT",
        message: "输入无效",
      });
      expect(transport.calls).toHaveLength(0);
    }
  });

  it("startImport maps malformed response to E_INTERNAL", async () => {
    const { transport, client } = bridge();
    transport.responses.push({ jobId: "not-a-uuid" });
    await expect(client.startImport(["/tmp/a.epub"])).rejects.toEqual(INTERNAL_APP_ERROR);
  });

  it("listBooks covers sorts, directions and optional query", async () => {
    for (const sort of ["title", "author", "importedAt"] as const) {
      for (const direction of ["asc", "desc"] as const) {
        const { transport, client } = bridge();
        transport.responses.push([]);
        await client.listBooks({ sort, direction });
        expect(transport.calls[0]?.command).toBe(COMMAND_NAMES.listBooks);
        expect(transport.calls[0]?.args).toEqual({ input: { sort, direction } });
      }
    }

    const { transport, client } = bridge();
    transport.responses.push([BOOK]);
    await client.listBooks({ query: "示例", sort: "title", direction: "desc" });
    expect(transport.calls[0]?.args).toEqual({
      input: { query: "示例", sort: "title", direction: "desc" },
    });
    expect(transport.responses).toHaveLength(0);
  });

  it("listBooks rejects unknown enums without invoking", async () => {
    for (const input of [
      { sort: "unknown", direction: "asc" },
      { sort: "title", direction: "up" },
      { sort: "title", direction: "asc", query: 42 },
    ]) {
      const { transport, client } = bridge();
      await expect(
        client.listBooks(input as unknown as { sort: "title"; direction: "asc" }),
      ).rejects.toEqual({ code: "E_INVALID_INPUT", message: "输入无效" });
      expect(transport.calls).toHaveLength(0);
    }
  });

  it("listBooks maps non-array or malformed summaries to E_INTERNAL", async () => {
    for (const response of [
      { items: [] },
      [{ ...BOOK, coverCacheKey: "secret" }],
      [{ ...BOOK, id: "" }],
    ]) {
      const { transport, client } = bridge();
      transport.responses.push(response);
      await expect(client.listBooks({ sort: "title", direction: "asc" })).rejects.toEqual(
        INTERNAL_APP_ERROR,
      );
    }
  });

  it("softRemoveBook invokes with bookId and resolves void", async () => {
    const { transport, client } = bridge();
    transport.responses.push(null);
    await expect(client.softRemoveBook(" book-1 ")).resolves.toBeUndefined();
    expect(transport.calls[0]?.command).toBe(COMMAND_NAMES.softRemoveBook);
    expect(transport.calls[0]?.args).toEqual({ bookId: " book-1 " });
  });

  it("selectImportFiles invokes without args and parses arrays", async () => {
    for (const paths of [
      [],
      ["/tmp/a.epub"],
      Array.from({ length: 1_000 }, (_, index) => `/tmp/${index}.epub`),
    ]) {
      const { transport, client } = bridge();
      transport.responses.push(paths);
      await expect(client.selectImportFiles()).resolves.toEqual(paths);
      expect(transport.calls[0]).toEqual({
        command: COMMAND_NAMES.selectImportFiles,
        args: undefined,
      });
    }
  });

  it("selectImportFiles maps invalid output to E_INTERNAL", async () => {
    for (const response of [
      { paths: [] },
      "not-an-array",
      [42],
      ["   "],
      Array.from({ length: 1_001 }, (_, index) => `/tmp/${index}.epub`),
    ]) {
      const { transport, client } = bridge();
      transport.responses.push(response);
      await expect(client.selectImportFiles()).rejects.toEqual(INTERNAL_APP_ERROR);
    }
  });

  it("softRemoveBook maps non-null success payloads to E_INTERNAL", async () => {
    for (const response of [{}, "ok", undefined, [], false, 0]) {
      const { transport, client } = bridge();
      transport.responses.push(response);
      await expect(client.softRemoveBook("book-1")).rejects.toEqual(INTERNAL_APP_ERROR);
    }
  });

  it("softRemoveBook rejects blank ids without invoking", async () => {
    for (const bookId of ["", "   ", 42]) {
      const { transport, client } = bridge();
      await expect(client.softRemoveBook(bookId as unknown as string)).rejects.toEqual({
        code: "E_INVALID_INPUT",
        message: "输入无效",
      });
      expect(transport.calls).toHaveLength(0);
    }
  });

  it("preserves all 14 valid AppErrors", async () => {
    for (const code of APP_ERROR_CODES) {
      const { transport, client } = bridge();
      const error: AppError = {
        code,
        message: "稳定错误",
        details: { retryable: true, attempt: 2, reason: "测试" },
      };
      transport.failure = error;
      await expect(client.listBooks({ sort: "title", direction: "asc" })).rejects.toEqual(error);
    }
  });

  it("maps invalid AppErrors and unknown errors to E_INTERNAL", async () => {
    const invalid = [
      { code: "E_UNKNOWN", message: "x" },
      { code: "E_INTERNAL" },
      { code: "E_INPUT", message: "" },
      { code: "E_STORAGE", message: "x", details: { nested: {} } },
      { code: "E_STORAGE", message: "x", details: [1] },
      { code: "E_STORAGE", message: "x", details: null },
      new Error("secret /tmp/private.epub"),
      "plain string",
      null,
    ];
    for (const failure of invalid) {
      const { transport, client } = bridge();
      transport.failure = failure;
      const error = await client
        .listBooks({ sort: "title", direction: "asc" })
        .catch((caught: unknown) => caught);
      expect(error).toEqual(INTERNAL_APP_ERROR);
      expect(JSON.stringify(error)).not.toContain("secret");
      expect(JSON.stringify(error)).not.toContain("/tmp");
    }
    expect(toAppError(new Error("boom"))).toEqual(INTERNAL_APP_ERROR);
  });
});

describe("IPC schemas", () => {
  it("accepts a valid BookSummary and rejects unknown fields", () => {
    expect(bookSummarySchema.safeParse(BOOK).success).toBe(true);
    expect(bookSummarySchema.safeParse({ ...BOOK, coverUrl: "cover://abc" }).success).toBe(true);
    expect(bookSummarySchema.safeParse({ ...BOOK, coverCacheKey: "secret" }).success).toBe(false);
    expect(bookSummarySchema.safeParse({ ...BOOK, coverUrl: "" }).success).toBe(false);
  });

  it("accepts valid import items and enforces status/error exclusivity", () => {
    expect(importItemResultSchema.safeParse(IMPORTED_ITEM).success).toBe(true);
    expect(importItemResultSchema.safeParse(FAILED_ITEM).success).toBe(true);

    expect(importItemResultSchema.safeParse({ ...FAILED_ITEM, error: undefined }).success).toBe(
      false,
    );
    expect(
      importItemResultSchema.safeParse({
        ...IMPORTED_ITEM,
        error: { code: "E_STORAGE", message: "x" },
      }).success,
    ).toBe(false);
    expect(importItemResultSchema.safeParse({ ...FAILED_ITEM, bookId: "book-1" }).success).toBe(
      false,
    );
    expect(
      importItemResultSchema.safeParse({ ...IMPORTED_ITEM, sourceName: "a/b.epub" }).success,
    ).toBe(false);
    expect(importItemResultSchema.safeParse({ ...IMPORTED_ITEM, status: "done" }).success).toBe(
      false,
    );
    expect(importItemResultSchema.safeParse({ ...IMPORTED_ITEM, extra: 1 }).success).toBe(false);
  });

  it("validates progress and completed events", () => {
    const progress = {
      jobId: JOB_ID,
      completed: 1,
      total: 2,
      item: IMPORTED_ITEM,
    };
    expect(importProgressEventSchema.safeParse(progress).success).toBe(true);
    expect(importProgressEventSchema.safeParse({ ...progress, completed: 3 }).success).toBe(false);
    expect(importProgressEventSchema.safeParse({ ...progress, total: 1_001 }).success).toBe(false);
    expect(importProgressEventSchema.safeParse({ ...progress, jobId: "uuid-1" }).success).toBe(
      false,
    );
    expect(importProgressEventSchema.safeParse({ ...progress, extra: true }).success).toBe(false);

    const completed = { jobId: JOB_ID, items: [IMPORTED_ITEM, FAILED_ITEM] };
    expect(importCompletedEventSchema.safeParse(completed).success).toBe(true);
    expect(importCompletedEventSchema.safeParse({ ...completed, items: [] }).success).toBe(false);
  });

  it("validates command inputs", () => {
    expect(startImportInputSchema.safeParse({ paths: ["/tmp/a.epub"] }).success).toBe(true);
    expect(startImportInputSchema.safeParse({ paths: [] }).success).toBe(false);
    expect(startImportInputSchema.safeParse({ paths: [" "] }).success).toBe(false);
    expect(
      listBooksInputSchema.safeParse({ input: { sort: "title", direction: "asc" } }).success,
    ).toBe(true);
    expect(listBooksInputSchema.safeParse({ input: { sort: "title" } }).success).toBe(false);
    expect(softRemoveBookInputSchema.safeParse({ bookId: "book-1" }).success).toBe(true);
    expect(softRemoveBookInputSchema.safeParse({ bookId: "  " }).success).toBe(false);
  });
});

describe("import events", () => {
  it("subscribes to exact event names and forwards valid payloads", async () => {
    const transport = new FakeEventTransport();
    const progress: unknown[] = [];
    const completed: unknown[] = [];
    const errors: AppError[] = [];
    const unsubscribe = await subscribeImportEvents(transport, {
      onProgress: (event) => progress.push(event),
      onCompleted: (event) => completed.push(event),
      onError: (event) => errors.push(event),
    });

    expect([...transport.listeners.keys()]).toEqual([
      IMPORT_EVENT_NAMES.progress,
      IMPORT_EVENT_NAMES.completed,
    ]);

    transport.emit(IMPORT_EVENT_NAMES.progress, {
      jobId: JOB_ID,
      completed: 1,
      total: 2,
      item: IMPORTED_ITEM,
    });
    transport.emit(IMPORT_EVENT_NAMES.completed, {
      jobId: JOB_ID,
      items: [IMPORTED_ITEM, FAILED_ITEM],
    });
    expect(progress).toHaveLength(1);
    expect(completed).toHaveLength(1);
    expect(errors).toHaveLength(0);

    unsubscribe();
    unsubscribe();
    expect(transport.unlistenCalls).toEqual([
      IMPORT_EVENT_NAMES.progress,
      IMPORT_EVENT_NAMES.completed,
    ]);
  });

  it("maps malformed or unknown event payloads to E_INTERNAL", async () => {
    const transport = new FakeEventTransport();
    const errors: AppError[] = [];
    const unsubscribe = await subscribeImportEvents(transport, {
      onProgress: () => undefined,
      onCompleted: () => undefined,
      onError: (event) => errors.push(event),
    });

    for (const payload of [
      { jobId: "bad", completed: 1, total: 2, item: IMPORTED_ITEM },
      { jobId: JOB_ID, completed: 2, total: 1, item: IMPORTED_ITEM },
      {
        jobId: JOB_ID,
        completed: 1,
        total: 2,
        item: { ...IMPORTED_ITEM, sourceName: "a\\b.epub" },
      },
      { jobId: JOB_ID, completed: 1, total: 2, item: IMPORTED_ITEM, extra: true },
      { jobId: JOB_ID, items: [] },
      { jobId: JOB_ID, items: [{ ...IMPORTED_ITEM, status: "done" }] },
    ]) {
      transport.emit(IMPORT_EVENT_NAMES.progress, payload);
      transport.emit(IMPORT_EVENT_NAMES.completed, payload);
    }
    expect(errors.length).toBeGreaterThan(0);
    expect(errors.every((error) => error.code === "E_INTERNAL")).toBe(true);
    unsubscribe();
  });

  it("cleans up the first listener when the second registration fails", async () => {
    const transport = new FakeEventTransport();
    transport.failSecond = true;
    await expect(subscribeImportEvents(transport, { onProgress: () => undefined })).rejects.toEqual(
      INTERNAL_APP_ERROR,
    );
    expect(transport.unlistenCalls).toEqual([IMPORT_EVENT_NAMES.progress]);
  });
});
