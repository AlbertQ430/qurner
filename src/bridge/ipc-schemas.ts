import { z } from "zod";

/** M2D 定义的 14 个稳定错误码。 */
export const APP_ERROR_CODES = [
  "E_INVALID_INPUT",
  "E_FILE_NOT_FOUND",
  "E_FILE_UNREADABLE",
  "E_EPUB_INVALID",
  "E_EPUB_ENCRYPTED",
  "E_EPUB_FIXED_LAYOUT",
  "E_EPUB_VERTICAL",
  "E_EPUB_SCRIPT_REQUIRED",
  "E_BOOK_NOT_FOUND",
  "E_BOOK_UNAVAILABLE",
  "E_SOURCE_CHANGED",
  "E_SESSION_INVALID",
  "E_STORAGE",
  "E_INTERNAL",
] as const;

export type AppErrorCode = (typeof APP_ERROR_CODES)[number];
export const appErrorCodeSchema = z.enum(APP_ERROR_CODES);

const detailsValueSchema = z.union([z.string(), z.number(), z.boolean()]);

/** IPC 错误；details 只允许标量值。 */
export const appErrorSchema = z.strictObject({
  code: appErrorCodeSchema,
  message: z.string().min(1),
  details: z.record(z.string(), detailsValueSchema).optional(),
});
export type AppError = z.infer<typeof appErrorSchema>;

const jobIdSchema = z
  .string()
  .regex(
    /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    "jobId 必须是 UUID v4",
  );

/** 书库摘要；不暴露 coverCacheKey 或源文件路径。 */
export const bookSummarySchema = z.strictObject({
  id: z.string().min(1),
  title: z.string(),
  author: z.string(),
  coverUrl: z.string().min(1).nullable(),
  importedAt: z.string().min(1),
  lastOpenedAt: z.string().min(1).nullable(),
});
export type BookSummary = z.infer<typeof bookSummarySchema>;

export const bookSummaryListSchema = z.array(bookSummarySchema);

export const importStatusSchema = z.enum(["imported", "reactivated", "duplicate", "failed"]);
export type ImportStatus = z.infer<typeof importStatusSchema>;

/** 单本导入结果；failed 必须带 error，其他状态不得带 error。 */
export const importItemResultSchema = z
  .strictObject({
    sourceName: z
      .string()
      .min(1)
      .refine(
        (value) => !value.includes("/") && !value.includes("\\"),
        "sourceName 只能是文件 basename",
      ),
    status: importStatusSchema,
    bookId: z.string().min(1).optional(),
    error: appErrorSchema.optional(),
  })
  .superRefine((value, ctx) => {
    if (value.status === "failed") {
      if (!value.error) {
        ctx.addIssue({
          code: "custom",
          path: ["error"],
          message: "failed 项必须带 error",
        });
      }
      if (value.bookId !== undefined) {
        ctx.addIssue({
          code: "custom",
          path: ["bookId"],
          message: "failed 项不能带 bookId",
        });
      }
      return;
    }
    if (value.error !== undefined) {
      ctx.addIssue({
        code: "custom",
        path: ["error"],
        message: "非 failed 项不能带 error",
      });
    }
  });
export type ImportItemResult = z.infer<typeof importItemResultSchema>;

export const importProgressEventSchema = z
  .strictObject({
    jobId: jobIdSchema,
    completed: z.number().int().min(1),
    total: z.number().int().min(1).max(1_000),
    item: importItemResultSchema,
  })
  .refine((value) => value.completed <= value.total, {
    message: "completed 不能超过 total",
    path: ["completed"],
  });
export type ImportProgressEvent = z.infer<typeof importProgressEventSchema>;

export const importCompletedEventSchema = z.strictObject({
  jobId: jobIdSchema,
  items: z.array(importItemResultSchema).min(1).max(1_000),
});
export type ImportCompletedEvent = z.infer<typeof importCompletedEventSchema>;

const isNonBlank = (value: string) => value.trim().length > 0;

/** startImport 输入：1–1,000 条非空白路径，保留原始字符串。 */
export const startImportInputSchema = z.strictObject({
  paths: z.array(z.string().refine(isNonBlank, "路径不能为空白")).min(1).max(1_000),
});
export type StartImportInput = z.infer<typeof startImportInputSchema>;

export const startImportOutputSchema = z.strictObject({
  jobId: jobIdSchema,
});
export type StartImportOutput = z.infer<typeof startImportOutputSchema>;

export const bookSortSchema = z.enum(["title", "author", "importedAt"]);
export type BookSort = z.infer<typeof bookSortSchema>;

export const sortDirectionSchema = z.enum(["asc", "desc"]);
export type SortDirection = z.infer<typeof sortDirectionSchema>;

export const listBooksInputSchema = z.strictObject({
  input: z.strictObject({
    query: z.string().optional(),
    sort: bookSortSchema,
    direction: sortDirectionSchema,
  }),
});
export type ListBooksInput = z.infer<typeof listBooksInputSchema>;

export const softRemoveBookInputSchema = z.strictObject({
  bookId: z.string().min(1).refine(isNonBlank, "bookId 不能为空白"),
});
export type SoftRemoveBookInput = z.infer<typeof softRemoveBookInputSchema>;

/** selectImportFiles 输出：0（取消）到 1,000 个非空白原始路径。 */
export const selectImportFilesOutputSchema = z
  .array(z.string().refine(isNonBlank, "路径不能为空白"))
  .max(1_000);
export type SelectImportFilesOutput = z.infer<typeof selectImportFilesOutputSchema>;
