# TASK-M5A：阅读会话、打开与关闭书籍

**状态：** accepted
**里程碑：** M5
**负责人：** DeepSeek harness
**创建人：** Codex
**创建日期：** 2026-11-15
**依赖：** TASK-M3D、TASK-M4D accepted

## 1. 目标

以稳定、可测试的 `openBook`/`closeBook` 契约把书库记录、原文件校验、已存 Locator、
阅读设置和 M3 的 `epub://` 会话注册表连接起来，为后续阅读器 UI 提供完整初始会话。

## 2. 范围

### 必须完成

- 新增 `openBook(bookId)` 与 `closeBook(sessionId)` Tauri command。
- 打开 inactive/不存在、文件失效、文件内容变化和 manifest 损坏时返回稳定 AppError。
- 打开成功时创建 M3 会话，返回首章 `frameUrl`、TOC、初始 Locator、进度和有效设置。
- `closeBook` 删除会话，使对应 `epub://` URL 立即失效。
- 增加 Rust domain/storage 读取接口和前端 Zod command bridge。
- 保持原始文件只读，不复制正文，不暴露路径、协议 token 之外的敏感信息。

### 不属于本任务

- 阅读器页面、iframe 组件、打开动画或返回书库 UI。
- 保存阅读位置、进度防抖、书签、设置修改命令或主题视觉效果。
- 目录侧栏、翻页、滚动、键盘、沉浸式或文本复制。
- 数据库 schema/version 修改、新依赖、OCR、TXT/PDF 或高亮批注。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收 |
|---|---|---|
| FR | FR-059, FR-210 | 点击书籍所调用的打开接口可验证失效文件并返回原因；成功返回阅读会话 |
| FR | FR-070–FR-073 | 会话返回已存/默认阅读模式和设置，供后续 UI 使用 |
| FR | FR-120–FR-122 | 会话返回保留层级的 EPUB TOC |
| FR | FR-160–FR-162 | 返回已存 Locator；无记录时从阅读顺序开头开始 |
| FR | FR-220 | 会话和校验只使用本地数据库与源文件 |
| AC | AC-040, AC-120, AC-160, AC-210 | 打开、文件失效、TOC 与初始位置契约成立 |
| NFR | NFR-010 | 打开只读原文件，不复制、改写或删除 |
| NFR | NFR-011 | 不发起网络请求 |
| NFR | NFR-012 | 打开失败显示稳定原因，不泄漏完整路径或底层异常 |

## 4. 公开契约

```ts
openBook(bookId: string): Promise<ReaderSession>
closeBook(sessionId: string): Promise<void>
```

`openBook` 调用参数严格为 `{ bookId }`，`closeBook` 严格为 `{ sessionId }`。二者使用
camelCase command 名和参数。

```ts
type ReaderSession = {
  sessionId: string              // UUID v4
  bookId: string
  title: string
  frameUrl: string              // epub://<token>/<首章相对路径>
  toc: TocNode[]
  initialLocator: ReaderLocator | null
  initialProgression: number    // 0..=1 有限数
  settings: ReaderSettings
}

type TocNode = {
  id: string
  label: string
  spineIndex: number
  fragment?: string
  children: TocNode[]
}

type ReaderSettings = {
  mode: 'paged' | 'scrolling'
  fontScale: number
  theme: 'day' | 'night'
}
```

- 不单独返回 session token；后续 UI 从受严格校验的 frame URL 第一段取得，不允许把
  token 写入日志、URL query、数据库或事件。
- `bookId` 只允许非空白字符串；`sessionId` 必须是 M3 生成的 UUID v4。
- `frameUrl` 必须指向该会话首章，token 与会话注册表一致，资源路径必须 URL 安全编码。
- TOC 使用已持久化 `ParsedBook.toc`，保留顺序、嵌套和 fragment；不得重新抓取正文。

## 5. 打开书籍流程

### 5.1 读取记录

- Storage 增加按 bookId 读取 active 书籍的只读 DTO，至少包含：
  `id`, `fingerprint`, `source_path`, `title`, `manifest_json`, `manifest_version`,
  `last_opened_at`。
- 不存在或 lifecycle 不是 `active` 时返回 `E_BOOK_NOT_FOUND`。
- 同时读取 `reading_states`、`book_settings` 和现有全局默认；空值表示使用内置默认。
- 不把 SQLite row、绝对路径或数据库错误文本传递给前端。

