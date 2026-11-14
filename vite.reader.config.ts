import { defineConfig } from "vite";

// 阅读脚本独立构建：无框架 IIFE，作为 Tauri 资源打包。
export default defineConfig({
  build: {
    lib: {
      entry: "src/reader-engine/index.ts",
      formats: ["iife"],
      name: "QurnerReaderEngine",
      fileName: () => "reader-engine.js",
    },
    outDir: "src-tauri/resources",
    emptyOutDir: false,
    minify: "esbuild",
    target: "safari16",
    sourcemap: false,
  },
});
