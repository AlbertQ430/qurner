# TASK-M0：工具链与项目骨架

**状态：** ready_for_review  
**里程碑：** M0  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** 无

## 1. 目标

在现有文档仓库中建立可启动、可通过 CLI 检查的 Qurner MVP 工程骨架，为 M1 的 EPUB Core 开发提供稳定基线。

M0 完成后，`Qurner` 应能启动并显示一个最小空书库界面；前端、Rust workspace、Tauri、格式和测试命令全部可执行。

## 2. 用户授权

用户在 2026-11-14 明确同意开始 M0。该授权允许 DeepSeek：

- 通过 `rustup` 安装 Rust stable 1.100.0、`rustfmt`、Clippy 和
  `aarch64-apple-darwin` target。
- 执行 `npm install` 安装项目本地依赖。
- 执行 `npx playwright install webkit` 安装 Playwright WebKit。
- 在本任务允许的路径内创建和修改应用工程文件。

该授权不包括：

- 安装完整 Xcode。
- 修改全局 npm 配置。
- 安装与 M0 无关的全局软件。
- 提交、推送或发布。

## 3. 范围

### 必须完成

- Tauri 2.x stable + React 19 + TypeScript 工程。
- `Qurner` 产品名和 `com.qurner.app` Bundle Identifier。
- 根 Rust workspace，以及 `epub-core`、`storage-core`、`reader-model` 三个可测试 crate。
- TypeScript strict、Biome、Vitest、React Testing Library、Playwright WebKit。
- Cargo `rustfmt`、Clippy 和 workspace 测试基线。
- Tauri capabilities 最小化，不引入 shell、HTTP 或通用文件系统插件。
- 最小应用壳、前端错误边界、简体中文空书库占位页。
- Rust 后端最小 `tracing` 初始化，日志先输出到标准错误。
- `.gitignore`，排除构建产物、缓存、数据库、私人 EPUB 和 `.DS_Store`。
- npm 与 Cargo 锁文件。
- 开发启动和构建说明，可放在 `README.md`。

### 不属于本任务

- EPUB 解析、ZIP、SHA-256、OPF、目录或封面。
- SQLite、数据库迁移、Repository、导入或书库数据。
- 自定义协议、iframe、阅读脚本、CSP nonce 和分页。
- 文件选择器、拖放、设置、进度、书签、主题或沉浸式阅读。
- 任何 P1/P2 功能。

## 4. 需求追踪

| 类型 | 编号 | M0 验收要求 |
|---|---|---|
| FR | 无 | M0 不交付 SRS 功能需求 |
| AC | 无 | M0 不声明功能 AC 通过 |
| NFR | NFR-001 | 应用骨架以 macOS 14+ Apple Silicon 为目标，构建架构为 arm64 |
| NFR | NFR-011 | M0 应用运行时不发网络请求；开发工具安装网络不属于产品运行时 |
| Plan | M0 | 空应用可启动，全部规定检查可执行 |

## 5. 设计约束

- 遵守 `SDS.md` 第 2、3、4、14 节。
- 前端入口使用 React 19 + TypeScript，不使用 JavaScript 源码。
- 后端使用 Rust Edition 2024。
- 根 `Cargo.toml` 是 workspace，包含三个领域 crate 和 `src-tauri`。
- Tauri 配置：
  - `productName`: `Qurner`
  - `identifier`: `com.qurner.app`
  - `window.title`: `Qurner`
  - 前端开发命令：`npm run dev`
  - 前端构建命令：`npm run build`
- `src-tauri/capabilities/default.json` 只授予 M0 需要的最小核心权限，不加入
  `shell`、`http`、`fs` 或 `dialog` 插件权限。
- React 页面不调用任意 Tauri 命令，不读取文件系统。
- M0 的数据库、EPUB 和阅读器目录只建立模块边界，不实现占位业务逻辑。
- 统一产品显示文字使用 `Qurner`，不得出现旧名称。

## 6. 文件权限

### 允许修改或创建

- `.gitignore`
- `.nvmrc`
- `README.md`
- `package.json`
- `package-lock.json`
- `index.html`
- `tsconfig.json`
- `tsconfig.app.json`
- `tsconfig.node.json`
- `vite.config.ts`
- `biome.json`
- `vitest.config.ts`
- `playwright.config.ts`
- `Cargo.toml`
- `Cargo.lock`
- `rust-toolchain.toml`
- `src/`
- `src-tauri/`
- `crates/`
- `tests/`
- `scripts/`
- 本任务第 10 节 DeepSeek 开发报告部分

### 禁止修改

