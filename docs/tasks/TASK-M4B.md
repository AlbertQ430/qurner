# TASK-M4B：系统文件选择、批量导入与结果汇总

**状态：** accepted  
**里程碑：** M4  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** TASK-M4A accepted

## 1. 目标

用户可以从 Qurner 主界面打开 macOS 系统文件选择器，选择一本或多本 EPUB，导入期间看到
真实进度，结束后看到逐项成功、恢复、重复和失败结果及可处理的中文原因。

## 2. 范围

### 必须完成

- 新增 Rust 自定义命令 `selectImportFiles(): Promise<string[]>`，内部使用
  `tauri-plugin-dialog` 的 Rust API 打开系统原生多选文件对话框。
- 注册 dialog 插件，但保持前端 capability 只有 `core:default`；前端不得直接调用
  dialog 插件命令。
- 在 M4A 命令桥中增加严格校验的 `selectImportFiles` wrapper。
- 接入 TanStack Query 5，提供单一 `QueryClientProvider` 和固定 query key。
- 增加“导入 EPUB”入口、选择/导入中的禁用状态、进度、结束汇总和可恢复错误提示。
- 结束汇总逐项显示安全文件名（`sourceName`）和本地化状态，仍不显示完整路径。
- 单本、批量、取消、无文件、超过 1,000 本、部分失败、全部失败和重复导入均有测试。

### 不属于本任务

- 书库网格/列表、搜索、排序、封面、软移除确认和书籍打开。
- 拖放导入、目录导入、后台扫描或文件系统监控。
- 导入取消、断点续传、导入历史持久化。
- 修改 Rust 导入语义、SQLite schema、事件载荷、Reader Engine 或 iframe 协议。
- 真实系统文件对话框的自动化操控或完整 `Qurner.app` 人工冒烟；由 M4D 覆盖。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收 |
|---|---|---|
| FR | FR-010–FR-014 | 通过系统文件选择器导入 1 本或多本；只使用用户明确选择的文件，不扫描目录、不复制原文件 |
| FR | FR-016–FR-019 | 展示重复、恢复、逐项失败和可理解原因；不覆盖已有数据 |
| AC | AC-010 | 系统选择器可选 1 本和多本，取消不启动导入，不开后台扫描 |
| AC | AC-011 | 结束时显示成功、恢复、重复、失败汇总和逐项状态 |
| NFR | NFR-011 | 不发起网络请求，不直接向前端开放 HTTP、shell、fs 或 dialog capability |
| NFR | NFR-012 | 选择器、启动导入和逐项失败均显示原因及下一步处理 |

## 4. 已批准依赖

用户在 2026-11-14 明确批准：

| 依赖 | 用途 | 使用边界 |
|---|---|---|
| `tauri-plugin-dialog = "2"` | Rust API 打开 macOS 原生文件选择器 | 仅注册 Rust API；不得安装或调用 `@tauri-apps/plugin-dialog`，capability 不得增加 `dialog:*` |
| `@tanstack/react-query = "^5.107.1"` | 管理本地命令结果、失效和异步状态 | 不轮询、不联网，不把文件路径或私密正文放入 query cache |

当前解析基线为 `tauri-plugin-dialog 2.8.4`（Apache-2.0 OR MIT）和
`@tanstack/react-query 5.107.1`（MIT）。锁文件必须记录实际解析版本。除这两个依赖外，
本任务不得新增第三方包或 Tauri plugin。

## 5. 设计与行为约束

### 5.1 文件选择命令

前端公开契约：

```ts
selectImportFiles(): Promise<string[]>
```

- invoke 名为 `selectImportFiles`，不发送参数。
- 使用 `tauri-plugin-dialog` 的 Rust API，启用多选并仅显示 `epub` 文件过滤项。
- 取消返回 `[]`，不得当作错误，也不得调用 `startImport`。
- 用户确认后返回 1–1,000 个原始路径，保持选择顺序；不得 trim、规范化或改写路径。
- 空白路径、超过 1,000 个路径或无法无损转换为 UTF-8 的路径返回
  `E_INVALID_INPUT`，不得静默丢弃或做有损转换。
