# TASK-M2D：Tauri 命令与 M2 集成验收

**状态：** accepted  
**里程碑：** M2  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** TASK-M2C accepted

## 1. 目标

将 `library-core` 接入 Tauri，用稳定的 IPC 契约提供异步导入、书库查询和软移除，
并完成应用启动、错误映射、并发和 M2 集成验收。

## 2. 范围

### 必须完成

- 实现 `startImport`、`listBooks`、`softRemoveBook` 三个 Tauri command。
- 实现 `import-progress` 和 `import-completed` 事件。
- 在应用启动时解析并初始化 `com.qurner.reader` 应用数据目录。
- 定义完整 `AppError`，将领域错误转换成 SDS 9.1 的 14 个稳定 code。
- 使用 Tauri mock IPC 和临时数据库完成命令、事件、错误和并发测试。
- 完成 1,000 本书库下的查询/导入集成验证和 Tauri 构建。

### 不属于本任务

- `openBook`、阅读会话、阅读进度、设置和书签命令。
- 文件选择器、拖放、导入汇总、书库和阅读器前端 UI。
- 封面生成、`cover://` 协议处理和 `epub://` 自定义协议。
- 导入取消、断点续传、后台扫描或文件系统监控。
- 数据库 schema、迁移或 M2A–M2C 已接受的导入/查询语义。
- 完整 M6 网络监测、发布、公证或自动更新。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收要求 |
|---|---|---|
| FR | FR-010–FR-014 | command 接收用户明确提供的 1–1,000 个路径；不扫描目录、不复制原文件 |
| FR | FR-016–FR-019 | 重复、恢复和失败状态通过逐项结果返回，不覆盖已有数据 |
| FR | FR-030, FR-037–FR-039, FR-042–FR-045 | `listBooks` 接受查询、排序和方向并返回仅含 active 书籍的摘要 |
| FR | FR-057–FR-058 | `softRemoveBook` 仅软移除；再次导入同指纹返回 `reactivated` |
| AC | AC-010, AC-011, AC-031, AC-032 | 命令层可观察行为与 M2 用例层一致 |
| NFR | NFR-004 | 1,000 本书集成查询 p95 不超过 200 ms |
| NFR | NFR-009 | 并发导入/恢复不产生重复 active 记录或部分写入 |
| NFR | NFR-011 | 不新增网络插件、远端请求代码或超出 `core:default` 的能力 |
| NFR | NFR-012 | 损坏文件、失效路径、初始化失败和数据库错误返回稳定、可理解错误 |

## 4. 设计约束

### 4.1 命令与参数

前端公开命令名必须严格为：

```ts
startImport(paths: string[]): Promise<{ jobId: string }>
listBooks(input: {
  query?: string
  sort: 'title' | 'author' | 'importedAt'
  direction: 'asc' | 'desc'
}): Promise<BookSummary[]>
softRemoveBook(bookId: string): Promise<void>
```

对应调用参数分别为 `{ paths }`、`{ input }`、`{ bookId }`。Tauri 函数可使用 Rust
snake_case 名称，但必须通过 command 的 `rename`/`rename_all` 暴露上述名称和参数键，
不得要求前端发送库内类型或额外适配层。

命令必须做运行时参数校验：

- `paths` 长度为 1–1,000；空数组、空白路径和超过上限返回
  `E_INVALID_INPUT`，不得创建 job 或后台任务。
- `listBooks` 的查询字段、排序和方向无效时返回 `E_INVALID_INPUT`。
- 空白 `bookId` 返回 `E_INVALID_INPUT`。
- 可使用 `tauri::ipc::Request`、`serde_json::Value` 或等价方式，使 handler 内的参数
  解析失败也转成 `AppError`，不得把框架默认参数错误文本直接暴露给前端。

### 4.2 异步导入与事件

`startImport` 在完成输入校验并为任务取得执行资格后立即返回 UUID v4 `jobId`。指纹计算、
EPUB 解析和数据库调用必须运行在阻塞任务中，不得阻塞 Tauri UI/event loop。

同一 job 按输入顺序逐本调用 `LibraryService::import`，第一本失败不阻断后续书籍。
不同 job 的 CPU 导入并发上限为 2；超出的 job 可以排队，但 `startImport` 仍应及时返回。

`import-progress` 载荷：

```ts
type ImportProgressEvent = {
  jobId: string
  completed: number
  total: number
  item: ImportItemResult
}
```

每个输入路径恰好产生一个进度事件，`completed` 从 1 连续增加到 `total`。
`ImportItemResult` 必须为 camelCase DTO：