- `SRS.md`
- `SDS.md`
- `DEVELOPMENT_PLAN.md`
- `AI_COLLABORATION.md`
- `AGENTS.md`
- `docs/tasks/TEMPLATE.md`
- 本任务第 1–9 节和第 11 节
- 仓库外任何用户文件
- `~/Desktop/SRS.md` 或 `~/Desktop` 下其他项目

## 7. 行为与边界

### 应用启动

- 双击构建后的 `Qurner.app` 或运行开发命令后，窗口标题显示 `Qurner`。
- 页面显示 Qurner 标题和简体中文空书库状态。
- 没有书籍时应说明可稍后通过导入添加书籍，但不显示不可用的导入按钮。
- 控制台允许显示开发日志，不显示用户路径或敏感数据。

### 错误边界

- React 渲染异常时显示中文错误占位界面，而不是白屏。
- Rust 初始化失败时记录错误并退出，不创建业务数据。
- M0 不实现完整错误码系统，后续任务按 SDS 实现。

### 退出与资源

- 正常关闭窗口后进程退出。
- 不创建数据库、书籍缓存或用户个人数据。
- M0 运行不发起网络请求。

## 8. 测试要求

### 前端

- Vitest + React Testing Library 至少一个测试：
  - 页面渲染 `Qurner`。
  - 展示空书库中文提示。
- Playwright WebKit 至少一个冒烟测试：
  - 启动前端预览页面。
  - 标题和空书库提示可见。

### Rust

- `epub-core`、`storage-core`、`reader-model` 各有一个最小 smoke test，
  证明 crate 可被 workspace 编译和测试。
- 不创建无意义业务占位接口。

### 配置

- TypeScript 启用 `strict`。
- `npm run check` 必须同时执行类型检查、Biome 检查和 Vitest。
- Tauri capabilities 不包含 shell、http、fs 或 dialog 权限。
- `.gitignore` 覆盖：
  - `node_modules/`
  - `dist/`
  - `target/`
  - `.DS_Store`
  - `*.sqlite3`
  - 本地环境文件
  - 私人 EPUB 测试文件

## 9. 必跑命令与验收证据

DeepSeek 必须按顺序运行并记录真实退出码和结果摘要：

```bash
rustup show
rustc --version
cargo --version

npm install
npx playwright install webkit

npm run check
npm run test:e2e
npm run build

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

npm run tauri build -- --bundles app
```

还要执行一次开发启动冒烟：

```bash
npm run tauri dev
```

启动后确认 Qurner 窗口和空书库界面出现，然后正常关闭应用并结束命令。报告中说明启动是否成功、窗口标题和可见内容。

构建产物不得提交。报告中给出最终 `.app` 的实际绝对路径，供 Codex 独立验收。

## 10. DeepSeek 开发报告

> DeepSeek 开始后把顶部状态改为 `in_progress`，完成后填写本节并改为
> `ready_for_review`。

**完成摘要：**

在现有文档仓库中建立了可启动、可通过 CLI 检查的 Qurner MVP 工程骨架：

- Tauri 2.12.5 + React 19.3 + TypeScript 5.9 + Vite 7 工程，产品名 `Qurner`，Bundle
  Identifier `com.qurner.app`，窗口标题 `Qurner`，构建目标 macOS 14+ / arm64。
- 根 Cargo workspace（Rust 1.100.0，Edition 2024，resolver 3），成员为
  `epub-core`、`storage-core`、`reader-model` 三个领域 crate 和 `src-tauri`；三个 crate
  各含一个 smoke test，不含业务占位接口。
- `src-tauri` 提供最小应用壳：`tracing` 初始化到标准错误、初始化失败时记录并退出、
  启动时输出领域 crate 链接信息。
- 前端最小应用壳、错误边界、简体中文空书库占位页；空书库页不显示导入按钮。
- TypeScript `strict`、Biome、Vitest + React Testing Library、Playwright WebKit 冒烟
  测试全部配置完成；`npm run check` 串联类型检查、Biome 与 Vitest。
- Tauri capabilities 仅授予 `core:default`，不含 shell、http、fs、dialog 插件权限。
- `.gitignore` 覆盖依赖、构建产物、缓存、数据库、本地环境文件、私人 EPUB 与
  `.DS_Store`；`.nvmrc`、`README.md`、npm 与 Cargo 锁文件已生成。

**修改文件：**

- 根配置：`.gitignore`、`.nvmrc`、`README.md`、`package.json`、`package-lock.json`、
  `index.html`、`tsconfig.json`、`tsconfig.app.json`、`tsconfig.node.json`、
  `vite.config.ts`、`vitest.config.ts`、`playwright.config.ts`、`biome.json`、
  `Cargo.toml`、`Cargo.lock`、`rust-toolchain.toml`
