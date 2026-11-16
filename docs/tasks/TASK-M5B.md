# TASK-M5B：阅读器壳与 iframe 生命周期

**状态：** accepted
**里程碑：** M5
**负责人：** DeepSeek harness
**创建人：** Codex
**创建日期：** 2026-11-15
**依赖：** TASK-M5A accepted

## 1. 目标

用户从书库点击一本书后直接进入阅读界面：应用调用 `openBook`、加载安全 iframe、完成
阅读握手，并在加载失败、章节错误或返回书库时可靠关闭会话。M5B 不实现目录、模式控制、
设置、进度或书签功能。

## 2. 范围

### 必须完成

- 书库书籍增加可访问的打开操作；点击后直接进入阅读器，不经过详情页。
- App 只维护一个活动书籍 ID；进入阅读器后书库页卸载，返回时重新显示书库。
- ReaderShell 调用 `openBook`，严格解析 frame URL/token，加载 sandbox iframe。
- 使用现有 `ParentBridge` 完成 `reader:init` → `reader:ready`；透传初始模式和 Locator。
- 显示打开书籍、握手、iframe 错误、文件名失效、源变化和恢复操作。
- 返回书库、打开失败、组件卸载和页面退出时只关闭一次 M5A session，不删除个人数据。
- 保持 frame URL 与 token 不进入日志、DOM 文本、错误提示或 query cache。

### 不属于本任务

- 目录侧栏/抽屉、章节跳转或 TOC UI；由 M5C 实现。
- 翻页、滚动模式切换、方向键或完整 Esc 层级；由 M5C 实现。
- 字号、主题、设置继承或沉浸式；由 M5D 实现。
- 阅读进度保存/恢复、书签、文本复制；由 M5E 实现。
- M5E 或 M6 问题；由 M6 负责。
- Rust command、`epub://` 协议、数据库 schema、依赖或 capability 修改。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收 |
|---|---|---|
| FR | FR-210 | 点击书库书籍直接进入阅读界面 |
| FR | FR-059 | 打开失效/变化文件时显示原因并保留书库记录 |
| FR | FR-070, FR-071 | ReaderShell 使用 openBook 返回的模式和 Locator 启动 M3 iframe |
| FR | FR-120, FR-160 | 已返回 TOC 与 Locator 被安全传给阅读层；UI 不在本任务展示 |
| AC | AC-040, AC-070, AC-160, AC-210 | 打开入口、失效文件、初始模式/位置和返回书库可观察 |
| NFR | NFR-010 | 阅读器生命周期不修改原始文件或软移除个人数据 |
| NFR | NFR-011 | iframe/父页面不产生网络请求 |
| NFR | NFR-012 | 打开/握手/章节失败显示稳定原因和下一步 |

## 4. 组件与数据流

```text
LibraryPage(open button)
  -> App activeBookId
  -> ReaderShell(bookId)
  -> commands.openBook(bookId)
  -> iframe src=ReaderSession.frameUrl
  -> ParentBridge.init
  -> reader:ready
  -> ReaderShell ready

Back/unmount
  -> closeBook(sessionId) once
  -> App clears activeBookId
  -> LibraryPage
```

### 4.1 App 与书库

- `App` 使用本地 React state 管理 `activeBookId`，不新增 Router 或状态库。
- `LibraryPage` 增加 `onOpenBook(bookId)` 回调。
- 每本书提供 accessible name 为“打开《书名》”的操作；移除按钮保持独立，不得嵌套按钮。
- 打开按钮覆盖封面/书名区域；键盘 Enter/Space 可打开。
- 进入 ReaderShell 后不继续挂载书库 toolbar/virtualizer。
- 返回书库时保留当前 QueryClient；本轮不要求保留上一次搜索/排序/view 状态，但不得破坏
  Query cache 或重新导入。

### 4.2 ReaderShell

- ReaderShell 接收 `bookId`、command client、可选 ParentBridge factory 和 `onBack`。
- 当 bookId 变化时先关闭旧 session，再打开新 session；同一时刻最多一个活动 session。
- 打开成功前显示“正在打开书籍…”；错误时停止 loading。
- ReaderShell 顶层使用全宽阅读区域；本任务只提供最小标题/返回栏，不实现沉浸式。
- “返回书库”按钮始终可键盘访问；点击时先请求关闭 session，然后调用 `onBack`。
- `closeBook` 失败不得阻止返回书库；保留稳定错误状态，避免重复调用。

