# TASK-M4A：前端命令桥与导入事件契约

**状态：** accepted  
**里程碑：** M4  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** TASK-M2D、TASK-M3D accepted

## 1. 目标

建立唯一、类型安全且运行时校验的 Tauri 命令桥，覆盖导入、书库查询、软移除和导入事件，
为后续 M4 UI 提供稳定数据入口，同时不提前实现页面。

## 2. 范围

### 必须完成

- Zod schema 与 TypeScript 类型覆盖三个命令输入、输出、导入事件和 14 个错误码。
- 使用现有 `@tauri-apps/api` 实现命令 wrapper。
- 实现 `import-progress`/`import-completed` 订阅、校验和取消订阅。
- 非法响应、未知事件和非法 AppError 转成稳定内部错误。
- 使用可注入 transport 完成 Vitest，不要求真实 Tauri。

### 不属于本任务

- 文件选择器、拖放、导入页面或任何用户可见布局。
- 书库网格/列表、封面 UI、搜索/排序控件和软移除确认。
- TanStack Query、虚拟列表或缓存策略。
- Rust command、SQLite、Reader Engine 或协议修改。
- M5 阅读器功能。

## 3. 需求追踪

| 类型 | 编号 | 验收 |
|---|---|---|
| FR | FR-010–FR-019 | 导入命令、批量结果和逐项失败可供 UI 使用 |
| FR | FR-030, FR-037–FR-045 | 查询、排序和 direction 类型完整 |
| FR | FR-057–FR-058 | 软移除命令可供 UI 调用 |
| AC | AC-010, AC-011, AC-031, AC-032 | 后续 UI 数据契约可追溯 |
| NFR | NFR-004 | 查询 wrapper 不增加额外记录处理或 N+1 |
| NFR | NFR-012 | IPC 错误映射为稳定 AppError |

## 4. IPC 契约

### 4.1 命令

```ts
startImport(paths: string[]): Promise<{ jobId: string }>
listBooks(input: {
  query?: string
  sort: 'title' | 'author' | 'importedAt'
  direction: 'asc' | 'desc'
}): Promise<BookSummary[]>
softRemoveBook(bookId: string): Promise<void>
```

- invoke 名与参数键严格为 camelCase：`{ paths }`、`{ input }`、`{ bookId }`。
- `startImport` 校验 1–1,000 个非空白路径；不得 trim 实际路径。
- `BookSummary` 使用 `coverUrl`，不暴露 `coverCacheKey` 或路径。

### 4.2 事件

```ts
import-progress: {
  jobId: string
  completed: number
  total: number
  item: ImportItemResult
}
import-completed: {
  jobId: string
  items: ImportItemResult[]
}
```

- jobId 为 UUID v4；completed 1..=total；total 1..=1,000。
- status 仅 imported/reactivated/duplicate/failed。
- sourceName 只允许 basename，不得包含 `/` 或 `\`。
- failed 项必须带 error；其他状态不得带 error。
- 严格拒绝未知字段。

### 4.3 AppError

接受 M2D 的 14 个 code；details 只允许字符串、数字、布尔。未知 code、缺少 message、
对象/数组/null details 必须拒绝。

- 有效 Tauri rejection 保留 code/message/details。
- transport Error、非对象或 schema 不匹配统一转 `E_INTERNAL`。
- message 不拼接 stack、路径或底层错误。

## 5. 设计边界

- 新代码位于 `src/bridge/commands.ts`、`src/bridge/ipc-schemas.ts` 或等价模块。
- 不修改 M3 iframe 协议文件；Tauri command 与 iframe message schema 独立。
- 通过接口注入：

```ts
interface CommandTransport {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>
}

