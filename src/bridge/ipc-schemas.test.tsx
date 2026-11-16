import { describe, expect, it } from "vitest";

import { readerSessionSchema } from "./ipc-schemas";

function session(overrides: Record<string, unknown> = {}) {
  return {
    sessionId: "123e4567-e89b-42d3-a456-426614174000",
    bookId: "book-1",
    title: "示例书",
    frameUrl: `epub://${"a".repeat(32)}/OEBPS/chapter1.xhtml`,
    spine: [
      { index: 0, href: "OEBPS/chapter1.xhtml" },
      { index: 1, href: "OEBPS/chapter2.xhtml" },
    ],
    chapterWeights: [1, 1],
    toc: [
      {
        id: "c1",
        label: "第一章",
        spineIndex: 0,
        children: [
          {
            id: "s1",
            label: "第一节",
            spineIndex: 0,
            fragment: "s1",
            children: [],
          },
        ],
      },
      { id: "c2", label: "第二章", spineIndex: 1, children: [] },
    ],
    initialLocator: null,
    initialProgression: 0,
    settings: { mode: "paged", fontScale: 1, theme: "day" },
    ...overrides,
  };
}

describe("reader session schema", () => {
  it("accepts a valid spine, nested toc and first/non-first locators", () => {
    expect(readerSessionSchema.safeParse(session()).success).toBe(true);
    expect(
      readerSessionSchema.safeParse(
        session({
          initialLocator: {
            version: 1,
            spineIndex: 1,
            spineHref: "OEBPS/chapter2.xhtml",
            textOffset: 0,
            intraSpineProgression: 0,
            quote: { exact: "", prefix: "", suffix: "" },
          },
        }),
      ).success,
    ).toBe(true);
  });

  it("allows repeated spine hrefs but rejects empty or non-contiguous spines", () => {
    expect(
      readerSessionSchema.safeParse(
        session({
          spine: [
            { index: 0, href: "OEBPS/chapter1.xhtml" },
            { index: 1, href: "OEBPS/chapter1.xhtml" },
          ],
          toc: [{ id: "c1", label: "第一章", spineIndex: 1, children: [] }],
        }),
      ).success,
    ).toBe(true);
    for (const spine of [
      [],
      [
        { index: 0, href: "OEBPS/chapter1.xhtml" },
        { index: 2, href: "OEBPS/chapter2.xhtml" },
      ],
      [
        { index: 0, href: "OEBPS/chapter1.xhtml" },
        { index: 1, href: "../chapter2.xhtml" },
      ],
    ]) {
      expect(readerSessionSchema.safeParse(session({ spine })).success).toBe(false);
    }
  });

  it("rejects out-of-range toc targets, locator mismatch and wrong frame url", () => {
    const invalid = [
      session({
        toc: [{ id: "c3", label: "越界", spineIndex: 2, children: [] }],
      }),
      session({
        initialLocator: {
          version: 1,
          spineIndex: 1,
          spineHref: "OEBPS/chapter1.xhtml",
          textOffset: 0,
          intraSpineProgression: 0,
          quote: { exact: "", prefix: "", suffix: "" },
        },
      }),
      session({
        initialLocator: {
          version: 1,
          spineIndex: 5,
          spineHref: "OEBPS/chapter2.xhtml",
          textOffset: 0,
          intraSpineProgression: 0,
          quote: { exact: "", prefix: "", suffix: "" },
        },
      }),
      session({
        frameUrl: `epub://${"a".repeat(32)}/OEBPS/chapter2.xhtml`,
      }),
      session({ unknownField: true }),
    ];
    for (const value of invalid) {
      expect(readerSessionSchema.safeParse(value).success).toBe(false);
    }
  });
});
