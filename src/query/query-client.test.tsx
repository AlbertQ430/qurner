import { describe, expect, it } from "vitest";

import { createQueryClient, queryKeys } from "./query-client";

describe("query client", () => {
  it("disables retry and focus refetch and exposes shared keys", () => {
    const client = createQueryClient();
    const defaults = client.getDefaultOptions();

    expect(defaults.queries?.retry).toBe(false);
    expect(defaults.queries?.refetchOnWindowFocus).toBe(false);
    expect(queryKeys.books).toEqual(["books"]);
  });

  it("builds book list keys from query, sort and direction only", () => {
    expect(
      queryKeys.bookList({
        query: "中文",
        sort: "title",
        direction: "asc",
      }),
    ).toEqual(["books", "list", "title", "asc", "中文"]);
    expect(queryKeys.bookList({ sort: "importedAt", direction: "desc" })).toEqual([
      "books",
      "list",
      "importedAt",
      "desc",
      "",
    ]);
  });
});
