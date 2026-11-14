import { QueryClient } from "@tanstack/react-query";

import type { BookSort, SortDirection } from "../bridge/ipc-schemas";

export interface BookListQueryInput {
  query?: string;
  sort: BookSort;
  direction: SortDirection;
}

/** 集中定义的 query key；组件不得散落字符串。 */
export const queryKeys = {
  books: ["books"] as const,
  bookList: (input: BookListQueryInput) =>
    [...queryKeys.books, "list", input.sort, input.direction, input.query ?? ""] as const,
};

/** 应用 QueryClient：本地命令结果，不轮询、不联网、失焦不重取。 */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        refetchOnWindowFocus: false,
      },
    },
  });
}