### 5.2 源文件校验

按顺序校验：

1. `source_path` 可解析为普通文件且可读；失败返回 `E_BOOK_UNAVAILABLE`。
2. 使用 `epub_core::fingerprint_file` 重新计算 SHA-256。
3. 指纹与数据库记录不一致返回 `E_SOURCE_CHANGED`。
4. `manifest_json` 可解析、版本受支持、spine 非空、首章 href 合法。
5. 任一步失败都不得创建 session、修改书籍 lifecycle 或个人数据。

文件失效和源内容变化必须保留书库记录、阅读进度、设置和书签。错误 message 使用固定中文，
不得包含完整路径、SQLite/rbook 类型或原始异常。

### 5.3 初始位置和设置

- `reading_states.locator_json` 存在时必须按 `ReaderLocator` 严格反序列化，版本必须为 1。
- Locator 的 `spineIndex/spineHref` 必须能在当前 manifest/spine 中找到；否则返回
  `E_STORAGE`，保留原记录并提示数据需修复，不得静默覆盖。
- `global_progression` 必须为 0..=1 的有限数；越界同样返回 `E_STORAGE`。
- 无 reading state 时返回 `initialLocator=null`、`initialProgression=0`，由后续阅读器从
  阅读顺序开头开始。
- 设置合并顺序固定为：内置默认 → 全局默认 → 单书覆盖。
  - 内置默认：`paged`、`1.0`、`day`。
  - `book_settings` 的 `NULL` 字段表示继承。
  - fontScale 仅接受有限数 `0.5..=3.0`；非法持久化值返回 `E_STORAGE`。
- 本任务只读取设置；修改命令由 M5D 实现。

### 5.4 创建会话

- 所有校验和解析完成后，以 canonical source path 与解析后的 `ParsedBook` 创建
  `SessionRegistry` 会话。
- 创建成功后更新 `last_opened_at`；更新失败不撤销已创建会话，但返回 `E_STORAGE` 前必须
  清理刚创建的会话，避免半开放状态。
- `frameUrl` 以首章 `spine[0].href` 构造，不默认加载 OPF、容器文件或任意未声明条目。
- 会话创建后返回 DTO；任何响应都不得包含 canonical source path 或 session token 字段。

## 6. 关闭会话

- `closeBook` 只关闭给定进程内会话，不删除书籍、进度、设置或书签。
- 未知/已关闭 sessionId 返回 `E_SESSION_INVALID`，不得影响其他会话。
- 成功后该 session 的 `epub://` 资源请求立即变成固定 404。
- 重复 close 不做隐式成功；调用方负责避免同一会话重复提交。
- 关闭不触碰原文件。

## 7. 错误映射

| 场景 | code |
|---|---|
| bookId/sessionId 格式无效 | `E_INVALID_INPUT` |
| 无 active 书籍 | `E_BOOK_NOT_FOUND` |
| 源文件缺失、非普通文件、不可读 | `E_BOOK_UNAVAILABLE` |
| 指纹不一致 | `E_SOURCE_CHANGED` |
| manifest/settings/state 损坏 | `E_STORAGE` |
| session 不存在/已关闭 | `E_SESSION_INVALID` |
| 无法分类的内部失败 | `E_INTERNAL` |

错误不得暴露内部细节。日志只记录稳定 code、book/session ID 和阶段。

## 8. 文件权限

### 允许修改

- `crates/reader-model/`（仅 ReaderTheme/ReaderSettings/ReaderSession 共用模型）
- `crates/storage-core/`（不修改 schema，仅增加读取与 last_opened 更新）
- `crates/library-core/`（打开书籍用例及错误映射）
- `src-tauri/src/commands.rs`、`dto.rs`、`lib.rs`、`state.rs`、`session/`
- `src-tauri/tests/`
- `src/bridge/commands.ts`、`ipc-schemas.ts`、`commands.test.tsx`
- 本任务状态和第 10 节开发报告

### 禁止修改

- `src/app/`、`src/features/`、页面布局或样式
- `src/reader-engine/`、父/iframe 协议语义
- 数据库 schema/version、CSP、capability、文件/网络权限
- `package.json`、锁文件、任何依赖
- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、ADR 和其他任务
- 原始 EPUB、用户真实书库或个人阅读数据