```ts
type ImportItemResult = {
  sourceName: string
  status: 'imported' | 'reactivated' | 'duplicate' | 'failed'
  bookId?: string
  error?: AppError
}
```

`sourceName` 只包含文件 basename，不暴露绝对路径。任务结束必须发出一次
`import-completed`，载荷为 `{ jobId, items }`；即使部分或全部书籍失败也要发出。
事件发送失败不得改变已完成的数据库结果，但必须记录稳定错误摘要。

### 4.3 应用状态

- 使用 Tauri 路径解析器取得应用数据目录，生产路径应为
  `~/Library/Application Support/com.qurner.reader/`。
- 创建目录并以 `reader.sqlite3` 调用 `StorageService::open`，不硬编码用户主目录。
- 初始化失败不得 panic 或退出正常 UI；全局应用状态保存该 `AppError`，三个业务命令
  均立即返回同一个 `E_STORAGE`，且不能执行任何业务写入。
- 初始化日志只记录稳定 code、阶段和底层错误类别，不记录绝对路径、书籍正文或 SQL。
- M2D 没有 UI，因此“向用户显示”的可观察验收点是命令桥能够稳定取得错误；视觉呈现
  由 M4 完成。
- 不创建网络连接、不启动目录扫描或文件监控。

### 4.4 DTO

返回给前端的书库摘要必须为：

```ts
type BookSummary = {
  id: string
  title: string
  author: string
  coverUrl: string | null
  importedAt: string
  lastOpenedAt: string | null
}
```

`cover_cache_key = null` 映射为 `coverUrl = null`，非空键映射为
`cover://<cover_cache_key>`。当前任务不注册封面协议，数据库现阶段也没有非空缓存键，
因此 M2D 不负责使封面实际可加载；不得把数据库行、绝对路径或 `BookSummary` 的领域
内部字段直接泄漏为未约定字段。

### 4.5 AppError

跨 IPC 错误只包含：

```ts
type AppError = {
  code: AppErrorCode
  message: string
  details?: Record<string, string | number | boolean>
}
```

必须定义并序列化以下 14 个 code：

`E_INVALID_INPUT`、`E_FILE_NOT_FOUND`、`E_FILE_UNREADABLE`、`E_EPUB_INVALID`、
`E_EPUB_ENCRYPTED`、`E_EPUB_FIXED_LAYOUT`、`E_EPUB_VERTICAL`、
`E_EPUB_SCRIPT_REQUIRED`、`E_BOOK_NOT_FOUND`、`E_BOOK_UNAVAILABLE`、
`E_SOURCE_CHANGED`、`E_SESSION_INVALID`、`E_STORAGE`、`E_INTERNAL`。

`BookError` 和 `EpubErrorCode` 至少按以下规则映射：

| 领域错误 | AppError code |
|---|---|
| `BookError::InvalidInput` | `E_INVALID_INPUT` |
| `BookError::FileNotFound` / `EpubErrorCode::FileNotFound` | `E_FILE_NOT_FOUND` |
| `BookError::FileUnreadable` / `EpubErrorCode::IoError` | `E_FILE_UNREADABLE` |
| `EpubErrorCode::EncryptedContent` | `E_EPUB_ENCRYPTED` |
| `EpubErrorCode::FixedLayout` | `E_EPUB_FIXED_LAYOUT` |
| `EpubErrorCode::VerticalWriting` | `E_EPUB_VERTICAL` |
| `EpubErrorCode::ScriptDependency` | `E_EPUB_SCRIPT_REQUIRED` |
| 其余 `EpubErrorCode` | `E_EPUB_INVALID` |
| `BookError::SourceChanged` | `E_SOURCE_CHANGED` |
| `BookError::Storage(_)` | `E_STORAGE` |
| `BookError::Internal` | `E_INTERNAL` |

`E_BOOK_NOT_FOUND`、`E_BOOK_UNAVAILABLE` 和 `E_SESSION_INVALID` 在本任务建立稳定契约并
验证序列化，但本任务的三个命令不会打开书籍或创建会话，不要求制造虚假调用路径。
`message` 使用固定中文说明，并在可恢复错误中包含下一步提示；不得包含绝对路径、
SQLite 原文、rbook/rusqlite 类型或书籍正文。无 `details` 时字段必须省略。

## 5. 文件权限

### 允许修改

