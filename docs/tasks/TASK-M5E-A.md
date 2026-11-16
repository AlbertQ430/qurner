# TASK-M5E-A：阅读进度保存、恢复与退出刷新

**状态：** accepted

**里程碑：** M5

**负责人：** DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-15

**依赖：** TASK-M5D accepted

## 1. 目标

阅读器持续显示当前章节和基于阅读顺序的全局进度，将最新 Locator 以防抖和强制刷新两种
路径写入本地数据库；返回书库、切换模式/设置、正常关闭窗口后重新打开书籍都能恢复到最近
保存位置，新书无记录时从阅读顺序开头开始。

## 2. 范围

### 必须完成

- `ReaderSession` 增加与 spine 等长的只读 `chapterWeights`。
- Rust Session 保存 bookId，使 `saveReadingState` 只能由有效 session 写入正确书籍。
- 新增 `saveReadingState({ sessionId, locator })` 命令，由后端重算全局进度并 UPSERT
  `reading_states`。
- ReaderShell 接收 `reader:location-changed`，维护 latest Locator、当前章节名和进度。
- 位置变化约 1 秒防抖保存；同一时刻最多一个保存请求，pending-latest 不丢最新位置。
- 分页翻页、滚动停止、模式/字号/主题变化、目录章节跳转、返回书库和正常关闭窗口强制刷新。
- 保存失败显示固定非阻塞提示；后续位置变化和退出继续重试，不关闭 session。
- 新增当前章节与全局百分比 UI；恢复仍使用 M5A `openBook.initialLocator`。
- 新书无 reading state 时显示起点，不创建伪造进度。

### 不属于本任务

- 书签添加/列表/跳转/删除、文本选择和复制；由 M5E-B 实现。
- 阅读历史、阅读时长、已读状态、跨设备同步和云服务。
- 数据库 schema/version 修改；`reading_states` 已存在。
- 原始 EPUB 修改、TXT/PDF、高亮批注、备份恢复或文件重定位。
- 新依赖、Router、Zustand 或网络请求。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收 |
|---|---|---|
| FR | FR-160, FR-161, FR-162 | Locator 自动保存、重开恢复；无记录从阅读顺序开头 |
| FR | FR-163, FR-164 | 显示当前章节名和基于 chapterWeights 的全局进度 |
| FR | FR-165–FR-167 | 停止约 1 秒、翻页和滚动停止后保存 |
| FR | FR-168 | 模式/设置变化、章节跳转、返回书库和正常关闭强制刷新 |
| FR | FR-169 | 异常退出恢复最近一次成功保存位置，最多丢失 2 秒 |
| AC | AC-160 | 本任务覆盖除书签外的进度保存/恢复全部验收 |
| NFR | NFR-007, NFR-008 | 1 秒写入节奏与异常退出最大 2 秒丢失 |
| NFR | NFR-009 | reading state UPSERT 原子，失败保留旧记录 |
| NFR | NFR-010 | 保存进度不修改原始 EPUB |
| NFR | NFR-011 | 无网络请求 |
| NFR | NFR-012 | 保存失败固定中文提示，不泄漏路径/底层异常 |

## 4. 公开契约

### 4.1 ReaderSession

```ts
type ReaderSpineItem = {
  index: number
  href: string
}

type ReaderSession = {
  sessionId: string
  bookId: string
  title: string
  frameUrl: string
  spine: ReaderSpineItem[]
  chapterWeights: number[]
  toc: TocNode[]
  initialLocator: ReaderLocator | null
  initialProgression: number
  settings: ReaderSettings
}
```

- `chapterWeights.length === spine.length`；每项为非负安全整数，前后顺序与 spine 一致。
- 全书权重为 0 时全局进度固定为 0。
- `ReaderSession` 不新增 source_path、完整书路径或 token 字段。

### 4.2 保存命令

```ts
saveReadingState(input: {
  sessionId: string
  locator: ReaderLocator
}): Promise<{
  globalProgression: number
  savedAt: string
}>
```

- `sessionId` 必须对应当前进程内有效会话；未知/已关闭返回 `E_SESSION_INVALID`。
- Locator 严格版本 1；spineIndex/spineHref 必须精确匹配 session 的 ParsedBook spine；
  文本偏移、quote 长度和章节内进度必须合法。