### 4.3 frame URL 与 token

- 只接受 M5A schema 已验证的 `epub://<token>/<resource>`。
- 从 frame URL 第一段提取 32–64 位小写十六进制 token；资源部分使用同一已验证 frame URL，
  不得重新拼接或 decode 后写回 iframe。
- token 和 frame URL 只保存在当前 ReaderShell 内存；禁止 console、错误对象、URL query、
  localStorage/sessionStorage、Query cache 或可访问文本。
- iframe：
  - `sandbox="allow-scripts"`，不得增加 `allow-same-origin`。
  - `referrerPolicy="no-referrer"`。
  - `title="《书名》阅读内容"`。
  - 不设置 allow 属性授予摄像头、麦克风等权限。

### 4.4 握手

- iframe `onLoad` 后创建 ParentBridge；也可以由注入 factory 在测试中提前控制。
- ParentBridge options：
  - `sessionId`
  - 从 frame URL 提取的 token
  - `initialMode = session.settings.mode`
  - `initialLocator = session.initialLocator`
- 调用 `start()` 发送 `reader:init`；只有 `reader:ready` 后显示阅读内容。
- 握手 timeout、非法 frame URL、ParentBridge 初始化失败、`reader:error` 都进入错误状态并
  停止 loading。
- 同一 session 的 ParentBridge 只创建一次；iframe 重新加载时不得复用旧 bridge。
- ReaderShell 卸载时调用 `bridge.close()`；不向已卸载 iframe 发送消息。

## 5. Session 生命周期

### 5.1 打开

- 每个 ReaderShell 实例使用单调 generation/cancelled 标记，防止 StrictMode 或快速切换时
  旧 openBook promise 覆盖新状态。
- openBook 成功但组件已卸载/切换时，立即调用 closeBook 关闭该 session，不设置 state。
- 记录当前 sessionId 和 close-once 状态；显式返回、错误返回和 unmount 共用同一个幂等关闭器。
- 打开失败时没有 sessionId，不得调用 closeBook。
- openBook 成功后若 iframe 失败，错误页的“返回书库”必须关闭 session。

### 5.2 重试

- 可恢复的 iframe/handshake 错误提供“重新加载阅读页”按钮。
- 重试前关闭旧 session/bridge，再对同一 bookId 调用新的 openBook；不得复用旧 token。
- `E_BOOK_UNAVAILABLE`、`E_SOURCE_CHANGED`、`E_BOOK_NOT_FOUND` 不提供无意义自动重试，
  只提供“返回书库”；可提示用户重新导入或检查文件。

### 5.3 返回

- 返回成功后 App 清空 activeBookId。
- `E_SESSION_INVALID` 视为 session 已关闭，不阻止返回。
- 其他 closeBook 错误不得阻止返回；不得展示 frame URL 或底层 message。
- 返回后书库 Query 可失效一次，以刷新 last opened 等未来状态。

## 6. 用户可见状态

| 状态 | 展示 |
|---|---|
| opening | “正在打开书籍…”，返回按钮可用 |
| connecting | “正在准备阅读内容…” |
| ready | iframe 阅读内容 |
| open-error | 固定中文原因、“返回书库”；文件错误不得自动重试 |
| frame/handshake error | 固定中文原因、“重新加载阅读页”“返回书库” |
| closing | 禁用重复返回，短暂显示“正在返回书库…” |

打开错误至少映射：

- `E_BOOK_NOT_FOUND`：该书已不在书库。
- `E_BOOK_UNAVAILABLE`：原始文件已被移动、删除或无法读取。
- `E_SOURCE_CHANGED`：文件内容与书库记录不一致，需要重新导入。
- `E_STORAGE`：本地阅读数据无法读取。
- 其他：无法打开该书，请重试。

所有错误不得渲染 backend message、stack 或文件路径。

## 7. 文件权限

### 允许修改

- `src/app/App.tsx`、`src/app/App.test.tsx`
- `src/features/library/LibraryPage.tsx`、`library-page.test.tsx`
- `src/features/reader/`（ReaderShell、状态、错误文案、测试）
- `src/styles.css`
- 任务专用测试支持文件
- 本任务状态和第 10 节开发报告

