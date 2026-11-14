import { QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./app/App";
import { ErrorBoundary } from "./app/ErrorBoundary";
import { createQueryClient } from "./query/query-client";
import "./styles.css";

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("找不到 Qurner 页面挂载节点 #root");
}

const queryClient = createQueryClient();

createRoot(rootElement).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <ErrorBoundary>
        <App />
      </ErrorBoundary>
    </QueryClientProvider>
  </StrictMode>,
);