interface EventTransport {
  listen<T>(event: string, handler: (event: T) => void): Promise<UnlistenFn>
}
```

- 默认 transport 包装 `@tauri-apps/api/core::invoke` 和
  `@tauri-apps/api/event::listen`；Vitest 可注入 fake。
- 不访问文件系统、不解码正文、不记录路径或个人数据。
- 不新增外部依赖。

## 6. 文件权限

### 允许修改

- `src/bridge/`（Tauri command/event schema、wrapper 和测试）
- 测试支持文件
- `package.json` scripts（不得新增依赖）
- 本任务状态和第 9 节开发报告

### 禁止修改

- `src/app/`、`src/features/`、页面布局和样式
- `src/reader-engine/`、iframe `messages.ts/parent.ts/iframe.ts` 语义
- `src-tauri/`、Rust crates、数据库 schema、CSP/capability
- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、ADR 和其他任务

不得安装 TanStack Query、dialog plugin 或其他依赖。

## 7. 测试要求

- 三个 invoke 的名称、参数和成功返回解析。
- startImport 1/1,000 条成功，空、空白、超量、非字符串失败且零 invoke。
- listBooks 三 sort、两 direction、optional query 和非法 enum。
- softRemoveBook 空白/格式错误 bookId。
- BookSummary 与 ImportItemResult 正/负向 schema。
- progress/completed 的 jobId、completed/total、status、sourceName、bookId/error
  互斥和未知字段。
- 14 个 AppError 通过；非法 details、未知 code 和普通 Error 统一 E_INTERNAL。
- listener 注册精确事件名；unsubscribe 正确且不重复。
- 失败响应不泄漏 stack、路径或底层 message。

## 8. 必跑命令

```bash
npm run check
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri build
```

## 9. DeepSeek 开发报告

> 仅在开始实现后填写。开始实现时将状态改为 `in_progress`。

**完成摘要：**

新增前端 Tauri 命令桥，覆盖导入、书库查询、软移除与导入事件，全部 Zod 严格校验、
可注入 transport、Vitest 覆盖；未实现任何页面或 UI。

第 10 节复验返工修复：

1. **`BookSummary` 可空字段**：`coverUrl`、`lastOpenedAt` 改为 required + nullable
   （`z.string().min(1).nullable()`），与 SDS 9.3 及 M2D `BookSummaryDto` 的显式
   `null` 一致；测试改用真实后端载荷（`coverUrl: null`、`lastOpenedAt: null`），
   并保留拒绝 `coverCacheKey`、空字符串、字段缺失/undefined 的负例。
2. **`softRemoveBook` 成功响应**：成功解析器严格校验后端约定的 `null`，任何其他
   载荷（对象、字符串、undefined、数组、布尔、数字）都转 `E_INTERNAL`；新增正向
   `null` 与 6 类负向测试。

- `src/bridge/ipc-schemas.ts`：
  - 14 个 AppError code（`E_INVALID_INPUT` … `E_INTERNAL`）、strict `appErrorSchema`
    （message 非空、details 仅 string/number/boolean 标量）；
  - `bookSummarySchema`（id/title/author/coverUrl?/importedAt/lastOpenedAt?，拒绝未知
    字段，不暴露 coverCacheKey）；
  - `importItemResultSchema`（status 四值；sourceName 仅 basename；failed 必须带
    error 且不得带 bookId，非 failed 不得带 error）；
  - `importProgressEventSchema`（jobId UUID v4、completed 1..total、total ≤1000、
    未知字段拒绝）与 `importCompletedEventSchema`；
  - 三个命令输入 schema（paths 1–1000 非空白且保留原始字符串、listBooks sort/
    direction/可选 query、bookId 非空白）与 startImport 输出 schema。
- `src/bridge/commands.ts`：
  - `createCommandBridge(transport)` 封装 `startImport` / `listBooks` /
    `softRemoveBook`，invoke 名与参数键严格 camelCase：`{ paths }`、`{ input }`、
    `{ bookId }`；
  - `toAppError` 保留合法 AppError；普通 Error、非对象、schema 不匹配、非法 details
    统一固定 `E_INTERNAL`（message“内部错误”，不拼接 stack/路径/底层信息）；
  - 本地输入校验失败固定 `E_INVALID_INPUT`，且零 invoke；
  - `subscribeImportEvents(transport, handlers)` 精确注册 `import-progress`/
    `import-completed`，解析后转发 onProgress/onCompleted，非法载荷 onError；
    返回幂等 unsubscribe；第二个监听失败时清理第一个；
  - 默认 transport 包装 `@tauri-apps/api/core::invoke` 与
    `@tauri-apps/api/event::listen`（向桥暴露 payload）；导出默认 `commands` 实例。
- `src/bridge/commands.test.tsx`：18 个 Vitest，覆盖命令名/参数/成功解析、1/1,000
  条路径与非法路径零 invoke、三 sort×两 direction、可选 query、非法枚举、软移除
  bookId、BookSummary/ImportItemResult/progress/completed 正负 schema、14 个 AppError、
  非法 details/未知 code/普通 Error→E_INTERNAL、不泄漏路径与底层 message、事件注册
  精确名称、非法载荷、unsubscribe 幂等与监听失败清理。

**修改文件：**

- `src/bridge/ipc-schemas.ts`（新增）
- `src/bridge/commands.ts`（新增）
- `src/bridge/commands.test.tsx`（新增）
- `docs/tasks/TASK-M4A.md`（状态与本第 9 节）

未修改 `src/app/`、`src/features/`、`src/reader-engine/`、`src/bridge/messages.ts`、
`parent.ts`、`iframe.ts` 语义、`src-tauri/`、Rust crates、数据库 schema、CSP/
capability、SRS/SDS/DEVELOPMENT_PLAN/ADR；未新增依赖；未创建 commit。

**关键设计选择：**

- IPC 契约与 iframe 消息协议完全独立：新增 `ipc-schemas.ts`，不复用 M3 阅读消息。
- 输入在本地先校验：非法输入直接抛 `E_INVALID_INPUT`，不触碰 transport；路径仅用
  trim 判断空白，实际发送原始字符串。
- 输出严格解析：返回结构不符合 schema 时转 `E_INTERNAL`，不把 Zod 细节泄漏给 UI。
- 错误映射：合法 AppError 原样保留（14 码、message、标量 details）；其他一切
  rejection 固定 `E_INTERNAL`，message 不拼接底层信息。
- 事件桥只接受 payload，默认 transport 负责把 Tauri `Event<T>` 解包为 payload；
  unsubscribe 幂等，监听注册失败时清理已注册监听。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | `tsc -b`、Biome、Vitest 7 文件 46 tests 通过 |
| `npm run build` | 0 | 引擎 + 主应用 Vite 构建成功 |
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 248 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.14 MiB |

**AC 验证：**

- FR-010–FR-019：startImport 输入/输出、批量结果与逐项失败（ImportItemResult）
  通过 schema 与 wrapper 提供。
- MAJOR 修复：真实 `BookSummary` 显式 `null` 可解析；`softRemoveBook` 仅接受 `null`。
- FR-030、FR-037–FR-045：listBooks 三 sort、两 direction、可选 query 与 BookSummary
  类型完整。
- FR-057–FR-058：softRemoveBook wrapper 可供 UI 调用。
- AC-010/AC-011/AC-031/AC-032：数据契约可追溯至 Zod schema 与测试。
- NFR-004：listBooks 只透传一次 invoke，不做额外记录处理或 N+1。
- NFR-012：IPC 错误映射为稳定 AppError；非法响应/未知事件/普通 Error → `E_INTERNAL`。

**未验证项：**

- 未在真实 Tauri WebView 中调用默认 transport（Vitest 只注入 fake transport）；
  `@tauri-apps/api` 的实际 invoke/listen 行为未端到端验证。
- `coverUrl` 限制为非空字符串或 `null`，未限制 `cover://` 协议前缀；是否限制取决于
  后续 UI/协议约定。