- dialog API 平台错误统一返回固定 `E_INTERNAL`，不得把底层错误、路径或系统详情传给前端
  或写入日志。
- 不得调用目录扫描、`fs::read`、文件元数据遍历或后台监控；选中的文件只在
  `startImport` 中按既有后端流程处理。
- 文件选择器等待方式不得阻塞 Tauri UI/event loop。优先使用插件异步回调/通道；
  若使用阻塞式 Rust API，必须放到 blocking 任务，并证明 UI 仍可重绘。

### 5.2 Dialog 注册与权限

- 在 Tauri builder 注册 `tauri_plugin_dialog::init()`。
- 在 `invoke_handler!` 注册自定义 `selectImportFiles`。
- `src-tauri/capabilities/default.json` 必须继续只包含 `core:default`。
- 现有“无插件/无网络依赖”测试应改为：允许且只允许 `tauri-plugin-dialog`，继续拒绝
  `plugin-http`、`plugin-shell`、`plugin-fs`、`reqwest`、`hyper` 和网络 socket 调用。
- 不修改 CSP 或放宽 iframe 安全边界。

### 5.3 前端命令桥

- 在 `src/bridge/ipc-schemas.ts` 增加严格 `selectImportFilesOutputSchema`：
  `string[]`，长度 0–1,000，每个元素非空白。
- 在 `src/bridge/commands.ts` 的 `CommandBridge` 增加 `selectImportFiles()`。
- 输出非法、Tauri Error 或非 AppError rejection 按 M4A 规则转 `E_INTERNAL`。
- 返回合法 AppError 时保留 code/message/details。
- 不记录或回显调用参数。文件路径只能存在于当前导入流程的内存中。

### 5.4 TanStack Query

- 在应用根部提供单一 `QueryClientProvider`。
- 默认 `queries.retry=false`、`refetchOnWindowFocus=false`；不使用轮询。
- 在集中模块定义 query key，例如 `queryKeys.books`，不得在组件内散落字符串。
- `startImport` 的启动调用使用 Query mutation 或等价受控异步状态；导入进度和完成仍由
  M4A 事件桥提供，不把逐项事件塞进 query cache。
- `import-completed` 后失效 `queryKeys.books`，为 M4C 书库刷新建立唯一入口。
- query cache、DOM、日志和 URL 中不得出现完整选择路径。

### 5.5 导入流程与事件竞态

导入流程状态至少区分：`idle`、`selecting`、`importing`、`completed`、`error`。

- 点击导入后立即进入 `selecting`，按钮禁用；取消后回到 `idle`。
- 获得非空路径数组后，必须先成功订阅导入事件，再调用 `startImport`。
- `startImport` 返回 `jobId` 前的进度/完成事件必须暂存；得到 jobId 后只处理该 job 的
  事件，防止后端过快发出首条事件导致 UI 丢进度。
- 其他 jobId 的迟到事件不得改变当前汇总。
- 当前 job 的 `import-completed` 到达后停止订阅、停止进度状态、展示汇总并失效书库 query。
- `selectImportFiles`、订阅或 `startImport` 失败时停止订阅、显示固定可理解错误并允许重试。
- 导入进行中必须禁用再次导入；不得启动第二个 UI 导入 job。
- 取消汇总后回到 `idle`，不保留路径。

### 5.6 用户界面

- 主界面和空书库状态均提供可键盘聚焦的“导入 EPUB”按钮，accessible name 明确。
- 选择中显示“正在打开文件选择器…”；导入中显示 `已完成 X / Y` 和只读进度条。
- 汇总以 modal/dialog 语义显示；键盘可关闭，关闭按钮有明确名称。
- 汇总计数必须覆盖：
  - 已导入：`imported`
  - 已恢复：`reactivated`
  - 重复：`duplicate`
  - 失败：`failed`
- 逐项列表只显示 `sourceName`、本地化状态和失败原因；不得显示 `/Users/...`、
  文件 URL 或 stack。