### 禁止修改

- `src-tauri/`、`crates/`、数据库、M5A command/DTO、`src/bridge/` 公开协议
- `src/reader-engine/`、iframe 消息类型或语义
- `package.json`、锁文件、依赖
- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、ADR 和其他任务
- CSP、capability、文件/网络权限

如需要修改上述禁止路径，标记 `blocked` 并报告。

## 8. 测试要求

### 书库与 App

- 点击书名/封面打开按钮调用 `onOpenBook(bookId)`，不调用移除命令。
- 移除按钮仍独立且不会触发行打开。
- App 从书库进入 ReaderShell；返回后显示书库并恢复可操作状态。
- 快速切换/卸载不会让旧 openBook promise 覆盖新页面。
- 不支持键盘打开时测试应失败。

### ReaderShell 生命周期

- openBook 成功：iframe src、sandbox、title、会话 ID 和初始 mode/Locator 正确。
- iframe load 后仅创建一次 ParentBridge 并 start；ready 后 loading 消失。
- bridge 初始化、握手 timeout、reader:error、非法 frame URL 进入稳定错误状态。
- openBook `E_BOOK_NOT_FOUND`/`E_BOOK_UNAVAILABLE`/`E_SOURCE_CHANGED`/`E_STORAGE`
  显示固定中文说明，不暴露 backend message/path。
- 返回书库只 closeBook 一次；closeBook 失败仍返回。
- 打开成功但组件卸载时自动 close；打开失败时不 close。
- 重试关闭旧 session、创建新 session 和新 bridge，不复用旧 token。
- ParentBridge close 与 session close 在 unmount 时各执行一次。
- DOM、console、storage、query cache 不含 frame URL/token。

测试使用 fake command client 和可注入 ParentBridge factory，不加载真实网络/自定义协议。

### 必跑命令

```bash
npm run check
npm run test:e2e
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri build
```

## 9. 禁止事项

- 不实现 M5C–M5E 功能。
- 不新增依赖、Router、Zustand 或其他状态库。
- 不修改 M3 消息协议或 Rust session 行为。
- 不向 iframe sandbox 添加 `allow-same-origin`。
- 不把 token/frame URL 写入日志、DOM 文本或持久化。
- 不在错误页展示 backend message。
- 不提交、推送或发布。

## 10. DeepSeek 开发报告

> 仅在开始实现后填写。开始实现时将状态改为 `in_progress`。

**完成摘要：**

实现书库打开入口与 ReaderShell iframe 生命周期：openBook → sandbox iframe →
ParentBridge 握手 → ready；打开/握手/章节错误、重试与返回书库；session 只关闭一次。

第 11 节复验返工修复：

1. **完整校验 openBook 返回值**：`ReaderShell` 对返回值执行
   `readerSessionSchema.parse`（frameUrl 整体解码 + token/路径安全检查），只有通过
   后才设置 session/iframe；编码穿越（`%2e%2e%2f`）、query、fragment、反斜杠、NUL、
   非法 token 均不会进入 iframe，并会关闭刚创建的 session 后显示
   `E_INVALID_FRAME_URL` 错误。新增参数化测试覆盖 6 类非法 frameUrl。
2. **iframe error 与关闭后迟到 load**：
   - iframe 错误改为原生 `addEventListener("error")`（jsdom 可触发），`onError` 分支
     新增单测；
   - `handleIframeLoad` 增加 `closedRef.current` 检查：返回/关闭后迟到的 load 不再
     创建 bridge 或发送 init；新增对应测试；
   - close-once 生命周期测试保留（返回/卸载/迟到 load）。
3. **token 泄露测试真正执行**：使用 fake `localStorage`/`sessionStorage` 与
   `console.log/error/warn` spy，断言存储为空、日志不含 token/frameUrl；DOM 文本
   断言保留。

- **书库/App**：每本书新增“打开《书名》”按钮（原生 `<button>`，覆盖封面与书名区域；
  键盘 Enter/Space 可用；移除按钮保持独立、不嵌套）。`App` 用本地 `activeBookId`
  切换书库与 ReaderShell；进入阅读器后书库 toolbar/virtualizer 不再挂载；返回后恢复
  书库并失效 `queryKeys.books` 一次。打开按钮保留 accessible name 供 E2E/键盘使用。