- 前端：`src/main.tsx`、`src/styles.css`、`src/vite-env.d.ts`、`src/test-setup.ts`、
  `src/app/App.tsx`、`src/app/ErrorBoundary.tsx`、`src/app/App.test.tsx`、
  `src/app/ErrorBoundary.test.tsx`、`src/features/library/EmptyLibrary.tsx`
- E2E：`tests/e2e/empty-library.spec.ts`
- Rust crate：`crates/epub-core/{Cargo.toml,src/lib.rs}`、
  `crates/storage-core/{Cargo.toml,src/lib.rs}`、
  `crates/reader-model/{Cargo.toml,src/lib.rs}`
- Tauri：`src-tauri/Cargo.toml`、`src-tauri/build.rs`、`src-tauri/src/main.rs`、
  `src-tauri/src/lib.rs`、`src-tauri/tauri.conf.json`、
  `src-tauri/capabilities/default.json`、`src-tauri/icons/`（`icon.png`、`icon.icns`、
  `32x32.png`、`128x128.png`、`128x128@2x.png`）
- 任务文件：`docs/tasks/TASK-M0.md`（顶部状态与本开发报告）
- 意外格式影响（详见「风险与待决问题」）：`SRS.md`、`SDS.md`、
  `DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、`docs/tasks/TEMPLATE.md` 以及
  `TASK-M0.md` 第 1–9、11 节仅被 Biome 的 Markdown 格式化器改写了表格对齐空白，
  文字内容未变；这些文件不在本任务的允许修改清单内。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `rustup show` | 0 | active toolchain 为 `1.100.0-aarch64-apple-darwin`，已安装 target `aarch64-apple-darwin` |
| `rustc --version` | 0 | `rustc 1.100.0 (0ef1bc386 2026-11-09)` |
| `cargo --version` | 0 | `cargo 1.100.0 (3acc79c0c 2026-11-05)` |
| `npm install` | 0 | 安装 167 个包并生成 `package-lock.json`；首次直接运行因沙箱无法写 `~/.npm` 缓存失败（EPERM），改用 `npm install --cache /tmp/qurner-npm-cache` 后成功（缓存目录在工作区外临时区，未写入仓库） |
| `npx playwright install webkit` | 0 | WebKit 27.2（playwright webkit v2388）与 ffmpeg 安装到 `~/Library/Caches/ms-playwright`；首次被文件沙箱阻止（EPERM），以已授权的写权限重试后成功 |
| `npm run check` | 0 | `tsc -b` 通过；Biome 检查 29 个文件无待修复内容；Vitest 2 个测试文件、2 个测试全部通过。首次运行因缺少 `vite-env.d.ts` 报 `Cannot find module './styles.css'`，补齐后通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 1 个测试通过（6.2s）：标题 `Qurner`、空书库中文提示可见、无按钮。首次因 webServer 健康检查使用 `127.0.0.1` 而 Vite 仅监听 `localhost` 超时失败，改为 `localhost` 后通过 |
| `npm run build` | 0 | 前端产物生成，31 modules，`dist/assets/index-*.js` 223.70 kB（gzip 69.98 kB） |
| `cargo fmt --all --check` | 0 | 首次退出码 1，发现 `src-tauri/src/lib.rs` 两处格式差异；执行 `cargo fmt --all` 后再次检查通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 首次退出码 101：`init_tracing` 中 `try_init()?` 的错误类型无法转换为 `Box<dyn Error>`；改为 `Box<dyn Error + Send + Sync>` 后全部通过，无 warning |
| `cargo test --workspace` | 0 | `epub-core`、`storage-core`、`reader-model` 各 1 个 smoke test 通过；`qurner` 库与 doc-tests 通过（0 个测试） |
| `npm run tauri build -- --bundles app` | 0 | 首次退出码 1：`failed to build bundler settings: invalid category`（`bundle.category` 值不被接受），移除该可选字段后成功；产出 `Qurner.app` 3.85 MiB |
| `npm run tauri dev` | 0 | 编译并启动成功，日志输出 `Qurner 启动 version="0.1.0"` 与领域 crate 链接信息；进程运行，SIGTERM 关闭后应用进程退出、1420 端口释放 |

**M0 验证：**

- 应用启动：`npm run tauri dev` 启动成功，Vite 监听 1420，`target/debug/qurner` 进程
  运行；日志确认 `Qurner 启动 version="0.1.0"` 与 `领域 crate 已链接`（epub-core /
  storage-core / reader-model）。构建后的 `Qurner.app` 二进制也能启动并输出同样日志。
- 窗口标题与可见内容：在用户授予屏幕录制授权后，用 `open` 通过 LaunchServices 启动
  构建的 `Qurner.app`，并 `screencapture` 截图确认：菜单栏应用名为 `Qurner`，窗口
  标题栏为 `Qurner`，页面显示 `Qurner`、`个人电子书阅读器`、`书库还是空的` 与
  `稍后可以通过导入添加书籍。`（系统深色外观）。System Events 读取窗口标题仍报
  权限违例，故以截图为准。`Info.plist` 的 `CFBundleName` / `CFBundleDisplayName`
  均为 `Qurner`、`CFBundleIdentifier` 为 `com.qurner.app`、`LSMinimumSystemVersion`
  为 `14.0`。
- 页面内容：Playwright WebKit 冒烟测试验证页面标题 `Qurner`、一级标题 `Qurner`、
  「书库还是空的」与「稍后可以通过导入添加书籍。」可见，且不存在按钮。
- 错误边界：Vitest 测试验证子组件抛错时显示中文错误占位（`role="alert"`），而非白屏。
- 退出与资源：终止应用进程后进程退出、开发端口释放；未创建 SQLite 数据库或书籍缓存。
- Bundler：capabilities 的 `permissions` 仅为 `["core:default"]`，没有 shell / http /
  fs / dialog 权限；`Info.plist` 与 `lipo -archs` 确认二进制为 arm64。
- 网络：代码中没有网络调用，未安装 HTTP 插件；未使用抓包工具验证零出站请求。

**构建产物路径：**

`/Users/quzhuoyan/Desktop/qurner/target/release/bundle/macos/Qurner.app`

（3.85 MiB，arm64，ad-hoc 签名，`CFBundleIdentifier = com.qurner.app`；
`target/` 已被 `.gitignore` 排除，构建产物未提交。）

**未验证项：**

- 未使用网络监测工具验证运行时零出站请求。
- 未在 macOS 14 实机验证，仅在本机 macOS 26.6 / arm64 上构建和运行。
- 「正常关闭窗口后进程退出」以 SIGTERM 结束进程验证，未用 UI 点击关闭按钮或 Cmd+Q。
- 直接执行 `Qurner.app/Contents/MacOS/qurner` 时，会话文件沙箱阻止 WebKit 写
  `~/Library/WebKit` 缓存目录并产生 NSCocoaError 日志；通过 `open` 启动时截图显示界面
  正常，但该沙箱限制未在无沙箱的普通用户环境复核。

**风险与待决问题：**

- **意外修改了任务禁止修改的文档（需要用户/Codex 决定处理方式）**：为修正 Biome
  报告，执行了 `npx biome check --write .`。Biome 2.6.7 会处理 Markdown 文件，因此
  除前端源码外，`SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、
  `docs/tasks/TEMPLATE.md` 和 `TASK-M0.md` 第 1–9、11 节的表格被改写为对齐格式
  （表格单元格补空格、分隔行从 `|---|` 变为 `| --- |`），文件字节数和修改时间随之
  变化。未删除或改写任何文字、条款、数字或链接；但确实违反了第 6 节「禁止修改」。
  仓库没有 Git 提交，无法逐字节回滚；请用户或 Codex 决定是保留该空白格式变化，还是
  从原稿恢复这些文档。已通过 `biome.json` 的 `files.includes` 将 Biome 检查范围限制
  为前端源码与配置，避免再次发生。
- `npm audit` 报告 3 个开发依赖漏洞（1 moderate、2 critical，均来自 Vitest 3.2.7 /
  tinypool），不影响应用运行时；任务未授权升级 major 版本，未擅自处理，建议后续任务
  评估 Vitest 4 迁移。
- 当前 `tauri.conf.json` 的 CSP 为 `null`。M0 不涉及 iframe 与书内资源，M3 前需按
  SDS 第 6、11 节实现 CSP nonce 与资源限制。
- 图标为占位设计（深绿圆角底 + 金色「Q」），未做正式品牌设计，后续可按需替换。
- 受限沙箱下 `npm install` 需要显式 `--cache`，普通开发环境直接 `npm install` 即可；
  该差异不改变工程配置。
- 首次运行各命令暴露的环境/配置问题均已修复，未发现遗留的跳过测试或 TODO。

## 11. Codex 审查

> Codex 仅在任务进入 `ready_for_review` 后填写。

**结论：** 待审核  
**审查日期：**

**独立执行的检查：**

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|

**M0 结果：**

**返工或后续任务：**