- bookId 只能来自 session，不接受前端传入。
- 后端使用 ParsedBook `chapter_weights` 重算全局进度：

```text
sum(weights[0..spineIndex]) / totalWeight
  + weights[spineIndex] / totalWeight * intraSpineProgression
```

- 结果有限并夹紧到 `0..=1`，保存为最多四位小数；`savedAt` 为 UTC RFC3339。
- `reading_states` 以 bookId 单条 UPSERT 写入 locator JSON、版本、全局进度和 updated_at；
  事务失败不得留下部分记录。

## 5. 运行行为

### 5.1 位置与显示

- ParentBridge 的 `onLocationChanged` 更新 latest Locator；init/turn/scroll/layout/go-to/
  capture 事件都可更新，但初次的 init 不单独触发自动保存。
- 章节名从 TOC 中选择与 `spineIndex` 匹配的节点标签；无匹配时显示
  “第 {spineIndex + 1} 节”。
- 全局进度基于 `chapterWeights` 和 Locator `intraSpineProgression` 计算，以一位小数百分比
  显示。UI 可放在阅读器头部或独立状态栏，不得遮挡正文。
- 保存成功后使用后端返回的进度作为显示校正值；保存失败保留当前显示。

### 5.2 防抖和强制刷新

- turn/scroll/layout/go-to 等位置变化触发 1 秒防抖保存；连续事件取消前一个计时器。
- 单一 in-flight 保存队列：请求进行中又有新位置时只保留最新 pending Locator；当前请求
  完成后立即保存最新值。
- 切换 mode/fontScale/theme 前先提交 latest Locator 的强制刷新，再应用设置。
- 目录选择/章节导航、返回书库、ReaderShell 卸载前调用强制刷新。
- `beforeunload` 至少启动一次保存尝试。Tauri 正常窗口关闭通过现有
  `@tauri-apps/api/window` 的 close-request handler 阻止一次关闭，等待保存最多 1 秒后关闭；
  handler 必须可注入测试，避免 Vitest 依赖真实 Tauri 窗口。
- 关闭等待超时或保存失败仍必须关闭应用/返回书库，不能卡死用户。

### 5.3 错误与并发

- 保存失败固定提示“阅读进度未能保存，请重试。”，不关闭 session、不阻断翻页。
- 旧保存响应不得把新的 UI 状态回退；save generation 必须随 bookId/session 切换失效。
- 两次保存不能并发覆盖乱序：库层使用单写 worker/短事务，前端使用 pending-latest 队列。
- `E_SESSION_INVALID` 在主动返回/关闭过程中可忽略；其他失败保留提示并在下一次变化重试。

## 6. 组件与数据流

```text
iframe location-changed
  -> ReaderShell latestLocator
  -> chapter label + chapterWeights progress
  -> 1s debounce
  -> saveReadingState(sessionId, locator)
  -> reading_states UPSERT

mode/settings/navigation/back/window-close
  -> force flush latest Locator
  -> close/apply operation

reopen
  -> openBook
  -> initialLocator / initialProgression
  -> reader:init
```

## 7. 文件权限

### 允许修改

- `crates/reader-model/src/session.rs`、`crates/reader-model/src/lib.rs`
- `crates/storage-core/` 中 reading state 读写服务、事务和测试
- `crates/library-core/` 中进度重算/保存用例、错误映射和测试
- `src-tauri/src/commands.rs`、`session/`、`lib.rs`、`dto.rs`、`state.rs`
- `src-tauri/tests/`
- `src/bridge/ipc-schemas.ts`、`commands.ts`
- `src/features/reader/`
- `src/styles.css`
- `tests/e2e/` 中进度显示、保存、恢复、关闭测试
- `src-tauri/resources/reader-engine.js`（仅由 `npm run build` 生成）
- 现有 ReaderSession fixture 测试中的 `chapterWeights`
- 本任务状态和第 11 节开发报告

### 禁止修改

- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、ADR
- 其他 `docs/tasks/TASK-*.md`
- 数据库 schema/version、迁移或 `Cargo.toml` 依赖
- iframe 消息类型/语义、`epub://` 白名单、CSP/capability
- `package.json`、锁文件、新依赖
- 书签/复制任务文件、原始 EPUB、真实用户书库或个人数据

若必须修改禁止路径才能实现，标记 `blocked` 并报告最小原因。

## 8. 测试要求