- `src-tauri/`
- `crates/library-core/`（仅 IPC 适配确有需要时）
- 根 `Cargo.toml`、`Cargo.lock`、`src-tauri/Cargo.toml`（仅接入已授权且已锁定的依赖）
- `src-tauri/tests/` 及任务专用测试夹具
- 本任务状态和第 9 节开发报告

### 禁止修改

- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`
- `src/`、前端依赖、CSP、Tauri capability
- `crates/storage-core/`、`crates/epub-core/`、`crates/reader-model/` 和数据库 schema
- 其他任务文件

不得新增外部 crate、Tauri plugin 或运行时网络依赖。M2 已授权的 `uuid` 和既有 workspace
依赖可以显式加入 Tauri crate；允许启用 Tauri 已锁定的 `test` feature，不得借此引入新包。
遇到必须修改禁止路径才能继续的问题时，标记 `blocked` 并报告，不得自行扩展权限。

## 6. 行为与测试要求

### 6.1 正常路径

- 单本有效 EPUB：命令返回唯一 jobId，1 个 progress + 1 个 completed，状态为
  `imported`。
- 多本混合批次：按输入顺序产生与书籍数量相同的 progress，最终 items 顺序和状态一致。
- 相同文件再次导入：`duplicate`，已有记录不变。
- 软移除后再次导入：`reactivated`，bookId 与原记录一致。
- `listBooks` 对查询、三种排序和升降序的透传及 camelCase DTO 正确。
- `cover_cache_key` 的 null/非空值分别映射为 `coverUrl` 的 null/协议 URL。
- `softRemoveBook` 后列表不再返回该 active 书籍。

### 6.2 错误路径

- 空批次、空白路径、超上限批次、非法查询枚举和空白 bookId 返回
  `E_INVALID_INPUT`。
- 不存在文件、目录、不可读文件、损坏 EPUB、加密/固定版式/竖排/脚本依赖 EPUB 返回
  对应稳定 code，且不会留下部分记录。
- 应用数据目录不可创建、数据库损坏或不支持 schema 时，命令返回 `E_STORAGE`，后续
  命令仍返回稳定错误且应用不崩溃。
- 错误序列化中不得出现 `/Users/`、测试临时绝对路径、`rusqlite`、`rbook` 或 SQL 文本。

### 6.3 测试证据

- 单元测试覆盖 14 个 `AppErrorCode` 的 JSON 字符串、完整 `AppError` 往返、可省略
  `details` 以及全部现有领域错误映射。
- Tauri mock IPC 必须实际调用三个公开 command 名和参数键，验证成功 DTO、错误对象、
  参数校验和状态不可用路径；不能只直接调用内部 helper。
- 以可控事件 sink 或 mock app 捕获事件，验证 jobId 隔离、事件数量、顺序、completed
  递增、sourceName 不含绝对路径和最终 summary。
- 并发测试使用临时数据库。预置 1,000 本 active 书籍，并发执行至少 10 次查询/排序和
  至少 2 个导入 job；断言无 `StorageError::Busy`、无意外 `E_STORAGE`、无重复 active
  指纹。查询 p95 不超过 200 ms，并记录测试构建类型。
- 两个 job 同时导入同一文件时只产生一个 imported，其余为 duplicate/reactivated；
  数据库只有一个 active 指纹。
- 启动测试验证数据目录和 `reader.sqlite3` 的创建，且测试不得读写真实用户应用数据。
- 静态检查 Tauri capability 仍仅为 `core:default`，`src-tauri/Cargo.toml` 不含
  shell/http/fs/dialog/network plugin，命令实现不含网络调用。完整运行时网络监测留 M6。

## 7. 必跑命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p qurner --lib
cargo test -p qurner --test m2_commands
cargo test -p library-core
cargo test --workspace
npm run check
npm run build
npm run tauri build
```

## 8. 禁止事项

- 不提交、不推送、不创建 PR。
- 不安装新依赖、不访问外部网络、不修改用户原始电子书或个人数据。
- 不实现 M3/M4/M5 功能，不修改前端 UI、CSP 或 capability。
- 不以直接插入测试数据替代正常导入行为测试；性能测试可以单独直接预置数据。
- 不跳过、删除或弱化失败测试，不把未验证内容写成通过。

## 9. DeepSeek 开发报告

> 仅在开始实现后填写。开始实现时将状态改为 `in_progress`。

**完成摘要：**

将 `library-core` 接入 Tauri，完成 IPC 契约、导入事件、启动初始化、错误映射与 M2
集成验收；本轮按第 10 节复验继续修复 `softRemoveBook` 的异步化遗漏。