- 汇总必须可处理空批次之外的零失败与混合结果。
- 界面文本使用简体中文。

### 5.7 错误文案

选择器/启动流程错误至少映射为：

- `E_INVALID_INPUT`：本次选择无效或超过 1,000 本，请重新选择。
- `E_INTERNAL`：无法打开文件选择器或启动导入，请重试。

逐项失败至少映射为：

| code | 用户可见说明 |
|---|---|
| `E_FILE_NOT_FOUND` | 文件已被移动或删除，请重新选择。 |
| `E_FILE_UNREADABLE` | 无法读取文件，请检查文件权限后重新选择。 |
| `E_EPUB_INVALID` | 文件不是有效或完整的 EPUB，请检查文件。 |
| `E_EPUB_ENCRYPTED` | 书籍受 DRM 或加密保护，Qurner 不支持。 |
| `E_EPUB_FIXED_LAYOUT` | 固定版式 EPUB 暂不支持。 |
| `E_EPUB_VERTICAL` | 竖排 EPUB 暂不支持。 |
| `E_EPUB_SCRIPT_REQUIRED` | 书籍依赖脚本，Qurner 出于安全原因不支持。 |
| `E_SOURCE_CHANGED` | 导入期间文件发生变化，请重新选择。 |
| `E_STORAGE` | 无法保存书库数据，请确认磁盘空间后重试。 |

其他 code 使用“无法导入该文件，请重试。”，不得直接渲染未知底层 message。

## 6. 文件权限

### 允许修改

- `src/bridge/` 下的命令 schema、wrapper 和测试
- `src/app/`、`src/features/import/`、`src/features/library/EmptyLibrary.tsx`
- `src/query/` 或等价集中 QueryClient/query key 模块
- `src/main.tsx`、`src/styles.css`
- `package.json`、`package-lock.json`
- `src-tauri/Cargo.toml`、根 `Cargo.lock`
- `src-tauri/src/lib.rs`、`src-tauri/src/commands.rs` 或任务专用 dialog 模块
- `src-tauri/tests/` 中与本任务直接相关的测试
- `src-tauri/gen/schemas/`（仅允许 Tauri 构建自动生成的权限元数据变化）
- 本任务状态和第 10 节开发报告

### 禁止修改

- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、ADR 和其他任务
- `crates/`、数据库 schema 和既有导入/查询语义
- `src/reader-engine/`、`src/bridge/messages.ts|parent.ts|iframe.ts`
- `src-tauri/capabilities/default.json`、CSP、网络或 fs/shell 权限
- 不得安装 `@tauri-apps/plugin-dialog`、Zustand、Router 或其他未批准依赖

如 dialog Rust API 在保持 `core:default` 的前提下无法工作，状态改为 `blocked` 并说明
最小证据；不得自行扩大前端权限。

## 7. 测试要求

### Rust

- 纯选择结果验证：空、1、1,000、超过 1,000、空白和保持顺序。
- 非 UTF-8 路径（Unix/`OsStringExt`）拒绝而不是有损转换。
- 插件和自定义命令已注册；capability 仍严格为 `core:default`。
- 依赖静态检查只允许 `tauri-plugin-dialog`，继续拒绝网络和 fs/shell/http 插件。
- 无目录扫描、文件内容读取或路径日志。

真实系统 dialog 不要求 Rust mock 自动点击，但必须通过 `cargo build`/`tauri build` 证明
注册和链接成功；真实交互由 M4D 冒烟覆盖。

### 前端

- `selectImportFiles` invoke 名、无参数、空数组、1/1,000 项和非法输出。
- 选择取消不调用 `startImport`。
- 单本成功、批量混合 imported/reactivated/duplicate/failed。
- 先订阅后 invoke；startImport 返回前的事件被暂存，返回后正确重放。
- 其他 jobId 和取消订阅后的迟到事件被忽略。
- 汇总计数、逐项 sourceName、本地化状态和可处理错误文案。
- 完整路径不出现在 DOM、console spy、query cache 或错误文案中。
- selecting/importing 时导入按钮禁用，结束后汇总可关闭并回到可导入状态。
- QueryClient 默认 retry/focus refetch 配置；完成事件失效 `queryKeys.books`。
- 文件选择失败、订阅失败、startImport 失败均停止加载并允许重试。

