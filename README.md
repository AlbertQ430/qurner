# Qurner

离线可用、尊重原作排版的个人电子书阅读器，面向 macOS 14 及以上 Apple Silicon。

## 当前进度

M0–M6 已完成并通过验收：MVP 阅读闭环、错误与离线加固、性能/恢复/兼容性及发布候选
验收均已完成。当前构建为发布候选
（`Qurner.app`，版本 0.1.0，arm64，未签名/未公证），发布由用户决定。

已实现：

- EPUB 2/3 导入、SHA-256 判重、书库搜索/排序与软移除恢复。
- 网格和列表书库、封面缓存，以及 1,000 本规模虚拟化。
- 目录、分页和连续滚动、字号、昼夜主题与沉浸式阅读。
- 阅读进度恢复、书签、正文选择复制和基础键盘操作。
- 内容安全扫描、`epub://` 受限资源加载和 iframe/CSP 隔离。
- 100 MB EPUB 流式导入，100 MB 单章大文本导入耗时满足当前 ≤10 秒门槛。

已知限制（非 MVP 阻塞，见 `docs/acceptance/TASK-M6C-REPORT.md`）：

- 106 MB 单章正文首次打开约需 32 秒；按需分段渲染和进一步性能优化尚未实施。
- 翻页性能以 60 Hz 外部录制口径验收（≥55 fps；+16.7 ms 量化保守值，本轮 p95 69.20 ms）。
- 真实整机关网未执行；使用进程级 `sandbox-exec` 断网与 `lsof` 无连接的组合证据。
- macOS 14 实机实测由用户明确豁免（2026-11-18）；当前证据为 macOS 26.6 运行 +
  `LSMinimumSystemVersion=14.0` / `minos 14.0` 部署目标，风险由发布方承担。
- 夜间主题切换瞬态已修复（先同步正文底色/文字色再切换主题类），并经 20 次切换验证。
- TXT、PDF、高亮、批注、收藏夹、文件重定位与备份恢复属于 V1.x/V2.x。
- 发布候选未签名、未公证；本仓库不包含自动更新或发布流程。

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

## 隐私与数据

- 完全离线：无遥测、无分析、无远程字体、无云同步；运行期 `lsof -a -p <pid> -i` 无连接，
  跨域资源在阅读器内被 CSP/资源清洗阻断。
- 书库、进度、设置和书签保存在本机应用数据目录的 SQLite 数据库；导入只建立索引，
  不复制、移动或改写原始 EPUB。
- 原始电子书与个人阅读数据不进入版本控制；测试使用隔离 HOME 与合成 EPUB。

## 发布候选产物

当前发布候选来自仓库构建（`npm run tauri build`）：

- `target/release/bundle/macos/Qurner.app`，7.9 MB，Mach-O arm64；
- Bundle Identifier `com.qurner.reader`，版本 0.1.0；
- 可执行文件 SHA-256 `f9c0909abdf0a18785217d8b3f6926e313f527eaafda4fcb5db8004511a515da`
  （构建后重新计算；签名/公证与发布不在本仓库流程内）。

## 项目文档

- `SRS.md`：产品需求与验收标准
- `SDS.md`：架构、接口、安全与数据设计
- `DEVELOPMENT_PLAN.md`：里程碑、任务和当前执行状态
- `AI_COLLABORATION.md`：Codex/DeepSeek 协作与验收流程
- `docs/tasks/`：任务范围和真实验收记录
