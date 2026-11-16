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

const UUID_V4_REGEX = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const MAX_TEXT_OFFSET = 9_007_199_254_740_991;

export const readerModeSchema = z.enum(["paged", "scrolling"]);
export type ReaderMode = z.infer<typeof readerModeSchema>;

export const readerThemeSchema = z.enum(["day", "night"]);
export type ReaderTheme = z.infer<typeof readerThemeSchema>;

export const readerSettingsSchema = z.strictObject({
  mode: readerModeSchema,
  fontScale: z.number().finite().min(0.5).max(3),
  theme: readerThemeSchema,
});
export type ReaderSettings = z.infer<typeof readerSettingsSchema>;

const readerQuoteContextSchema = z.strictObject({
  exact: z.string().refine((value) => value.length <= 128, "exact 超长"),
  prefix: z.string().refine((value) => value.length <= 32, "prefix 超长"),
  suffix: z.string().refine((value) => value.length <= 32, "suffix 超长"),
});

const spineHrefSchema = z
  .string()
  .min(1)
  .max(2_048)
  .refine(
    (value) =>
      !value.includes("\0") &&
      !value.includes("\\") &&
      !value.startsWith("/") &&
      !value
        .split("/")
        .some((segment) => segment.length === 0 || segment === "." || segment === ".."),
    "spineHref 必须是 EPUB 根内的相对路径",
  );

export const readerSpineItemSchema = z.strictObject({
  index: z.number().int().min(0),
  href: spineHrefSchema,
});
export type ReaderSpineItem = z.infer<typeof readerSpineItemSchema>;

export const readerLocatorSchema = z.strictObject({
  version: z.literal(1),
  spineIndex: z.number().int().min(0).max(4_294_967_295),
  spineHref: spineHrefSchema,
  textOffset: z.number().int().min(0).max(MAX_TEXT_OFFSET),
  intraSpineProgression: z.number().finite().min(0).max(1),
  quote: readerQuoteContextSchema,
});
export type ReaderLocator = z.infer<typeof readerLocatorSchema>;

export interface ReaderTocNode {
  id: string;
  label: string;
  spineIndex: number;
  fragment?: string;
  children: ReaderTocNode[];
}

export const readerTocNodeSchema: z.ZodType<ReaderTocNode> = z.lazy(() =>
  z.strictObject({
    id: z.string().min(1),
    label: z.string(),
    spineIndex: z.number().int().min(0),
    fragment: z.string().min(1).optional(),
    children: z.array(readerTocNodeSchema),
  }),
);

export const frameUrlSchema = z.string().refine((value) => {
  const match = /^epub:\/\/([0-9a-f]{32,64})\/(.+)$/.exec(value);
  if (!match) {
    return false;
  }
  const encodedResource = match[2];
  // 先整体单次解码，再检查编码后的斜杠、反斜杠、NUL、query/fragment 与穿越段。
  let resource: string;
  try {
    resource = decodeURIComponent(encodedResource);
  } catch {
    return false;
  }
  if (
    resource.includes("?") ||
    resource.includes("#") ||
    resource.includes("\\") ||
    resource.includes("\0") ||
    resource.startsWith("/")
  ) {
    return false;
  }
  return resource
    .split("/")
    .every((segment) => segment.length > 0 && segment !== "." && segment !== "..");
}, "frameUrl 必须是带会话令牌的 epub:// 首章资源 URL");

function frameUrlResource(frameUrl: string): string | null {
  const match = /^epub:\/\/([0-9a-f]{32,64})\/(.+)$/.exec(frameUrl);
  if (!match) {
    return null;
  }
  try {
    return decodeURIComponent(match[2]);
  } catch {
    return null;
  }
}

export const readerSessionSchema = z
  .strictObject({
    sessionId: z.string().regex(UUID_V4_REGEX, "sessionId 必须是 UUID v4"),
    bookId: z.string().min(1),
    title: z.string(),
    frameUrl: frameUrlSchema,
    spine: z.array(readerSpineItemSchema).min(1),
    chapterWeights: z.array(z.number().int().min(0).max(Number.MAX_SAFE_INTEGER)).min(1),
    toc: z.array(readerTocNodeSchema),
    initialLocator: readerLocatorSchema.nullable(),
    initialProgression: z.number().finite().min(0).max(1),
    settings: readerSettingsSchema,
  })
  .superRefine((value, context) => {
    value.spine.forEach((item, position) => {
      if (item.index !== position) {
        context.addIssue({
          code: "custom",
          path: ["spine", position, "index"],
          message: "spine 必须按阅读顺序从 0 连续编号",
        });
      }
    });
    // 同一 manifest 资源可以被多个 itemref 引用；spine 允许重复 href，
    // 逻辑位置由 index 区分（见 SDS 0.15）。
    const walk = (nodes: ReaderTocNode[], path: Array<string | number>): void => {
      nodes.forEach((node, index) => {
        if (node.spineIndex >= value.spine.length) {
          context.addIssue({
            code: "custom",
            path: [...path, index, "spineIndex"],
            message: "TOC spineIndex 超出 spine 范围",
          });
        }
        walk(node.children, [...path, index, "children"]);
      });
    };
    walk(value.toc, ["toc"]);
    if (value.chapterWeights.length !== value.spine.length) {
      context.addIssue({
        code: "custom",
        path: ["chapterWeights"],
        message: "chapterWeights 必须与 spine 等长",
      });
    }
    const resource = frameUrlResource(value.frameUrl);
    if (resource === null || resource !== value.spine[0]?.href) {
      context.addIssue({
        code: "custom",
        path: ["frameUrl"],
        message: "frameUrl 必须指向 spine 首项",
      });
    }
    if (value.initialLocator !== null) {
      const item = value.spine[value.initialLocator.spineIndex];
      if (!item || item.href !== value.initialLocator.spineHref) {
        context.addIssue({
          code: "custom",
          path: ["initialLocator"],
          message: "initialLocator 必须精确匹配 spine 项",
        });
      }
    }
  });
