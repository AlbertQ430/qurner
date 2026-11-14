import { z } from "zod";

/** 阅读消息协议版本。 */
export const PROTOCOL_VERSION = 1;

/** JavaScript 安全整数上限（2^53 - 1），与 Rust MAX_TEXT_OFFSET 一致。 */
export const MAX_TEXT_OFFSET = 9_007_199_254_740_991;
export const MAX_QUOTE_EXACT_UTF16 = 128;
export const MAX_QUOTE_CONTEXT_UTF16 = 32;

const sessionIdSchema = z
  .string()
  .regex(
    /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    "sessionId 必须是 UUID v4",
  );
const sessionTokenSchema = z.string().regex(/^[0-9a-f]{32,64}$/);

const baseFields = {
  protocolVersion: z.literal(PROTOCOL_VERSION),
  sessionId: sessionIdSchema,
  sessionToken: sessionTokenSchema,
};

export const readerModeSchema = z.enum(["paged", "scrolling"]);
export type ReaderMode = z.infer<typeof readerModeSchema>;

const spineHrefSchema = z
  .string()
  .min(1)
  .max(2048)
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

const quoteSchema = z.strictObject({
  exact: z.string().refine((value) => value.length <= MAX_QUOTE_EXACT_UTF16, "exact 超长"),
  prefix: z.string().refine((value) => value.length <= MAX_QUOTE_CONTEXT_UTF16, "prefix 超长"),
  suffix: z.string().refine((value) => value.length <= MAX_QUOTE_CONTEXT_UTF16, "suffix 超长"),
});

export const readerLocatorSchema = z.strictObject({
  version: z.literal(1),
  spineIndex: z.number().int().min(0).max(4_294_967_295),
  spineHref: spineHrefSchema,
  textOffset: z.number().int().min(0).max(MAX_TEXT_OFFSET),
  intraSpineProgression: z.number().min(0).max(1),
  quote: quoteSchema,
});
export type ReaderLocator = z.infer<typeof readerLocatorSchema>;

export type ReaderQuoteContext = ReaderLocator["quote"];

const pageStateFields = {
  pageIndex: z.number().int().min(0),
  pageCount: z.number().int().min(0),
  atStart: z.boolean(),
  atEnd: z.boolean(),
};

const pageStateSchema = z
  .strictObject(pageStateFields)
  .refine(
    (value) => (value.pageCount === 0 ? value.pageIndex === 0 : value.pageIndex < value.pageCount),
    "分页时 pageIndex 必须小于 pageCount",
  );

export const locationChangedSchema = z
  .strictObject({
    reason: z.enum(["init", "turn", "scroll", "layout", "go-to", "capture"]),
    locator: readerLocatorSchema,
    mode: readerModeSchema,
    ...pageStateFields,
  })
  .refine(
    (value) => (value.pageCount === 0 ? value.pageIndex === 0 : value.pageIndex < value.pageCount),
    "分页时 pageIndex 必须小于 pageCount",
  );
export type ReaderLocationChangedPayload = z.infer<typeof locationChangedSchema>;

export type ReaderPageChangedPayload = z.infer<typeof pageStateSchema>;

export type ReaderSpineRequestPayload = {
  direction: "prev" | "next";
  locator: ReaderLocator;
};

const directionSchema = z.enum(["prev", "next"]);

/** 严格消息信封；所有对象（含 payload）拒绝未知字段。 */
export const readerMessageSchema = z.discriminatedUnion("type", [
  z.strictObject({
    ...baseFields,
    type: z.literal("reader:init"),
    payload: z.strictObject({
      mode: readerModeSchema,
      initialLocator: readerLocatorSchema.nullable(),
    }),
  }),
  z.strictObject({
    ...baseFields,
    type: z.literal("reader:ready"),
    payload: z.strictObject({ engineVersion: z.string().min(1).max(64) }),
  }),
  z.strictObject({
    ...baseFields,
    type: z.literal("reader:turn-page"),
    payload: z.strictObject({ direction: directionSchema }),
  }),
  z.strictObject({
    ...baseFields,
    type: z.literal("reader:error"),
    payload: z.strictObject({
      code: z.string().min(1).max(64),
      message: z.string().min(1).max(500),
    }),
  }),
  z.strictObject({
    ...baseFields,
    type: z.literal("reader:go-to"),
    payload: z.strictObject({ locator: readerLocatorSchema }),
  }),
  z.strictObject({
    ...baseFields,
    type: z.literal("reader:update-settings"),
    payload: z.strictObject({ mode: readerModeSchema }),
  }),
  z.strictObject({
    ...baseFields,
    type: z.literal("reader:capture-locator"),
    payload: z.strictObject({}),
  }),
  z.strictObject({
    ...baseFields,
    type: z.literal("reader:location-changed"),
    payload: locationChangedSchema,
  }),
  z.strictObject({
    ...baseFields,
    type: z.literal("reader:page-changed"),
    payload: pageStateSchema,
  }),
  z.strictObject({
    ...baseFields,
    type: z.literal("reader:spine-request"),
    payload: z.strictObject({
      direction: directionSchema,
      locator: readerLocatorSchema,
    }),
  }),
]);

export type ReaderMessage = z.infer<typeof readerMessageSchema>;
export type ReaderMessageType = ReaderMessage["type"];

/** 校验会话 ID 是否为 M3A 生成的小写 UUID v4。 */
export function isValidSessionId(value: string): boolean {
  return sessionIdSchema.safeParse(value).success;
}

/** 校验消息；失败返回 null，不抛异常。 */
export function parseReaderMessage(value: unknown): ReaderMessage | null {
  const result = readerMessageSchema.safeParse(value);
  return result.success ? result.data : null;
}

type PayloadOf<T extends ReaderMessageType> = Extract<ReaderMessage, { type: T }>["payload"];

/** 创建并校验消息；非法参数抛 ZodError（调用方必须捕获）。 */
export function createReaderMessage<T extends ReaderMessageType>(
  type: T,
  sessionId: string,
  sessionToken: string,
  payload: PayloadOf<T>,
): ReaderMessage {
  return readerMessageSchema.parse({
    protocolVersion: PROTOCOL_VERSION,
    sessionId,
    sessionToken,
    type,
    payload,
  });
}