测试不得依赖真实 Tauri、真实原生 dialog 或用户文件；使用可注入 command client、
event subscriber 和 QueryClient。

## 8. 必跑命令

```bash
npm run check
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri build
```

## 9. 禁止事项

- 不启动 M4C 或提前实现书库、搜索、排序、封面或软移除 UI。
- 不调用目录扫描、文件监控、原始文件读取或复制。
- 不记录、展示或持久化完整路径。
- 不修改事件 DTO、导入并发规则、错误稳定性或数据库。
- 不修改 capability 或让前端直接调用 dialog 插件。
- 不新增已批准列表之外的依赖。
- 不创建 commit、推送或发布。

## 10. DeepSeek 开发报告

> 仅在开始实现后填写。开始实现时将状态改为 `in_progress`。

**完成摘要：**

实现系统文件选择、批量导入进度与结果汇总；依赖严格限制在已批准的
`tauri-plugin-dialog`（Rust）与 `@tanstack/react-query`。

第 11 节复验返工修复：

1. **async act 干净**：`clickImport()` 使用 async `act` 包裹点击并等待异步状态更新
   （0ms 定时器 flush 微任务），取消、选择错误与第二次导入场景的
   `not wrapped in act(...)` 警告全部消失；测试使用 `@testing-library/react` 的 `act`，
   `src/test-setup.ts` 设置 `IS_REACT_ACT_ENVIRONMENT = true`；
   `npx vitest run` 全量 60 tests 的 stderr 无 act/Warning 输出。
2. **dialog 注册验证测试**：新增 `src-tauri/tests/m4_dialog.rs`（3 passed），在不启动
   真实对话框的前提下检查：`tauri_plugin_dialog::init()` 与
   `commands::select_import_files` 已注册、命令名 `selectImportFiles`、EPUB 过滤、
   `blocking_pick_files()`、`spawn_blocking`、`validate_selected_files` 存在；
   capability 仍严格 `core:default` 且无 `dialog:*`；commands 无目录扫描/读文件/路径
   日志。
3. **全部失败汇总测试**：新增 all-failed 批次用例，断言“失败 2”、其余三项为 0、
   两种失败 code 的安全文案显示、底层 message 不渲染、关闭汇总后可再次导入。

- **Rust**：
  - 新增 `selectImportFiles` 自定义命令：`tauri-plugin-dialog` 的
    `blocking_pick_files()` 放入 `spawn_blocking`，不阻塞 UI/event loop；仅 EPUB
    过滤、多选；取消返回 `[]`。
  - `validate_selected_files` 纯函数：0（取消）或 1–1,000 个路径；空白路径、非 UTF-8
    路径、超过 1,000 个均返回 `E_INVALID_INPUT`；保持选择顺序，不 trim/规范化。
  - 注册 `tauri_plugin_dialog::init()` 与命令；`capabilities/default.json` 仍只有
    `core:default`，前端不获得 `dialog:*`。
  - 更新 M2/M3 依赖静态检查：只允许 `tauri-plugin-dialog`，继续拒绝
    `plugin-http/shell/fs`、`reqwest`、`hyper` 与网络 socket。
  - 新增单元测试：空、1、1,000、1,001、空白、顺序保持、非 UTF-8（`OsStringExt`）。
- **前端命令桥**：`selectImportFilesOutputSchema`（0–1,000 非空白字符串）、
  `CommandBridge.selectImportFiles()`（无参数 invoke、非法输出→`E_INTERNAL`）。
- **TanStack Query**：`src/query/query-client.ts` 集中 `queryKeys.books` 与
  `createQueryClient()`（retry=false、refetchOnWindowFocus=false）；`main.tsx` 单一
  `QueryClientProvider`。