## 9. 测试要求

### Storage/library

- active 书籍读取；removed/不存在返回 not found。
- 有效 reading state/settings 读取和内置默认。
- 单书 NULL 覆盖项继承全局；非法 fontScale/progression/Locator 返回 storage error。
- 文件失效、目录、不可读、指纹变化、manifest 损坏/版本不支持均不创建会话。
- 原文件 bytes 前后不变。

### Session/DTO/commands

- 成功 openBook 返回 UUID v4 sessionId、首章 frame URL、TOC、Locator、进度和 settings。
- frame URL 实际可被 M3 协议读取首章，token 与 URL 一致但不作为独立字段返回。
- 多个会话独立；close 一个不影响另一个。
- close 后 frame URL 404；未知/重复 close 返回 `E_SESSION_INVALID`。
- last_opened_at 成功更新；模拟更新失败时会话被清理。
- 响应和错误 JSON 不含 source_path、token 字段或 SQLite/rbook 类型。
- Tauri mock command 参数名严格 camelCase；前端 Zod 正/负向契约测试。

### 必跑命令

```bash
npm run check
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri build
```

## 10. 禁止事项

- 不实现阅读器 UI 或 M5B–M5E 功能。
- 不新增依赖、数据库列或 schema migration。
- 不在打开时重新导入、重新生成封面或修改 fingerprint。
- 不把完整路径、正文或 token 写入日志/响应 cache。
- 不提交、推送或发布。

## 11. DeepSeek 开发报告

> 仅在开始实现后填写。开始实现时将状态改为 `in_progress`。

**完成摘要：**

实现 `openBook`/`closeBook` 的完整数据链路：storage 只读记录、library 打开用例、
M3 会话创建、frame URL、TOC 映射、初始 Locator 与设置合并、前端 Zod 命令桥。

第 12 节复验返工修复：

1. **空 fragment 契约**：TOC href 以 `#` 结尾（空 fragment）时后端省略
   `fragment`（`None`），不再序列化 `"fragment":""`；新增真实契约测试：NAV 含
   `chapter2.xhtml#` → `fragment == None` 且序列化 JSON 不含空 fragment。
2. **前端严格路径校验**：
   - `ReaderLocator.spineHref` 采用与 M3 相同的相对路径规则（非空、无 NUL/反斜杠、
     不以 `/` 开头、拒绝空段/`.`/`..`）；
   - `frameUrl` 严格校验 `epub://` + 32/64 hex token + 资源路径；**先对整个资源
     路径单次 `decodeURIComponent`**，再检查 `/`、`\`、NUL、绝对路径、query/
     fragment 与 `.`/`..` 段（包括编码斜杠穿越）；
   - 新增前端负例测试覆盖 `../`、`/abs`、反斜杠、双斜杠、`?query`、`#frag`、
     `%2e%2e`、`%2e%2e%2f`、`%5c`、`%00`、`%2f`、短 token；并增加 percent-encoded
     中文路径正例。
3. **mark_opened 失败清理自动化**：把会话创建/记录拆为 `finalize_open(opened,
   sessions, mark_opened)`；新增单元测试注入 `mark_opened` 返回
   `BookError::Storage`，断言返回 `E_STORAGE` 且 `SessionRegistry` 为空（会话已清理）。
4. **E_SOURCE_CHANGED 文案中性化**：改为“源文件内容与书库记录不一致，请重新导入
   该书。”，同时适用于导入与打开场景。

- **reader-model**：新增 `ReaderTheme`、`ReaderSettings`（内置默认 paged/1.0/day，
  fontScale 0.5–3.0 校验）、`ReaderTocNode`、`ReaderSession`（camelCase，不含 token
  字段）。
- **storage-core**：新增只读记录 `OpenBookRecord`/`ReadingStateRecord`/
  `BookSettingsRecord` 与 `app_setting` 读取；`mark_opened` 使用显式事务更新
  `last_opened_at`（0 行返回错误）。新增 `open_book_record`/`reading_state`/
  `book_settings`/`app_setting`/`mark_opened` 服务方法与 worker 请求；未修改 schema。