export type ReaderSession = z.infer<typeof readerSessionSchema>;

export const readerSettingsPatchSchema = z
  .strictObject({
    mode: readerModeSchema.optional(),
    fontScale: z.number().finite().min(0.5).max(3).optional(),
    theme: readerThemeSchema.optional(),
  })
  .refine(
    (value) =>
      value.mode !== undefined || value.fontScale !== undefined || value.theme !== undefined,
    "patch 至少包含一个字段",
  );
export type ReaderSettingsPatch = z.infer<typeof readerSettingsPatchSchema>;

export const bookSettingsPatchSchema = z
  .strictObject({
    mode: readerModeSchema.nullable().optional(),
    fontScale: z.number().finite().min(0.5).max(3).nullable().optional(),
    theme: readerThemeSchema.nullable().optional(),
  })
  .refine(
    (value) =>
      value.mode !== undefined || value.fontScale !== undefined || value.theme !== undefined,
    "patch 至少包含一个字段",
  );
export type BookSettingsPatch = z.infer<typeof bookSettingsPatchSchema>;

export const getEffectiveSettingsInputSchema = z.strictObject({
  bookId: z.string().min(1).refine(isNonBlank, "bookId 不能为空白"),
});
export type GetEffectiveSettingsInput = z.infer<typeof getEffectiveSettingsInputSchema>;

export const updateAppSettingsInputSchema = z.strictObject({
  patch: readerSettingsPatchSchema,
});
export type UpdateAppSettingsInput = z.infer<typeof updateAppSettingsInputSchema>;

export const updateBookSettingsInputSchema = z.strictObject({
  input: z.strictObject({
    bookId: z.string().min(1).refine(isNonBlank, "bookId 不能为空白"),
    patch: bookSettingsPatchSchema,
  }),
});
export type UpdateBookSettingsInput = z.infer<typeof updateBookSettingsInputSchema>;

export const saveReadingStateInputSchema = z.strictObject({
  sessionId: z.string().regex(UUID_V4_REGEX, "sessionId 必须是 UUID v4"),
  locator: readerLocatorSchema,
});
export type SaveReadingStateInput = z.infer<typeof saveReadingStateInputSchema>;

export const saveReadingStateResultSchema = z.strictObject({
  globalProgression: z.number().finite().min(0).max(1),
  savedAt: z.string().min(1),
});
export type SaveReadingStateResult = z.infer<typeof saveReadingStateResultSchema>;

export const resetBookSettingsInputSchema = z.strictObject({
  bookId: z.string().min(1).refine(isNonBlank, "bookId 不能为空白"),
});
export type ResetBookSettingsInput = z.infer<typeof resetBookSettingsInputSchema>;

export const bookmarkSchema = z.strictObject({
  id: z.string().regex(UUID_V4_REGEX, "书签 id 必须是 UUID v4"),
  bookId: z.string().min(1),
  locator: readerLocatorSchema,
  excerpt: z.string().max(128).nullable(),
  createdAt: z.string().min(1),
});
export type Bookmark = z.infer<typeof bookmarkSchema>;

export const bookmarkListSchema = z.array(bookmarkSchema);

export const listBookmarksInputSchema = z.strictObject({
  bookId: z.string().min(1).refine(isNonBlank, "bookId 不能为空白"),
});
export type ListBookmarksInput = z.infer<typeof listBookmarksInputSchema>;

export const createBookmarkInputSchema = z.strictObject({
  sessionId: z.string().regex(UUID_V4_REGEX, "sessionId 必须是 UUID v4"),
  locator: readerLocatorSchema,
  excerpt: z.string().max(128).optional(),
});
export type CreateBookmarkInput = z.infer<typeof createBookmarkInputSchema>;

export const deleteBookmarkInputSchema = z.strictObject({
  bookId: z.string().min(1).refine(isNonBlank, "bookId 不能为空白"),
  bookmarkId: z.string().regex(UUID_V4_REGEX, "bookmarkId 必须是 UUID v4"),
});
export type DeleteBookmarkInput = z.infer<typeof deleteBookmarkInputSchema>;

export const openBookInputSchema = z.strictObject({
  bookId: z.string().min(1).refine(isNonBlank, "bookId 不能为空白"),
});
export type OpenBookInput = z.infer<typeof openBookInputSchema>;

export const closeBookInputSchema = z.strictObject({
  sessionId: z.string().regex(UUID_V4_REGEX, "sessionId 必须是 UUID v4"),
});
export type CloseBookInput = z.infer<typeof closeBookInputSchema>;

/** selectImportFiles 输出：0（取消）到 1,000 个非空白原始路径。 */
export const selectImportFilesOutputSchema = z
  .array(z.string().refine(isNonBlank, "路径不能为空白"))
  .max(1_000);
export type SelectImportFilesOutput = z.infer<typeof selectImportFilesOutputSchema>;