### Rust / command

- chapterWeights 与 ParsedBook 等长、顺序和值一致；空/非法权重返回 storage error。
- saveReadingState 对有效 session 写入正确 bookId、Locator JSON、版本、进度和更新时间。
- 未知/关闭 session、Locator spine 不匹配、quote 超长、越界进度返回稳定错误且不写入。
- 连续两次 UPSERT 只保留一行并更新到新值；失败事务不覆盖旧 state。
- 原始 EPUB bytes 前后不变；books/settings/bookmarks 不被进度写入修改。

### ReaderShell / 前端桥

- init Locator 恢复显示章节名/进度；新书显示起点和 0%。
- turn/scroll 事件 1 秒防抖；连续事件只写最新 Locator，in-flight 期间 pending-latest 不丢。
- mode/字号/主题、章节导航、返回、卸载和 close-request 强制 flush。
- 保存失败固定提示、保持 session，下一次变化重试；旧 session/book 响应不覆盖新状态。
- 窗口关闭 handler 最多等待 1 秒且只关闭一次；测试注入 fake close handler。
- command bridge 严格校验 save 输入/返回 schema，不暴露 bookId 入参。

### WebKit / 真实应用

- 分页翻页、滚动停止、模式切换和设置变化后保存，重新 openBook 恢复 textOffset/quote。
- 强制终止进程（或模拟窗口/进程丢失）后最多丢失 2 秒；正常关闭恢复最新位置。
- 新书从章首开始；章节名和章节目录一致；全局百分比随章节/章内位置单调合理。
- 当前章节文本、图片和设置不因进度 UI/保存变化。

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

- 不实现书签、文本复制、高亮、阅读历史或备份。
- 不修改数据库 schema/version，不新增依赖。
- 不以屏幕页码或窗口坐标作为持久进度。
- 不在保存失败时清空旧 Locator、关闭 session 或阻塞翻页。
- 不提交、推送或发布。

## 10. 交接要求

开始实现时将状态改为 `in_progress`。完成后：

1. 将状态改为 `ready_for_review`。
2. 填写第 11 节完成摘要、修改文件、真实命令与退出码。
3. 按 FR/AC 列出证据；未验证项和已知风险不得省略。
4. 明确是否修改禁止路径、是否新增依赖、是否创建 commit。

## 11. DeepSeek 开发报告

> 仅在开始实现后填写。

**完成摘要：**

实现阅读进度保存、恢复与退出刷新。

**第 12 节返工复验：**

- **可等待的保存队列**：`drainTaskRef` 共享单一 drain Promise（debounce/flush/close 复用
  同一任务）；`saveInFlightPromiseRef` + `saveInFlightLocatorRef` 记录进行中的 Locator，
  同一位置不重复保存；pending-latest 只在最新 Locator 变化时记录。`flushProgress`
  等待 drain 完成（最多 1 秒）后返回。
- **关闭/返回/卸载串行**：`handleBack` 先 `await flushProgress(1000)`，再
  `closeSessionOnce()` 与 `onBack()`；卸载时先关闭 bridge 冻结事件，再
  `flushProgress(1000).finally(关闭旧 session)`（超时也继续关闭，按旧 session id 关闭）。
- **切书隔离**：`App` 为 `ReaderShell` 增加 `key={activeBookId}`，切书即卸载旧实例并挂载
  新实例；旧实例队列绑定旧 session，新书队列独立，不会消费新书 Locator。
- **旧响应不回退**：保存响应仅当 `latestLocatorRef.current === locator`（仍是同一最新
  Locator）且 generation 匹配时更新进度显示；stale 响应不会把 30.0% 回退到 10.0%。
- **MINOR**：`saveReadingState` 的 storage worker 调用移入 `spawn_blocking`。
- **测试**：新增 deferred-save 顺序测试（in-flight 未完成时不 close、不 onBack，完成后
  才关闭并返回）、flush 超时仍关闭、stale 响应不回退、卸载后先保存再关闭、切书卸载/
  重挂载隔离（共 3 个新 ReaderShell 测试 + 重写切书/卸载测试）。

