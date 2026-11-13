import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "./App";

describe("App", () => {
  it("渲染 Qurner 标题与简体中文空书库提示", () => {
    render(<App />);

    expect(screen.getByRole("heading", { level: 1, name: "Qurner" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "书库还是空的" })).toBeInTheDocument();
    expect(screen.getByText("稍后可以通过导入添加书籍。")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