- **导入流程**：`ImportFlowProvider` 状态机 `idle/selecting/importing/completed/error`；
  先订阅事件再 `startImport`；返回 jobId 前的事件暂存、返回后只重放当前 job；其他
  jobId 与完成后迟到事件忽略；completed 后停止订阅、失效 `queryKeys.books`；选择/
  订阅/启动失败停止加载并可重试；导入中禁用第二次导入；取消回到 idle。
- **UI**：主界面与空书库均提供“导入 EPUB”按钮；选择中显示“正在打开文件选择器…”，
  导入中显示“已完成 X / Y”和只读进度条；完成后显示 modal 汇总（四项计数、逐项
  sourceName/本地化状态/失败原因），Escape 与“关闭导入结果”可关闭；错误提示含重试。
  逐项错误 code 按任务表格映射中文文案，未知 code 使用固定兜底；不渲染底层 message。
- **测试**：Rust 单元 1 个（覆盖 7 类路径场景）+ `m4_dialog` 3 个；前端
  `commands.test.tsx` 新增 selectImportFiles 2 个；`import-flow.test.tsx` 11 个；
  `query-client.test.tsx` 1 个；App 测试更新；`test-setup.ts` 增加 cleanup 与
  act 环境配置。总计 Vitest 60 tests。

**修改文件：**

- `src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`、`src-tauri/Cargo.toml`
- `Cargo.lock`、`package.json`、`package-lock.json`
- `src-tauri/tests/m2_commands.rs`、`src-tauri/tests/m3_protocol.rs`（依赖静态检查）
- `src-tauri/tests/m4_dialog.rs`（注册与 capability 验证）
- `src/bridge/ipc-schemas.ts`、`src/bridge/commands.ts`、`src/bridge/commands.test.tsx`
- `src/query/query-client.ts`、`src/query/query-client.test.tsx`
- `src/features/import/`（ImportFlowProvider、ImportButton、ImportStatus、
  ImportSummary、error-text、import-flow.test.tsx）
- `src/app/App.tsx`、`src/app/App.test.tsx`、`src/features/library/EmptyLibrary.tsx`
- `src/main.tsx`、`src/styles.css`、`src/test-setup.ts`
- `docs/tasks/TASK-M4B.md`（状态与本第 10 节）