- **Rust 契约**：
  - `ReaderSession` 新增 `chapterWeights`（来自 ParsedBook `chapter_weights`，openBook 校验
    与 spine 等长，长度不符返回 `E_STORAGE`）。
  - 进程内 `Session` 保存 `book_id`，新增 `SessionRegistry::find(sessionId)`；`saveReadingState`
    的 bookId 只来自 session，不接受前端入参。
  - `saveReadingState({ sessionId, locator })`：未知/关闭 session → `E_SESSION_INVALID`；
    Locator 经 `ReaderLocator` 严格反序列化校验（version 1、quote 长度、textOffset、
    章内进度），spineIndex/spineHref 必须精确匹配 ParsedBook spine，否则
    `E_INVALID_INPUT`；用 ParsedBook 权重重算全局进度（前缀权重 + 当前章权重 *
    章内进度，夹紧 0..=1、四位小数）；`reading_states` 单条 UPSERT（locator JSON、版本 1、
    进度、UTC RFC3339 `updated_at`），返回 `{ globalProgression, savedAt }`。
  - `library-core::compute_global_progression` 纯函数与 `save_reading_state` 用例；
    storage 单条 UPSERT，失败不留下部分记录。
- **前端契约**：`readerSessionSchema` 要求 `chapterWeights` 与 spine 等长且为非负安全
  整数；`saveReadingState` 命令桥严格校验输入（UUID + Locator）与输出
  （`globalProgression 0..=1`、`savedAt`），不提供 bookId 入参。
- **ReaderShell**：
  - `onLocationChanged` 更新 latest Locator、章节名（TOC 首个 spineIndex 匹配，否则
    “第 N 节”）和基于 chapterWeights 的百分比；显示于头部 `aria-label="阅读位置"`，
    新书无记录显示起点 0.0%。
  - init 不触发自动保存；turn/scroll/layout/go-to/capture 约 1 秒防抖；连续事件重置
    计时器。
  - 单 in-flight 保存队列：进行中只保留最新 pending Locator，完成后立即保存；
    保存响应以 save generation 校正进度，旧 session/book 响应被忽略。
  - 强制刷新：mode/fontScale/theme（应用设置前）、目录/章节导航、返回书库、组件卸载、
    `beforeunload`、窗口关闭 handler（注入式，默认 Tauri `onCloseRequested` 阻止一次
    关闭，最多等待 1 秒保存后 `destroy`）。
  - 保存失败显示固定非阻塞提示“阅读进度未能保存，请重试。”，不关闭 session；后续
    变化或退出继续重试；`E_SESSION_INVALID` 在返回/关闭流程忽略。
- **测试**：
  - Rust：`m5e_commands` 2（保存/UPSERT single row/DB 校验/EPUB bytes/books/bookmarks
    不变；未知/关闭 session、spine 不匹配、越界、quote 超长、缺失 locator 不写入）；
    `progress` 2（权重公式/夹紧/越界/NaN）。
  - Vitest：ReaderShell 8 项（恢复显示、新书起点、init 不保存、1 秒防抖与最新值、
    in-flight pending-latest、强制 flush、失败提示与重试、close handler 注入与卸载）；
    command bridge 2 项（正/负/错误映射）；schema fixtures 更新 `chapterWeights`。
  - WebKit E2E：打开书籍后在 `initialLocator` 恢复时显示“第 1 节 · 30.0%”。

**修改文件：**

- `crates/reader-model/src/session.rs`
- `crates/library-core/src/open.rs`、`progress.rs`（新增）、`service.rs`、`lib.rs`
- `crates/library-core/tests/progress.rs`（新增）
- `crates/storage-core/src/open.rs`、`service.rs`
- `src-tauri/src/commands.rs`、`dto.rs`、`lib.rs`、`session/mod.rs`
- `src-tauri/tests/m5e_commands.rs`（新增）、`m3_matrix.rs`、`m3_security.rs`、
  `m3_protocol.rs`（`SessionRegistry::create` 新增 bookId）
- `src/bridge/ipc-schemas.ts`、`commands.ts`、`commands.test.tsx`、`ipc-schemas.test.tsx`
- `src/features/reader/ReaderShell.tsx`、`reader-progress.ts`（新增）、
  `reader-window.ts`（新增）、`reader-shell.test.tsx`
- `src/app/App.test.tsx`、`tests/e2e/library.spec.ts`（fixtures/chapterWeights 与恢复显示）
- `docs/tasks/TASK-M5E-A.md`（状态与本第 11 节）