基础实现：

- 三个公开命令（严格 camelCase 名称，参数键 `paths` / `input` / `bookId`）：
  - `startImport`：校验 1–1,000 个非空白路径，立即返回 UUID v4 `jobId`，在
    `tauri::async_runtime::spawn_blocking` 中逐本调用 `LibraryService::import`；
  - `listBooks`：解析 `{ input: { query?, sort, direction } }`，返回
    `BookSummary[]`（camelCase，`coverUrl` 由缓存键映射 `cover://<key>` 或 null）；
  - `softRemoveBook`：校验非空 `bookId` 后软移除。
- 事件：每个路径一次 `import-progress`（jobId、completed 1..total、camelCase
  `ImportItemResult`），任务结束一次 `import-completed`（{ jobId, items }）；发送失败
  只记录稳定错误摘要，不改变数据库结果。
- 不同 job 的导入并发上限为 2（`JobScheduler`，按 job 占用槽位，超出者在阻塞任务中
  排队）；`startImport` 仍立即返回。
- 启动：`setup` 使用 Tauri 路径解析器取得应用数据目录，创建目录并以
  `reader.sqlite3` 调用 `StorageService::open`；失败时保存同一 `E_STORAGE` 状态，三个
  命令均立即返回该错误且不执行写入；日志只记录阶段与稳定 code。
- `AppError`：14 个 SDS 9.1 code 全部定义并 camelCase 序列化，`details` 无值时省略；
  `BookError`/`EpubErrorCode` 按任务表格映射；message 为固定中文且不含路径、SQLite、
  rbook 类型或正文。

第 10 节复验后修复（本轮）：

1. **`softRemoveBook` 异步化（源码核实）**：上一轮报告声称已改为异步，但源码仍是
   同步 command；本轮改为 `pub async fn`，`book_id` 通过 `parse_book_id(&body)?`
   取得后立即 `.to_string()` 为 owned `String` 并移入
   `tauri::async_runtime::spawn_blocking`，`.await` 结果并用
   `map_err(|_| AppError::new(AppErrorCode::Internal))` 映射任务失败。参数校验、DTO
   与 `AppError` 映射契约不变。
2. **三命令主线程行为核对**：
   - `startImport`：同步命令，但只做参数校验、生成 jobId 并启动 `spawn_blocking`，
     不等待导入；
   - `listBooks`：async + `spawn_blocking` 等待存储线程；
   - `softRemoveBook`：本轮改为 async + `spawn_blocking`。
3. 重新运行第 7 节全部命令，并更新本报告与源码一致。

此前已修复并复验通过（第 10 节确认）：

- 路径保真：`parse_paths` 只用 `trim()` 的临时值判断全空白，入队/导入保留原始
  字符串；尾随空格文件名 mock IPC 回归测试通过。
- `AppError.details`：`DetailValue` 仅接受 bool / number / string；对象、数组、null
  反序列化失败；无 details 时省略。
- 真实初始化失败：目录不可创建、数据库损坏、schema 版本 99 三类均通过
  `initialize_state` + 临时路径验证三命令返回 `E_STORAGE`、无事件、无 panic。
- 并发上限：3 个大文件 job，观测最大并发恰为 2、任一时刻 ≤2、全部完成、无重复
  active。

**修改文件：**

- `src-tauri/Cargo.toml`（library-core、uuid、serde、serde_json；dev 启用 tauri
  `test` feature；dev 增加 rusqlite 用于 schema 版本夹具）
- `src-tauri/src/lib.rs`（命令注册、setup 初始化）
- `src-tauri/src/error.rs`（14 code、映射、`DetailValue` 与 details 契约）
- `src-tauri/src/dto.rs`（BookSummary/导入结果/事件载荷）
- `src-tauri/src/state.rs`（AppState、JobScheduler、`active_count`、initialize_state）
- `src-tauri/src/commands.rs`（三命令、路径保真、async list/remove、导入 runner）
- `src-tauri/tests/m2_commands.rs`（mock IPC 集成测试）
- 根 `Cargo.toml`（workspace.dependencies 增加 `library-core` 路径依赖）
- `Cargo.lock`
- `docs/tasks/TASK-M2D.md`（顶部状态与本第 9 节）

未修改 `src/`、前端依赖、CSP、Tauri capability、`crates/storage-core/`、
`crates/epub-core/`、`crates/reader-model/` 和数据库 schema，未创建 commit。

**关键设计选择：**