- **library-core**：新增打开用例（`LibraryService::open_book` / `mark_opened`）：
  - active 记录不存在 → `BookNotFound`；文件缺失/非普通文件/不可读 →
    `BookUnavailable`；重新计算 SHA-256 不一致 → `SourceChanged`；
  - `manifest_json` 解析、schema/版本、spine 非空校验失败 → `Storage`；
  - `reading_states` 严格反序列化 Locator（version 1、spineIndex/href 在 spine 内、
    progression 有限 0..=1），非法 → `Storage`；
  - 设置合并：内置默认 → `app_settings` 全局 `reader_defaults` → `book_settings`
    非空覆盖；非法 mode/theme/fontScale → `Storage`；
  - TOC 递归映射到 spineIndex/fragment，保留顺序与嵌套；无法映射父节点提升可映射
    子项；
  - 打开前后原文件 bytes 不变。
- **Tauri 命令**：新增 `openBook`（参数 `{bookId}`）与 `closeBook`（`{sessionId}`，
  UUID v4 校验）：
  - `openBook` 在 spawn_blocking 中完成校验/解析，创建 M3 会话；`frameUrl` 为
    `epub://<token>/<首章 href>`（percent-encode 非安全字符，保留 `/`）；
  - `mark_opened` 失败时关闭刚创建的会话并返回 `E_STORAGE`；
  - `closeBook` 成功使 frame URL 立即 404；未知/重复 close 返回
    `E_SESSION_INVALID`；
  - 响应仅含 `ReaderSession` 契约字段，不含 token/source_path。
- **前端 bridge**：`ipc-schemas.ts` 增加 `readerModeSchema`/`readerThemeSchema`/
  `readerSettingsSchema`/`readerLocatorSchema`/递归 `readerTocNodeSchema`/
  `readerSessionSchema`/`openBookInputSchema`/`closeBookInputSchema`；
  `commands.ts` 增加 `openBook`/`closeBook`（严格参数、输出解析、close 成功必须
  `null`）。
- **测试**：storage 单测 3；library open_book 7（成功/TOC/默认、合法 locator、
  非法 locator/progression、设置合并、非法设置、缺失/removed/不可读/目录/指纹变化/
  manifest 损坏/版本不支持、空白 id、原文件 bytes）；m5_commands 6（成功会话/
  frame URL 协议读取/TOC/settings/last_opened、close 失效与重复、多会话独立、
  无效输入/未知书/未知会话、removed、缺失/变化文件）；commands.rs 单元 1
  （frame URL 编码）；前端 commands.test.tsx 新增 8 个（正/负 Zod、参数、错误）。
  全量 Vitest 86 tests。

**修改文件：**

- `crates/reader-model/src/session.rs`（新增）、`src/lib.rs`
- `crates/storage-core/src/open.rs`（新增）、`src/lib.rs`、`src/service.rs`
- `crates/library-core/src/open.rs`（新增）、`src/lib.rs`、`src/service.rs`、
  `src/error.rs`、`Cargo.toml`（dev-dep rusqlite）、`tests/open_book.rs`（新增）
- `src-tauri/src/commands.rs`、`src/error.rs`、`src/lib.rs`
- `src-tauri/tests/m5_commands.rs`（新增）
- `src/bridge/ipc-schemas.ts`、`src/bridge/commands.ts`、`src/bridge/commands.test.tsx`
- `docs/tasks/TASK-M5A.md`（状态与本第 11 节）

未修改 `src/app/`、`src/features/`、`src/reader-engine/`、父/iframe 协议语义、
数据库 schema/version、CSP、capability、`package.json`/锁文件；未新增依赖；
未提交/推送。

**关键设计选择：**

- 打开分两段：`LibraryService::open_book` 完成只读校验与数据组装；
  `openBook` 命令创建会话后调用 `mark_opened`，失败时关闭会话并返回 `E_STORAGE`，
  避免半开放状态。
- 全局设置键采用 `reader_defaults`（SDS 只规定合并顺序、未指定键名；与 M5D 的
  设置命令需要保持同一约定）。
- TOC 映射到 spine 使用 OPF 目录内相对路径匹配；无法映射的父节点提升其可映射
  子项，避免丢失章节。
- frame URL 由会话 token + percent-encoded 首章 href 构造，token 只出现在 URL；
  响应不单独返回 token/source_path。