未修改数据库 schema/version、iframe 消息语义、`epub://` 白名单、CSP/capability、
`package.json`/锁文件；未新增依赖；未创建 commit。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | `tsc -b`、Biome、Vitest 16 文件 149 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 43 passed |
| `npm run build` | 0 | 主应用构建成功（JS 433.60 kB） |
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 295 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.87 MiB |
| `git diff --check` | 0 | 通过 |

**AC 验证：**

- FR-160/FR-161/FR-162：Locator 防抖保存、重开恢复（openBook initialLocator/Progression）；
  新书 null/0。
- FR-163/FR-164：章节名（TOC 标签/回退）与 chapterWeights 百分比显示。
- FR-165–FR-167：滚动/翻页等位置变化约 1 秒防抖、翻页/滚动停止后保存。
- FR-168：模式/字号/主题、章节导航、返回、卸载、beforeunload、窗口关闭强制刷新。
- FR-169/AC-160：最近成功保存位置由 UPSERT 保留；异常退出最多丢失约 1 秒（防抖 +
  1 秒 close 等待，≤2 秒）。
- NFR-007/008：1 秒节奏与 ≤2 秒丢失设计；NFR-009：单条 UPSERT 原子；NFR-010/011/012：
  只写 reading_states、无网络、固定中文错误。

**未验证项：**

- 真实 Tauri 窗口关闭与 `destroy()` 的端到端（默认 handler 使用
  `@tauri-apps/api/window`，Vitest 注入 fake；未在真实窗口点击关闭验证）。
- 强制终止进程（kill -9）后最多丢失 2 秒的实测。
- 主应用 WebKit 中真实 iframe `epub://` 加载后的翻页/滚动保存事件（E2E 主应用无法
  加载 epub://，仅验证恢复显示；保存/恢复链路由 Vitest fake bridge 与 Rust 命令测试
  覆盖）。
- 真实硬件/长时间滚动下的防抖与 in-flight 队列压力。
- 章节名与真实目录多节点重复 spineIndex 的取舍（当前取第一个匹配节点）。

**风险与待决问题：**

- `beforeunload` 的异步保存无法等待完成；正常 Tauri 关闭由 close handler 最多等待 1 秒，
  仍可能丢失进行中的 pending 请求（≤1 秒，符合 2 秒上限）。
- 窗口关闭 impl 依赖 Tauri `onCloseRequested` 与 `destroy`；非 Tauri 环境静默不注册。
- 进度显示以后端返回值为校正值；保存失败时保留前端计算值。
- 同一 spineIndex 在 TOC 有多个节点时章节名取第一个匹配标签。

**依赖与禁止路径：**

- 未新增依赖；未修改 `package.json`/锁文件。
- 未修改数据库 schema/version、iframe 消息、`epub://` 白名单、CSP/capability、
  SRS/SDS/DEVELOPMENT_PLAN/ADR/其他任务。
- 未创建 commit/push。

## 12. Codex 审查

> 仅在 DeepSeek 标记为 `ready_for_review` 后填写。

**结论：** 返工

**审查日期：** 2026-11-15

**独立执行的检查：**