未修改 `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、ADR、
`crates/`、数据库 schema、既有导入/查询语义、`src/reader-engine/`、M3 iframe 协议、
`capabilities/default.json`、CSP、网络/fs/shell 权限；未安装 `@tauri-apps/plugin-dialog`、
Zustand、Router；未创建 commit。

**关键设计选择：**

- dialog 使用 blocking API 但放入 `spawn_blocking`，避免阻塞 Tauri UI/event loop；
  纯函数分离选择结果校验，便于不启动真实对话框测试。
- capability 保持 `core:default`：dialog 仅经自定义 Rust 命令暴露，前端无法直接调用
  dialog 插件。
- 导入流程采用可注入 `commandClient`/`eventSubscriber`/`QueryClient`；事件先订阅后
  启动，返回 jobId 前暂存重放，保证快速后端事件不丢且其他 job 不污染汇总。
- 路径只保留在流程内存；DOM 仅显示 `sourceName`，不写日志、不进入 query cache。
- 逐项错误只用稳定 code 映射文案，未知 code 使用固定兜底，绝不渲染底层 message。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | `tsc -b`、Biome、Vitest 9 文件 60 tests 通过 |
| `npm run build` | 0 | 主应用 Vite 构建成功（JS 350.25 kB） |
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 252 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.33 MiB |

依赖解析：`tauri-plugin-dialog 2.8.4`（Cargo.lock）、`@tanstack/react-query 5.107.1`
（package-lock）。

**AC 验证：**

- FR-010–FR-014：系统多选 EPUB、只处理用户选择路径、不扫描目录、不复制原文件。
- FR-016–FR-019：汇总展示 imported/reactivated/duplicate/failed 与可理解失败原因。
- AC-010：可选 1 本/多本；取消返回 `[]` 且不调用 `startImport`（前端测试）。
- AC-011：结束时显示四项计数与逐项状态（import-flow 测试）。
- NFR-011：无网络请求；capability 仅 `core:default`；未安装 dialog 前端包。
- NFR-012：选择/启动/逐项失败均有稳定中文原因与重试入口。

**未验证项：**

- 真实 macOS 原生文件对话框的自动点击未验证（由 M4D 冒烟覆盖）；Rust 侧通过
  `cargo build`/`tauri build` 证明插件链接与命令注册。
- 未在真实 Tauri WKWebView 中验证 `selectImportFiles`、事件订阅与进度时序；Vitest
  使用注入的 fake command client/event subscriber。
- 既有 `tests/e2e/empty-library.spec.ts` 断言页面无按钮，与 M4B 新增“导入 EPUB”按钮
  冲突；`tests/e2e/` 不在本任务允许修改路径，未运行/更新 e2e（M4D 负责）。
- 逐项失败文案映射以 code 为准，未验证后端所有 code 的真实触发路径。

**风险与待决问题：**

- `blocking_pick_files` 在 `spawn_blocking` 中的主线程调度依赖插件实现；真实 macOS
  对话框行为需 M4D 冒烟确认。
- 主界面与空书库各有一个同名“导入 EPUB”按钮；测试使用 `getAllByRole`，后续 UI
  调整需保持 accessible name 一致。
- 导入事件在 `startImport` 返回前到达的窗口依赖暂存缓冲；极高并发或事件丢失时 UI
  可能停在“正在准备导入…”，后续可增加超时/恢复入口。
- 汇总 modal 打开时背景仍可聚焦；后续可加焦点陷阱（本任务未要求）。

**依赖变更：**

- 新增 `tauri-plugin-dialog = "2"`（Rust，解析 2.8.4）。
- 新增 `@tanstack/react-query@^5.107.1`（解析 5.107.1）。
- 未新增其他依赖。

## 11. Codex 审查

**结论：** 通过  
**审查日期：** 2026-11-14  

### 终轮复核

- act 问题已修复：`clickImport()` 使用 Testing Library 的 async `act` 包裹点击并等待异步
  状态更新；独立运行 `npm run check` 时 11 个 import-flow tests 无 act/Warning stderr，
  全项目 9 files、60 tests 通过。
- dialog 注册验证有效：`m4_dialog` 3 tests 覆盖插件、自定义命令、EPUB 过滤、blocking
  包装、capability、目录扫描和路径日志约束。
- 全部失败场景有效：覆盖失败计数 2、其他计数为 0、两种安全错误文案、隐藏底层 message、
  关闭后再次导入。
- 未发现 UI、Rust 语义、数据库、CSP/capability 或依赖越界修改。
- 既有 `tests/e2e/empty-library.spec.ts` 的按钮断言更新仍按计划由 M4D 负责，不阻塞
  M4B 当前 AC。

### 独立执行的检查

- `npm run check`：通过；9 files、60 tests，stderr 无 act 警告。
- `npm run build`：通过；主应用 production bundle 350.25 kB。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：252 passed、0 failed、1 ignored。
- `npm run tauri build`：通过；`Qurner.app` 7.33 MiB。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现 `BLOCKER` 或 `MAJOR` 问题。 | — |

**AC 结果：**

- FR-010–FR-014：通过；系统多选 EPUB、取消与路径上限契约成立；真实 dialog 冒烟留 M4D。
- FR-016–FR-019、AC-010、AC-011：通过；进度、四类汇总、逐项失败与安全错误文案成立。
- NFR-011：通过；capability 仅 `core:default`，无前端 dialog 包或网络请求。
- NFR-012：通过；选择、订阅、启动和逐项失败均有稳定中文反馈及恢复入口。

**返工或后续任务：**

- 无。
- 真实 Qurner.app dialog、事件时序和既有 e2e 更新由 M4D 集成验收覆盖。