- Locator/设置反序列化失败一律 `Storage`（保留原记录，不静默覆盖）。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | `tsc -b`、Biome、Vitest 10 文件 87 tests 通过 |
| `npm run build` | 0 | 主应用 Vite 构建成功（JS 405.36 kB） |
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 283 passed，0 failed，1 ignored |
| `npm run test:e2e` | 0 | Playwright WebKit 29 passed（未涉及本任务 UI） |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.79 MiB |

**AC 验证：**

- FR-059/FR-210：openBook 验证失效文件与指纹变化并返回稳定原因；成功返回会话。
- FR-070–FR-073：会话返回 settings（内置/全局/单书合并）与模式。
- FR-120–FR-122：返回持久化 TOC 的层级与 fragment。
- FR-160–FR-162：返回已存 Locator/进度；无记录时 null/0。
- FR-220：只使用本地 DB 与源文件，无网络调用。
- AC-040/AC-120/AC-160/AC-210：打开、失败、TOC 与初始位置契约成立。
- NFR-010：只读原文件；打开前后 bytes 断言。
- NFR-011：无网络请求（CSP/协议未变）。
- NFR-012：稳定中文错误（不泄漏路径/底层异常）；测试检查响应字段不含
  source_path/token。

**未验证项：**

- `mark_opened` 失败清理已通过 `finalize_open` 注入测试覆盖（单元测试）。
- 真实 Tauri WKWebView/真实书库的 openBook 端到端未测试（使用 tauri mock runtime
  与隔离临时 DB）。
- 复杂真实 EPUB 的 TOC（含大量无法映射节点、NCX）与 Locator 映射未回归。
- 全局设置键 `reader_defaults` 需要与 M5D 设置命令保持一致，尚未实现写入端。
- 前端未实现阅读器 UI（按任务范围）。

**风险与待决问题：**

- `BookError` 新增 `BookNotFound`/`BookUnavailable` 变体，后续所有 match 需覆盖
  （已更新 src-tauri error 映射；其他调用方使用通配或未涉及）。
- TOC 无法映射节点的“提升子项”策略可能改变目录层级，需真实书籍验证。
- `mark_opened` 使用写事务，频繁打开时与导入/软移除共享 storage 线程，可能排队。
- frame URL 含 token；前端日志/缓存不得记录该 URL（后续 UI 需遵守）。
- 全局设置键名未在 SDS 固定，若 M5D 采用不同键名需迁移约定（无 schema 变更）。

**依赖变更：**

- 无新增运行时依赖；library-core 仅新增 dev-dependency `rusqlite`（已存在 workspace
  依赖，用于测试直接准备 reading_states/book_settings）。

## 12. Codex 审查

**结论：** 通过
**审查日期：** 2026-11-15

### 终轮复核

- 空 TOC fragment 已规范化为 `None`，`chapter1.xhtml#` 正例通过。
- `spineHref` 与 M3 安全路径规则一致，拒绝绝对路径、反斜杠、空段和 `.`/`..`。
- frameUrl 先整体单次解码，再检查编码斜杠、反斜杠、NUL、query/fragment、绝对路径和穿越段；
  合法 UTF-8 编码章节路径通过，`%2e%2e%2f`、`%5c`、`%00`、`%2f` 均被拒绝。
- `finalize_open` 注入测试证明 mark-opened 失败时会话注册表恢复为空并返回 `E_STORAGE`。
- SourceChanged 文案已同时适用于导入和打开操作。
- 未发现新增响应字段、路径泄漏、原文件修改或范围外依赖。

### 独立执行的检查

- `npm run check`：通过；10 files、87 tests，stderr 干净。
- `npm run build`：通过。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：282 passed、0 failed、1 ignored。
- `npm run tauri build`：通过；`Qurner.app` 7.79 MiB。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现 `BLOCKER` 或 `MAJOR` 问题。 | — |

**AC 结果：**

- FR-059、FR-210：通过；打开接口、失效路径和源内容变化错误成立。
- FR-070–FR-073：通过；会话返回模式与合并设置。
- FR-120–FR-122：通过；嵌套 TOC 和 fragment 映射成立。
- FR-160–FR-162：通过；恢复 Locator/进度及新书默认起点成立。
- AC-040、AC-120、AC-160、AC-210：当前后端契约通过。
- NFR-010–NFR-012：通过；原文件只读、无网络、稳定且不泄漏错误。

**返工或后续任务：**

- 无。
- 真实 WKWebView 的 openBook 端到端由 M5B 阅读器壳集成时验证。