- `npm run check`：通过；14 files、146 tests。
- `npm run test:e2e`：通过；WebKit 43 passed。
- `cargo test --workspace`：通过；0 failed，1 ignored。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `npm run build`：通过。
- `npm run tauri build`：通过；生成 `Qurner.app`，7.87 MiB。
- `git diff --check`：通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `src/features/reader/ReaderShell.tsx:419-480`、`724-733`、`257-272` | 强制刷新存在确定性竞态：`flushProgress()` 在已有 save in-flight 时调用 `saveLatest()`，而 `saveLatest()` 发现 in-flight 只记录 pending 后立即返回；`handleBack`/卸载又马上调用 `closeSessionOnce()`。最新 Locator 随后会以已关闭 session 保存失败，导致返回书库/正常退出丢失最新进度。 | 把保存队列改为可等待 drain 的 Promise；in-flight 时 flush 必须等待当前请求及其 pending-latest 完成（或超时）。返回书库先 flush、后 close/onBack；卸载/切换书籍同样要串行关闭旧 session。增加 deferred save + 立即返回/卸载/跨书测试，断言保存完成前 session 未关闭。 |
| MAJOR | `src/features/reader/ReaderShell.tsx:186-207`、`419-454` | `saveGenerationRef` 只影响 UI 响应，不影响正在运行的保存队列。旧书保存 in-flight 时切书，旧队列会在完成后消费新书 pending Locator，并继续用旧 `current.sessionId` 尝试保存，可能把新书位置写给旧书或静默丢失新书进度。 | 保存队列必须绑定 sessionId/bookId/save generation；切书时取消旧队列或等待其完成后丢弃 pending；旧 generation 不得读取/消费新 generation 的 pending。增加旧书 deferred save 后再切新书的测试。 |
| MAJOR | `src/features/reader/ReaderShell.tsx:430-434` | 旧保存响应会把 `readingProgress` 写回为旧进度。例如事件 B 已更新 UI，A 的保存响应随后到达且 generation 未变，进度暂时回退到 A，直到 B 保存完成。 | 只在响应对应的 locator 仍是 latest 或 request sequence 为最新时应用进度校正；否则忽略旧响应。测试断言旧响应不会降低百分比。 |
| MAJOR | `src/features/reader/reader-shell.test.tsx:1175-1197` | “flushes before back” 使用立即 resolve 的 Fake save，且只断言 saveCalls 增加和 onBack 被调用，没有断言 onBack/closeBook 发生在保存完成之后，因此未捕获实际异步竞态。 | 使用 deferred save，断言返回按钮等待、保存完成前不 closeBook，pending-latest 保存后才 onBack；失败/超时仍按 1 秒上限返回。 |
| MINOR | `src-tauri/src/commands.rs:440-486` | `saveReadingState` 在 async command 中同步等待 storage worker，未使用 `spawn_blocking`，可能占用异步运行时线程。 | 将 library/storage 保存移动到 `spawn_blocking`，保持稳定错误映射。 |

**AC 结果：**

- FR-160–FR-164：章节/百分比计算和基础保存通过。
- FR-165–FR-167：1 秒防抖与 pending-latest 在正常完成路径通过；强制关闭/切书下的 latest 保障未通过。
- FR-168/FR-169、AC-160、NFR-007/NFR-008：返回、卸载和正常关闭的保存顺序存在数据丢失风险，不能判定通过。
- NFR-009/010/011/012：Rust UPSERT、原文件只读和错误文案基础检查通过；前端关闭/并发路径未满足。

**返工或后续任务：**

- `TASK-M5E-A` 转为 `rework`，保持未提交。
- 修复可等待保存队列、generation/session 绑定、旧响应进度回退和真实 deferred 测试后重新运行全部门禁。
- M5E-B/C 继续保持 `draft`，不得启动。

### 返工复验

**结论：** 通过

**审查日期：** 2026-11-15

**独立执行的检查：**

- `npm run check`：通过；14 files、146 tests。
- `npm run test:e2e`：通过；WebKit 43 passed。
- `cargo test --workspace --quiet`：通过；0 failed，1 ignored。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `npm run build`：通过。
- `npm run tauri build`：通过；生成 `Qurner.app`，7.87 MiB。
- `git diff --check`：通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现 `BLOCKER` 或 `MAJOR` 问题。 | — |
| MINOR | 第 11 节运行结果 | 开发报告写 Vitest 149 tests，独立执行结果为 146 tests；不影响功能结论，但报告数字需在后续统一。 | 后续文档清理时修正。 |

**确认结果：**

- `drainLatest()` 可等待 in-flight 保存及 pending-latest；返回/卸载超时后才关闭旧 session。
- `saveGeneration` 与 sessionId/bookId 绑定，旧队列不会把新书 Locator 写给旧书。
- 仅当响应对应最新 Locator 时才校正进度，旧响应不会造成 UI 百分比回退。
- deferred save + back、超时、跨书/卸载和旧响应测试均通过。
- `saveReadingState` 的 storage 调用已移入 `spawn_blocking`。

**AC 结果：**

- FR-160–FR-164：通过。
- FR-165–FR-167：通过。
- FR-168/FR-169、AC-160、NFR-007/NFR-008：自动化关闭顺序和失败/超时路径通过。
- NFR-009–NFR-012：通过。

**后续：**

- `TASK-M5E-A` 标记 `accepted`。
- 真实 Tauri 窗口关闭、强制终止恢复和完整应用 UI 证据由 `TASK-M5E-C` 覆盖。
- M5E-B/C 继续保持 `draft`，未自动启动；未创建 commit。
