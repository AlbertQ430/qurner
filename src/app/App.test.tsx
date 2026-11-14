import { QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ImportCommandClient } from "../features/import/ImportFlowProvider";
import type { LibraryCommandClient } from "../features/library/library-client";
import { createQueryClient } from "../query/query-client";
import App from "./App";

const libraryClient: LibraryCommandClient = {
  listBooks: async () => [],
  softRemoveBook: async () => undefined,
};

const importCommandClient: ImportCommandClient = {
  selectImportFiles: async () => [],
  startImport: async () => ({ jobId: "123e4567-e89b-42d3-a456-426614174000" }),
};

describe("App", () => {
  it("渲染 Qurner 标题、书库区域与导入入口", async () => {
    render(
      <QueryClientProvider client={createQueryClient()}>
        <App libraryClient={libraryClient} importCommandClient={importCommandClient} />
      </QueryClientProvider>,
    );

    expect(screen.getByRole("heading", { level: 1, name: "Qurner" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "书库" })).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText("书库还是空的")).toBeInTheDocument();
    });
    expect(screen.getByText("稍后可以通过导入添加书籍。")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "导入 EPUB" })).toHaveLength(2);
  });
});