- `softRemoveBook` 严格要求成功响应为 `null`；若未来后端改为返回其他值，需要同步更新
  契约与测试。
- listBooks query 空字符串是否应视为无查询由后端语义决定，前端原样透传。
- 未验证事件在高频导入下的背压/顺序（Tauri 事件顺序由 WebView 保证）。

**风险与待决问题：**

- 前端 schema 与 Rust DTO 需保持同步；若后端增加字段，strict schema 会拒绝并转
  `E_INTERNAL`，需要后续集成测试或契约演练。
- 事件监听失败会清理已注册监听并抛 `E_INTERNAL`，UI 需要提供重试入口（后续任务）。
- startImport 返回后导入在后台线程运行；UI 需先订阅事件再调用命令（后续 UI 任务）。

**依赖变更：**

- 无新增依赖；使用既有 `@tauri-apps/api` 与 `zod`。

## 10. Codex 审查

**结论：** 通过  
**审查日期：** 2026-11-14  

### 首轮问题复核

- `BookSummary.coverUrl`、`lastOpenedAt` 已改为 required + nullable；测试主载荷现在
  明确包含 `coverUrl: null`、`lastOpenedAt: null`，与 SDS 9.3 和 M2D 的 Rust DTO
  一致。
- `listBooks` 对真实非空书库载荷解析成功；缺失 nullable 字段、空字符串和
  `coverCacheKey` 未知字段仍会被拒绝并映射为 `E_INTERNAL`。
- `softRemoveBook` 只接受后端成功命令实际返回的 `null`；对象、字符串、`undefined`、
  数组、布尔和数字均被拒绝并映射为 `E_INTERNAL`。
- 未发现 UI、Rust、数据库、CSP/capability 或其他 bridge 协议的越界修改。

### 独立执行的检查

- `npm run check`：通过；7 files、46 tests。
- `npm run build`：通过；reader engine 与主应用均生成 production bundle。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：248 passed、0 failed、1 ignored。
- `npm run tauri build`：通过；生成 7.14 MiB `Qurner.app`。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现 `BLOCKER` 或 `MAJOR` 问题。 | — |

**AC 结果：**

- FR-010–FR-019：通过命令输入/输出、批量结果和逐项错误 schema 验证。
- FR-030、FR-037–FR-045：通过真实 nullable `BookSummary` 载荷及查询输入验证。
- FR-057–FR-058：通过软移除命令的成功与非法响应验证。
- AC-010、AC-011、AC-031、AC-032：当前任务提供的数据契约全部成立。
- NFR-004：`listBooks` 单次 invoke，不增加记录处理或 N+1。
- NFR-012：合法 AppError 保留，非法错误和响应统一映射为 `E_INTERNAL`。

**返工或后续任务：**

- 无。
- 真实 Tauri WebView 的默认 transport 和真实导入事件端到端验证按计划由 M4D
  集成冒烟覆盖，不阻塞本任务。
