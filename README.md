# Qurner

离线可用、尊重原作排版的个人电子书阅读器，面向 macOS 14 及以上 Apple Silicon。

当前处于 M0：只包含最小可启动应用骨架、Rust workspace 与前端测试基线，尚未实现
EPUB 解析、书库或阅读器功能。

## 环境要求

- macOS 14+（Apple Silicon，构建架构 arm64）
- Node.js `>=26 <27`（见 `.nvmrc`）
- Rust `1.100.0`（见 `rust-toolchain.toml`，含 `rustfmt`、Clippy）
- Xcode Command Line Tools

## 安装依赖

```bash
npm install
npx playwright install webkit
```

Rust 工具链可通过 rustup 安装：

```bash
rustup toolchain install 1.100.0 --component rustfmt clippy --target aarch64-apple-darwin
```

## 开发

```bash
npm run tauri dev
```

窗口标题应为 `Qurner`，页面显示简体中文空书库提示。

## 检查与测试

```bash
npm run check        # TypeScript 类型检查 + Biome + Vitest
npm run test:e2e     # Playwright WebKit 冒烟测试
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 构建

```bash
npm run tauri build -- --bundles app
```

产物位于 `src-tauri/target/release/bundle/macos/Qurner.app`。构建产物、缓存、数据库和
私人 EPUB 测试文件均被 `.gitignore` 排除，不进入版本控制。