- 命令参数使用 `tauri::ipc::Request` + `serde_json::Value` 手动解析，确保框架参数
  错误也转换为 `E_INVALID_INPUT`，不泄漏框架默认错误文本。
- `startImport` 的 `AppHandle` 参数为泛型 `R: tauri::Runtime`，使同一命令实现可在
  Wry 生产构建与 MockRuntime 测试中复用。
- 状态保存 `Result<Arc<LibraryService>, AppError>`：初始化失败时三个命令返回同一个
  `E_STORAGE`，不 panic、不退出 UI。
- 并发调度按 job 粒度（每个 job 占一个槽位，job 内按输入顺序串行），符合“不同 job
  的 CPU 导入并发上限 2”；`active_count()` 仅为测试观测，不参与业务逻辑。
- `listBooks`/`softRemoveBook` 的阻塞存储调用放入 `spawn_blocking`，命令 future 不
  占用主线程；参数校验仍在主线程快速完成。
- `DetailValue` 使用 untagged 枚举限制跨 IPC 标量，避免公共契约被扩大。
- 初始化失败测试全部使用临时目录与临时数据库，不读写真实
  `~/Library/Application Support/`。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p qurner --lib` | 0 | 13 passed（含 details 标量契约测试） |
| `cargo test -p qurner --test m2_commands` | 0 | 13 passed（含路径保真、真实初始化失败、3-job 并发上限） |
| `cargo test -p library-core` | 0 | 19 passed（6 lib、8 import、5 list） |
| `cargo test --workspace` | 0 | 206 passed，0 failed，1 ignored |
| `npm run check` | 0 | Biome 14 个文件无修复；Vitest 2/2 通过 |
| `npm run build` | 0 | Vite 构建成功（dist 223.70 kB JS） |
| `npm run tauri build` | 0 | release 30.98s；`Qurner.app` 6.97 MiB |

1,000 本并发集成测试（mock IPC，debug 测试构建，查询与导入 job 并发）：

```text
debug build p95 query = 7.023917ms (10 runs, max)
```

**AC 验证：**

- AC-010 / FR-010–FR-014：单本/多本混合导入、尾随空格路径保真、空白路径拒绝均通过
  mock IPC；不扫描目录、不复制文件。
- AC-011 / FR-016–FR-019：相同文件 `duplicate`、软移除后再导入 `reactivated` 且
  bookId 一致、失败项无残留记录。
- AC-031 / AC-032 / FR-030、FR-037–FR-039、FR-042–FR-045：`listBooks` 三种排序 ×
  升降序、搜索、`coverUrl` null、camelCase 字段通过；异步命令契约不变。
- FR-057–FR-058：`softRemoveBook` 后列表隐藏，再次导入恢复；命令为 async 且
  `spawn_blocking`，不再同步等待存储线程。
- NFR-004：1,000 本并发查询 p95 7.02 ms（debug，10 次最大）≤ 200 ms。
- NFR-009：两个并发 job 导入同一文件 → 一个 `imported`、一个 `duplicate`；3 个 job
  测试中最大并发 2、全部完成、无重复 active。
- NFR-011：capability 仍为 `["core:default"]`；无插件/网络依赖（静态检查）。
- NFR-012：真实初始化失败（目录不可创建、数据库损坏、更高 schema）均返回
  `E_STORAGE` 且不 panic；7 类导入错误码保持稳定映射。
- `AppError.details`：仅字符串、数字、布尔可序列化；对象/数组/null 被拒绝；无
  details 时字段省略。

**未验证项：**

- 未在真实 macOS WebView 中调用命令或接收事件；全部命令/事件测试使用 MockRuntime。
- 启动测试使用临时应用数据目录，未读写真实用户 `~/Library/Application Support/`。
- 未在 release 构建下重测 1,000 本 p95；7.02 ms 为 debug mock IPC 测量值。
- 未构造只读文件系统或磁盘满验证 `E_STORAGE`（覆盖了目录不可创建、数据库损坏、
  更高 schema 三类）。
- 3 个 job 的排队时序通过轮询 `active_count()` 观测，未使用精确时间线断言；大文件
  使 job 持续足够长以保证观察到并发 2。
- `coverUrl` 非空映射仅在 DTO 单元测试验证（数据库无写 cover_cache_key 的公开接口）。
- 未测试真实前端 TypeScript 类型/调用；`npm run build` 仅构建现有 M0 前端。
- 未运行/人工操作打包后的 `Qurner.app`。
- 未测试事件发送失败分支。
- 未对 `softRemoveBook` 的 `spawn_blocking` 任务失败分支做注入测试；映射为
  `E_INTERNAL` 的代码已存在，但未构造 JoinError。

**风险与待决问题：**

- 命令在唯一写线程上串行执行数据库操作；大量导入 job 会排队，`startImport` 仍立即
  返回，但事件延迟随队列增长；`active_count()` 是公开只读观测，未来若不希望暴露可
  改为测试 feature。
- 导入 job 无取消能力；进程退出时进行中的 `spawn_blocking` 任务可能被中断。
- 应用数据目录解析失败时统一返回 `E_STORAGE`，用户无法区分路径解析与数据库问题；
  只能依赖日志阶段。
- `startImport` 路径校验只判断非空，不验证文件存在；尾随空格路径依赖真实文件存在。
- `listBooks` 允许未知字段并忽略；未来契约收紧需改参数解析。
- 事件发送失败只记录日志，前端可能收不到 `import-completed`；M4 需超时兜底。
- `BookSummary` 含书名/作者，前端展示与日志需遵守隐私规则。
- 初始化失败状态不可恢复；需重启应用。
- `DetailValue` 使用 `serde_json::Number`，前端数值为 JSON number；极端精度（如
  u64 超 JS 安全范围）仍由 JSON 层决定，本任务无此类 details 生产者。

**依赖变更：**

- `src-tauri` 新增 `library-core.workspace = true`、`uuid.workspace = true`、
  `serde = { workspace = true }`、`serde_json = { workspace = true }`（均为 M2 已
  授权且已在锁文件的依赖）。
- 根 `Cargo.toml` 的 `workspace.dependencies` 增加 `library-core`（路径依赖）。
- `src-tauri` dev-dependencies 启用 `tauri` 的 `test` feature，并增加
  `rusqlite.workspace = true`（仅测试构造更高 schema 版本的数据库；rusqlite 已在
  workspace 依赖与锁文件中，非新增外部 crate）。
- 未新增外部 crate、Tauri plugin 或网络依赖；capability 未修改。

## 10. Codex 审查

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p qurner --lib`：13 passed。
- `cargo test -p qurner --test m2_commands -- --nocapture`：13 passed；1,000 本并发
  查询 p95 复测为 7.009959 ms；3 个 job 的最大并发观测为 2。
