import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri 使用 1420 端口进行开发；构建目标对齐 macOS 14 的 WKWebView。
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**", "**/crates/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "safari16",
    minify: "esbuild",
    sourcemap: false,
  },
});
