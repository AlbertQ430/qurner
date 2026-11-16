import { describe, expect, it } from "vitest";

import { frameUrlSchema } from "../../bridge/ipc-schemas";
import { extractSessionToken } from "./reader-errors";
import { buildSpineFrameUrl, encodeSpineHref } from "./session-navigation";

const TOKEN = "a".repeat(32);

describe("spine frame URLs", () => {
  it("encodes reserved bytes and keeps unreserved characters and slashes", () => {
    expect(encodeSpineHref("OEBPS/chapter1.xhtml")).toBe("OEBPS/chapter1.xhtml");
    expect(encodeSpineHref("OEBPS/第一章 节.xhtml")).toBe(
      "OEBPS/%E7%AC%AC%E4%B8%80%E7%AB%A0%20%E8%8A%82.xhtml",
    );
    expect(encodeSpineHref("OEBPS/100%.xhtml")).toBe("OEBPS/100%25.xhtml");
    expect(encodeSpineHref("a/b/c.xhtml")).toBe("a/b/c.xhtml");
  });

  it("builds a frame URL that passes the strict frame schema", () => {
    for (const href of [
      "OEBPS/chapter1.xhtml",
      "OEBPS/第一章 节.xhtml",
      "OEBPS/100%.xhtml",
      "a/b/c.xhtml",
    ]) {
      const url = buildSpineFrameUrl(TOKEN, href);
      expect(url).not.toBeNull();
      expect(frameUrlSchema.safeParse(url).success).toBe(true);
      expect(extractSessionToken(url ?? "")).toBe(TOKEN);
      expect(decodeURIComponent((url ?? "").slice(`epub://${TOKEN}/`.length))).toBe(href);
    }
  });

  it("rejects unsafe or invalid paths and tokens", () => {
    for (const href of [
      "",
      "/absolute.xhtml",
      "a/../b.xhtml",
      "a/./b.xhtml",
      "a//b.xhtml",
      "a\\b.xhtml",
      "a\0b.xhtml",
      "..",
      "a".repeat(2_049),
    ]) {
      expect(buildSpineFrameUrl(TOKEN, href)).toBeNull();
    }
    expect(buildSpineFrameUrl("not-a-token", "a.xhtml")).toBeNull();
    expect(buildSpineFrameUrl("A".repeat(32), "a.xhtml")).toBeNull();
  });
});