- `cargo test -p library-core`：19 passed。
- `cargo test --workspace`：206 passed，0 failed，1 ignored。
- `npm run check`：通过；Biome 14 个文件无修复，Vitest 2/2。
- `npm run build`：通过。
- `npm run tauri build`：通过；生成 6.97 MiB `Qurner.app`。

**本轮确认：**

- 路径校验只在临时值上判断全空白，实际导入保留原始字符串；尾随空格路径测试通过。
- `AppError.details` 已限制为字符串、数字或布尔；对象、数组、null 的反序列化测试通过。
- 目录不可创建、数据库损坏和更高 schema 版本均通过 `initialize_state` 的真实失败路径
  验证，三个命令返回 `E_STORAGE`。
- 3 个 job 的调度测试观察到最大并发 2，全部 job 完成且无重复 active 记录。
- `soft_remove_book` 已改为 `async fn`，`bookId` 转为 owned `String` 后移入
  `spawn_blocking`；存储错误保持原映射，任务失败映射为 `E_INTERNAL`。
- 开发报告中关于 `listBooks` 和 `softRemoveBook` 异步化的描述与源码一致。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现 `BLOCKER`、`MAJOR` 或影响当前 AC 的 `MINOR` 问题。 | — |

**AC 结果：**

- AC-010 / FR-010–FR-014：通过。
- AC-011 / FR-016–FR-019：通过。
- AC-031 / FR-030、FR-037–FR-039、FR-042–FR-045：通过。
- AC-032 / FR-057–FR-058：通过；查询和软移除均不在 Tauri 主线程等待存储线程。
- NFR-004：通过，独立复测 p95 约 7.01 ms。
- NFR-009：通过，双 job 判重和 3 job 并发上限均通过。
- NFR-011：通过当前任务要求的静态依赖与 capability 检查；完整运行时网络监测留 M6。
- NFR-012：通过，领域错误、初始化失败和稳定 `AppError` 均有验证。

**M2 结果：**

TASK-M2A–TASK-M2D 全部通过验收，M2 里程碑完成。真实 WebView 命令调用、事件发送
失败注入和打包应用人工操作留到后续 UI/集成阶段验证，不阻塞 M2 当前退出标准。
