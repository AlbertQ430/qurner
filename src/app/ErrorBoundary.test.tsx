import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ErrorBoundary } from "./ErrorBoundary";

function BrokenComponent(): never {
  throw new Error("测试渲染异常");
}

describe("ErrorBoundary", () => {
  it("渲染异常时显示中文错误占位而不是白屏", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});

    render(
      <ErrorBoundary>
        <BrokenComponent />
      </ErrorBoundary>,
    );

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("页面出现异常，应用无法继续显示。")).toBeInTheDocument();
    expect(screen.getByText("请重新加载 Qurner 后再试。")).toBeInTheDocument();
  });
});