- **ReaderShell**：
  - 调用 `commandClient.openBook(bookId)`；opening 显示“正在打开书籍…”，成功后
    渲染 `sandbox="allow-scripts"`、`referrerPolicy="no-referrer"`、
    `title="《书名》阅读内容"` 的 iframe（src 为已验证的 frameUrl）；
  - iframe `onLoad` 后仅创建一次 ParentBridge（每 session 一次），options 传
    `sessionId`、从 frameUrl 第一段提取的 token、`initialMode`/`initialLocator`，
    调用 `start()`；`onReady` 后 iframe 才可见并隐藏 loading；
  - iframe 错误/握手超时/`reader:error`/非法 frame URL/bridge 创建失败进入稳定
    错误状态（固定中文文案，不渲染 backend message）；
  - 可恢复帧/握手错误提供“重新加载阅读页”：先关闭旧 session/bridge，再对同一
    bookId 重新 openBook（新 token、新 iframe、新 bridge）；
  - 打开失败按 code 映射固定文案；`E_BOOK_NOT_FOUND`/`E_BOOK_UNAVAILABLE`/
    `E_SOURCE_CHANGED` 不提供重试，`E_STORAGE` 可重试；
  - 显式返回、错误页返回、卸载共用 `closeSessionOnce`，保证每个 session 只
    closeBook 一次；close 失败不阻止返回；打开失败（无 session）不调用 closeBook；
  - generation/cancel 标记防止 StrictMode 或快速切换时旧 openBook 覆盖新状态；
    卸载后 resolve 的 session 立即关闭且不 setState；
  - unmount 时关闭 bridge 与 session 各一次。
- **Token 保护**：token 只存在于 ReaderShell 内存与 iframe src 属性；不写 console、
  storage、错误文案或 query cache；DOM 文本不含 token/frameUrl。测试通过 console
  spy、localStorage、textContent 断言。
- **测试**：ReaderShell 11 个；LibraryPage 新增打开按钮/独立移除测试；App 新增
  进入阅读器并返回书库测试（含虚拟化尺寸 mock）；E2E 新增“打开书籍进入阅读器并
  返回书库”和“非法 frame URL 显示错误并返回书库”。全量 Vitest 100 tests。

**修改文件：**

- `src/app/App.tsx`、`src/app/App.test.tsx`
- `src/features/library/LibraryPage.tsx`、`library-page.test.tsx`
- `src/features/reader/ReaderShell.tsx`（新增）、`reader-errors.ts`（新增）、
  `reader-shell.test.tsx`（新增）
- `src/styles.css`
- `tests/e2e/library.spec.ts`
- `docs/tasks/TASK-M5B.md`（状态与本第 10 节）

未修改 `src-tauri/`、`crates/`、数据库、M5A command/DTO、`src/bridge/` 公开协议、
`src/reader-engine/`、iframe 消息语义、`package.json`/锁文件、CSP/capability；
未新增依赖；未提交/推送。

**关键设计选择：**

- `App` 只用本地 `activeBookId` 切换；不引入 Router/状态库。
- `ReaderShell` 用 generation + cancelled + `closedRef` 组合：打开/切换/卸载时旧
  promise 不能覆盖新状态；每个 session 的关闭是幂等 close-once。
- ParentBridge 由 iframe `onLoad` 创建，每个 session 只创建一次；重试关闭旧 bridge
  后创建新 session/iframe/bridge，避免复用旧 token。
- iframe 在 ready 前 `visibility: hidden`，loading 文案独立显示，满足“ready 后
  才显示阅读内容”。
- 错误文案按稳定 code 映射；文件/书库错误无重试，帧/握手错误可重试。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | `tsc -b`、Biome、Vitest 11 文件 102 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 31 passed（首次并行运行出现 1 个 flake，复跑通过） |
| `npm run build` | 0 | 主应用 Vite 构建成功（JS 416.13 kB） |
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 283 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.79 MiB |

**AC 验证：**

- FR-210：书库打开按钮直接进入 ReaderShell，无详情页。
- FR-059：打开失败显示稳定原因并保留书库记录（不调用 closeBook 无 session）。
- FR-070/FR-071：ReaderShell 使用 openBook 返回的 mode 与 initialLocator 初始化
  ParentBridge。
