# Qurner

离线可用、尊重原作排版的个人电子书阅读器，面向 macOS 14 及以上 Apple Silicon。

## 当前进度

M0–M5 已完成并通过验收，当前可运行 MVP 阅读闭环；M6 性能、可靠性与发布加固尚未启动。

已实现：

- EPUB 2/3 导入、SHA-256 判重、书库搜索/排序与软移除恢复。
- 网格和列表书库、封面缓存，以及 1,000 本规模虚拟化。
- 目录、分页和连续滚动、字号、昼夜主题与沉浸式阅读。
- 阅读进度恢复、书签、正文选择复制和基础键盘操作。
- 内容安全扫描、`epub://` 受限资源加载和 iframe/CSP 隔离。
- 100 MB EPUB 流式导入，100 MB 单章大文本导入耗时满足当前 ≤10 秒门槛。

尚未完成：

- M6 最终性能、可靠性、安全报告和发布流程。
- TXT、PDF、高亮、批注、收藏夹、文件重定位与备份恢复。
- 106 MB 单章正文首次打开约需 32 秒；按需分段渲染和进一步性能优化尚未实施。

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

开发数据保存在应用数据目录；导入仅建立索引，不复制或修改原始 EPUB。

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
npm run tauri build
```

产物位于 `target/release/bundle/macos/Qurner.app`。构建产物、缓存、数据库和私人 EPUB
测试文件均被 `.gitignore` 排除，不进入版本控制。

## 项目文档

- `SRS.md`：产品需求与验收标准
- `SDS.md`：架构、接口、安全与数据设计
- `DEVELOPMENT_PLAN.md`：里程碑、任务和当前执行状态
- `AI_COLLABORATION.md`：Codex/DeepSeek 协作与验收流程
- `docs/tasks/`：任务范围和真实验收记录
