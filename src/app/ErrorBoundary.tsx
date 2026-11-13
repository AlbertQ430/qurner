import { Component, type ErrorInfo, type ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
}

/**
 * 渲染异常时显示中文占位界面，避免用户看到白屏。
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("Qurner 页面渲染失败", error, info.componentStack);
  }

  render(): ReactNode {
    if (this.state.hasError) {
      return (
        <main className="app" role="alert">
          <header className="app-header">
            <h1>Qurner</h1>
          </header>
          <section className="error-placeholder">
            <p>页面出现异常，应用无法继续显示。</p>
            <p>请重新加载 Qurner 后再试。</p>
          </section>
        </main>
      );
    }

    return this.props.children;
  }
}
