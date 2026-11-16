import { frameUrlSchema } from "../../bridge/ipc-schemas";

const SPINE_TOKEN = /^[0-9a-f]{32,64}$/;
const UNRESERVED = /^[A-Za-z0-9\-._~/]$/;

/** 按 Rust `build_frame_url` 规则编码 spine href：保留 unreserved 与 `/`。 */
export function encodeSpineHref(href: string): string | null {
  if (href.length === 0 || href.length > 2_048) {
    return null;
  }
  if (href.startsWith("/") || href.includes("\0") || href.includes("\\")) {
    return null;
  }
  const segments = href.split("/");
  if (segments.some((segment) => segment.length === 0 || segment === "." || segment === "..")) {
    return null;
  }
  let output = "";
  for (const byte of new TextEncoder().encode(href)) {
    const character = String.fromCharCode(byte);
    output += UNRESERVED.test(character)
      ? character
      : `%${byte.toString(16).toUpperCase().padStart(2, "0")}`;
  }
  return output;
}

/** 以已验证 token 与 spine href 构造章节 URL；结果必须通过完整 frame URL 校验。 */
export function buildSpineFrameUrl(token: string, href: string): string | null {
  if (!SPINE_TOKEN.test(token)) {
    return null;
  }
  const encoded = encodeSpineHref(href);
  if (encoded === null) {
    return null;
  }
  const url = `epub://${token}/${encoded}`;
  return frameUrlSchema.safeParse(url).success ? url : null;
}