- FR-120/FR-160：TOC/Locator 从会话传递（TOC UI 未实现，属 M5C）。
- AC-040/AC-070/AC-160/AC-210：打开入口、失效文件错误、初始模式/位置与返回书库。
- NFR-010：阅读器生命周期只关闭会话，不修改原文件或个人数据。
- NFR-011：iframe/父页面未新增网络请求；测试 mock IPC。
- NFR-012：打开/握手/非法 URL 失败有固定中文原因与返回/重试入口。

**未验证项：**

- 真实 Tauri WKWebView 中 `epub://` iframe 加载与 ParentBridge 握手（Vitest 使用
  fake bridge；E2E 打开测试使用 mock IPC，iframe 指向 `epub://` 但未断言真实握手）。
- iframe 原生 `error` 已通过原生监听在 jsdom 单测覆盖；真实 WebKit 中
  `epub://` 未知 scheme 的 iframe 错误行为仍未端到端断言。
- 真实 `closeBook` IPC 失败路径（E2E mock 恒成功）。
- 目录/翻页/模式/设置/进度 UI 按任务范围未实现（M5C–M5E）。
- 快速打开-返回-再打开的连续压力未做真实浏览器测量。

**风险与待决问题：**

- iframe src 属性必然包含 frameUrl/token（任务允许内存与 src），DOM 文本/日志无
  泄漏；后续 UI 仍需避免把 src 写入文本。
- 前端 `commands.openBook` 与 ReaderShell 都会校验 frameUrl；ReaderShell 的
  `E_INVALID_FRAME_URL` 分支在直接注入非法 session 的测试中覆盖，生产路径通常由
  `commands.openBook` 先行拒绝并显示打开错误。
- E2E 套件存在一次并行 flake（首次 30 passed/1 failed，复跑 31 passed）；需关注
  webServer/浏览器资源竞争，但最终门禁通过。
- iframe 在 ready 前隐藏，若握手失败会显示错误页；真实 WKWebView 中 epub:// 协议
  可用性由 M5D/M6 真实应用冒烟验证。
- closeBook 失败被忽略，不展示错误；返回优先保证用户体验。

**依赖变更：**

- 无新增依赖；未修改 `package.json`/锁文件。

## 11. Codex 审查

**结论：** 通过
**审查日期：** 2026-11-15

### 终轮复核

- ReaderShell 对 openBook 返回值重新执行完整 `readerSessionSchema`，编码穿越、query、
  fragment、反斜杠和 NUL frame URL 均被拒绝并关闭 session。
- iframe `error` 有独立测试；关闭后迟到的 load 由 `closedRef` 拦截，不创建 bridge。
- Token 泄露测试使用可用的 localStorage/sessionStorage stub，并检查 DOM、console.log、
  console.error、console.warn；未出现存储或日志写入。
- 书库打开/移除分离、App 返回、bridge 单次创建、重试换 token、close-once、卸载和
  StrictMode 迟到 openBook promise 均通过。
- 未发现依赖、Rust 协议、CSP/capability 或范围越界修改。

### 独立执行的检查

- `npm run check`：通过；11 files、102 tests，stderr 干净。
- `npm run test:e2e`：WebKit 31 passed。
- `npm run build`：通过。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：283 passed、0 failed、1 ignored。
- `npm run tauri build`：通过；`Qurner.app` 7.79 MiB。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现 `BLOCKER` 或 `MAJOR` 问题。 | — |

**AC 结果：**

- FR-210：通过；书库点击直接进入 ReaderShell。
- FR-059：通过；文件失效/变化错误有稳定说明并保留记录。
- FR-070/FR-071：通过；当前会话的模式和 Locator 传入阅读 iframe。
- FR-120/FR-160：通过；TOC/Locator 完成安全传递，TOC UI 留 M5C。
- AC-040、AC-070、AC-160、AC-210：本任务范围通过。
- NFR-010–NFR-012：通过；不修改原文件、不联网、错误稳定不泄漏。

**返工或后续任务：**

- 无。
- 真实 WKWebView `epub://` 握手由 M5D/M6 真实应用集成继续覆盖。
