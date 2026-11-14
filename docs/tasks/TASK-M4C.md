# TASK-M4C：书库网格、列表、搜索排序与软移除

**状态：** accepted  
**里程碑：** M4  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** TASK-M4A、TASK-M4B accepted

## 1. 目标

Qurner 主界面通过 `listBooks` 展示真实 active 书库，用户可以在网格和列表之间切换，
按书名/作者搜索，按书名/作者/最近导入时间排序，并经过二次确认软移除书籍；空、加载、
刷新失败和无匹配状态都有清楚反馈。

## 2. 范围

### 必须完成

- 使用 TanStack Query 调用 `commands.listBooks`，查询参数包含搜索、排序和方向。
- 实现默认网格视图、列表视图和无刷新切换。
- 网格和列表均展示书名、作者、封面或占位封面。
- 搜索书名与作者，采用后端 Unicode 包含匹配；输入完成后再查询，避免逐键重复请求。
- 支持书名、作者、最近导入三种排序及升降序切换。
- 软移除二次确认；成功后立即从当前列表移除并重新验证书库 query。
- 初始加载、空书库、无搜索结果、初次加载失败、刷新失败和操作失败状态。
- 导入完成后通过既有 `queryKeys.books` 前缀自动刷新书库。
- 仅键盘操作搜索、排序、视图切换、软移除确认和关闭弹层。

### 不属于本任务

- 1,000 本书虚拟化、冷启动/搜索性能总验收和真实 `Qurner.app` 冒烟；由 M4D 负责。
- 生成封面缓存、注册 `cover://` 协议或引入图像解码依赖；由 M4D 集成阶段负责。
- 记住上次视图和排序（FR-034、FR-046，P1）。
- 收藏夹筛选、最近阅读排序、书籍详情、书籍打开和阅读器页面。
- 拖放导入、导入取消、数据库/schema、Rust command、事件 DTO 或安全策略修改。
- 新增任何第三方依赖。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收 |
|---|---|---|
| FR | FR-030–FR-033, FR-035 | active 书库、网格/列表、切换，且两种视图显示书名、作者、封面 |
| FR | FR-037–FR-039 | 书名/作者搜索，匹配语义由后端 Unicode 包含查询保证 |
| FR | FR-042–FR-045 | 三种排序和升降序透传后端，不改变稳定排序语义 |
| FR | FR-057–FR-058 | 通过确认调用 `softRemoveBook`；不删除原文件和个人数据 |
| AC | AC-030, AC-031, AC-032 | 书库视图、搜索排序和软移除 UI 可观察行为成立 |
| NFR | NFR-004 | 搜索结果直接使用单次 `listBooks`，不做前端全量再过滤或 N+1 |
| NFR | NFR-012 | 加载、无匹配、刷新和移除失败显示可理解原因与下一步 |

FR-034、FR-036、FR-046 属于 P1/P2，本任务不实现。封面缓存与协议属于 M4D，本任务只负责
消费合法 `coverUrl` 并在缺失/无效时显示占位。

## 4. 设计与行为约束

### 4.1 Query 与参数

集中 query key，保持 `["books"]` 为可失效前缀：

```ts
queryKeys.books = ["books"] as const
queryKeys.bookList(input) = [
  ...queryKeys.books,
  "list",
  input.sort,
  input.direction,
  input.query ?? "",
] as const
```

- 默认搜索为空、`sort="importedAt"`、`direction="desc"`、网格视图。
- 搜索输入 trim 后为空时发送 `query: undefined`；非空时发送 trim 后的字符串。
- 搜索停止约 150 ms 后更新 query 参数；输入立即反映在控件中。
- `useQuery` 使用对应 `bookList` key，queryFn 只调用一次
  `commands.listBooks(input)`。
- 使用 `placeholderData: keepPreviousData`，参数变化或后台刷新时保留上次结果，避免列表闪烁。
- 不在前端重新搜索、排序或拼接 1,000 条记录；数组顺序原样渲染。
- 视图切换不改变 query 输入，不触发 `listBooks`。
- Query 默认仍为 `retry=false`、`refetchOnWindowFocus=false`。
- `ImportFlowProvider` 完成导入时继续失效 `queryKeys.books` 前缀。

### 4.2 可注入客户端与 App 集成

- 书库查询和软移除必须通过可注入 command client 测试，组件测试不得调用真实 Tauri。
- `App` 默认使用真实 `commands`，测试可注入完整 fake client；不得用 `vi.mock` 依赖真实
  IPC 才能通过基础渲染。
- 初始无搜索且书库为空时只显示导入入口；只要已有结果或搜索条件非空，搜索、排序和视图
  控件必须继续显示，确保无匹配状态可以清除搜索。
- 保留 M4B 的导入按钮、进度和结果汇总；导入完成后书库 query 自动刷新。

### 4.3 搜索、排序与视图控件

- 搜索输入 accessible name 为“搜索书名或作者”，支持中文、英文和特殊字符。
- 有搜索文字时提供“清除搜索”按钮；清除后恢复未搜索 query。
- 排序下拉提供“书名”“作者”“最近导入”。
- 升降序使用独立按钮或等价控件，状态文字和 accessible name 明确为“升序/降序”。
- 网格/列表使用两个可键盘操作的 toggle button，带 `aria-pressed` 或等价可访问状态。
- 控件变化后焦点不丢失；搜索请求进行中不得清空当前控件值。

### 4.4 网格与列表

- 两种视图都使用语义化列表结构；每本书由书名、作者、封面区域和移除操作组成。
- 网格卡片限制封面比例；列表行保持同样字段，长书名/作者允许换行或受控省略，不得溢出。
- 书名和作者来自 `BookSummary`，前端不自行替换成文件名或“未知作者”；后端已负责回退。
- 封面规则：
  - `coverUrl=null`：显示占位封面。
  - `coverUrl` 只接受 `cover://` URL 时才设置 `<img src>`；其他 scheme 一律按无效处理，
    不发请求。
  - 图片加载失败 `onError` 后切换为占位封面。
  - 占位封面不依赖外部网络或新增资源库。
- 至少提供一张有效 cover URL、null、非法 scheme 和加载失败的前端测试。
- 本任务不伪造后端封面数据；当前实际书库可能全部显示占位，真实缓存由 M4D 补齐。

### 4.5 页面状态

- 首次加载：显示“正在加载书库…”和可识别 loading 状态，不先闪现空书库。
- 无 active 书籍：显示“书库还是空的”、导入按钮和导入说明。
- 搜索无结果：显示“没有匹配的书籍”、当前搜索摘要和“清除搜索”入口；不要误报空书库。
- 首次加载失败：显示“无法加载书库”和固定中文说明、“重试”按钮；不得显示底层 message、
  stack 或路径。
- 已有数据刷新失败：保留上次列表，并在顶部显示非阻断错误条和“重试”。
- 请求参数快速变化时，只允许最新参数结果成为最终显示；旧请求不得覆盖新请求。
- 错误不写入 console 与 query cache，不展示完整本地路径。

### 4.6 软移除

- 每本书提供 accessible name 为“从书库移除《书名》”的按钮。
- 点击后显示 modal/dialog，标题“从书库移除书籍？”，明确说明：
  - 原始文件不会被删除或修改；
  - 阅读进度和书签会保留；
  - 再次导入同一本书可以恢复。
- 确认按钮调用 `commands.softRemoveBook(book.id)`，只传当前书籍 ID。
- 取消、Escape 或点击关闭不得调用命令；保存中禁用确认/取消/关闭，避免重复提交。
- 打开时焦点移到取消按钮，关闭后焦点回到原移除按钮；不要求完整焦点陷阱。
- 成功后关闭弹层、显示非阻断“已从书库移除《书名》”状态，并通过
  `setQueriesData`/失效使当前书籍立即消失。
- `E_BOOK_NOT_FOUND` 显示“该书已不在书库，正在刷新列表。”并触发刷新。
- `E_STORAGE` 显示“无法更新书库，请确认磁盘空间后重试。”
- 其他错误显示“无法从书库移除该书，请重试。”；弹层保留当前书名，不显示底层 message。
- 失败后允许重试或取消；不得乐观从列表永久删除。

### 4.7 可访问性

- 页面只有一个明确书库区域标题；搜索、排序、视图切换和移除操作都有 accessible name。
- 所有功能均可用键盘完成，焦点样式可见。
- 状态消息使用 `role="status"`，错误使用 `role="alert"`。
- modal/dialog 有 `aria-modal`、标题关联和明确关闭/取消按钮。
- 封面 `<img>` 具有与书名关联的 alt；占位封面可标记为装饰，避免重复朗读。

## 5. 文件权限

### 允许修改

- `src/features/library/`（书库组件、hook、状态、封面占位和测试）
- `src/query/query-client.ts`、`src/query/query-client.test.tsx`
- `src/app/App.tsx`、`src/app/App.test.tsx`
- `src/features/import/ImportFlowProvider.tsx` 及测试，仅限保持 query key 前缀失效兼容
- `src/styles.css`
- 任务专用测试支持文件
- 本任务状态和第 10 节开发报告

### 禁止修改

- `package.json`、锁文件、任何依赖或 Tauri plugin
- `src-tauri/`、`crates/`、数据库 schema、Rust command、cover 协议或封面缓存
- `src/bridge/` 的公开 DTO、Zod schema、命令名或错误码
- `src/reader-engine/` 和 iframe 协议
- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、ADR 和其他任务
- CSP、capability、网络或本地文件访问策略

如实现必须修改上述禁止路径或新增依赖，标记 `blocked` 并报告，不得自行扩大范围。

## 6. 测试要求

### Query 与数据流

- 首次查询使用默认 `{ sort:"importedAt", direction:"desc" }`，没有 `query` 字段。
- 搜索 150 ms 防抖；连续输入只产生最终参数请求。
- 书名/作者/最近导入三种 sort 与 asc/desc 的精确参数。
- query key 包含查询、排序和方向，但不含视图模式。
- 网格/列表切换不触发 `listBooks`。
- 参数变化保留上一页数据；旧请求完成不影响新参数结果。
- 导入完成仍失效 `["books"]` 前缀。

### 视图与封面

- 默认网格；切换到列表后两种视图都显示同一书名、作者、封面/占位。
- 长中英文标题和作者不会导致 DOM 缺失；具体视觉换行由 CSS 覆盖。
- 合法 `cover://` URL 生成图片；null、非法 scheme 和 `onError` 显示占位。
- 封面失败不发网络请求，不泄漏 URL/路径。

### 状态与错误

- loading 不闪现空书库。
- 空书库显示导入入口。
- 非空书库显示控件。
- 搜索无结果与空书库文案不同，清除搜索恢复列表。
- 初次加载失败可重试且不显示底层错误。
- 有数据时刷新失败保留旧列表并显示非阻断错误。

### 软移除

- 每本书移除按钮有包含书名的 accessible name。
- 打开/取消/Escape 不调用 `softRemoveBook`。
- 确认只传当前 bookId；pending 时按钮禁用。
- 成功后关闭、状态提示、当前书籍消失并失效 `["books"]`。
- 失败保留书籍和弹层，映射 `E_BOOK_NOT_FOUND`、`E_STORAGE` 与兜底；可重试。
- 完整路径、底层 message 和 stack 不出现在 DOM/console。
- 焦点进入取消按钮并在关闭后恢复。

### 必跑命令

```bash
npm run check
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri build
```

## 7. 禁止事项

- 不实现虚拟列表、1,000 本性能报告或 M4D 冒烟。
- 不实现封面缓存、`cover://` 协议、图像解码或新增图片依赖。
- 不把 `listBooks` 改为前端全量过滤/排序，不增加 N+1。
- 不修改原始 EPUB、数据库记录结构或软移除语义。
- 不新增收藏夹、最近阅读状态、阅读器入口或路由。
- 不提交、推送、发布或访问外网。

## 8. DeepSeek 开发报告

> 仅在开始实现后填写。开始实现时将状态改为 `in_progress`。

**完成摘要：**

实现真实书库查询、网格/列表、150 ms 搜索防抖、三种排序与方向、封面占位与软移除
二次确认；不新增依赖、不修改 Rust/数据库，前端不做搜索或排序。

第 9 节复验返工修复：

1. **防抖测试可捕获缺陷**：防抖用例改为连续输入“中” → “中文” → “  中文  ”，
   每次输入后等待 80 ms 断言没有任何中间 `listBooks` 请求；计时结束后断言只产生
   一次请求，且参数精确为 `query: "中文"`。已做负向验证：临时移除防抖实现时该测试
   失败，恢复后通过。
2. **列表视图封面断言**：切换到列表视图后补充断言有效 `cover://` 图片仍存在、无封面
   书籍不渲染 `<img>`、占位封面数量正确。

- **Query key**：`queryKeys.bookList(input)` = `["books","list",sort,direction,query??""]`，
  保留 `["books"]` 失效前缀；`createQueryClient` 默认 retry=false、
  refetchOnWindowFocus=false。
- **可注入客户端**：`LibraryCommandClient` 接口（listBooks/softRemoveBook）默认包装
  `commands`；`App` 支持注入 fake library client 与 import 依赖，组件测试不触碰真实
  Tauri。
- **LibraryPage**：
  - 默认 `{ sort:"importedAt", direction:"desc" }`，无 `query` 字段；
  - 搜索输入 150 ms 防抖，trim 后空则发送 `query: undefined`，非空发送 trim 值；
  - 书名/作者/最近导入三种排序 + 升/降序 toggle；网格/列表 toggle 带 `aria-pressed`，
    视图切换不改变 query、不触发 listBooks；
  - `useQuery` 只调用一次 `client.listBooks(input)`，`placeholderData: keepPreviousData`
    保留上次数据；旧参数请求完成不覆盖新参数结果（TanStack key 隔离）；
  - 状态：首次加载“正在加载书库…”（不闪现空书库）、空书库导入入口、无匹配
    “没有匹配的书籍”+ 当前搜索 + 清除搜索、初次失败“无法加载书库”+ 重试、有数据刷新
    失败保留列表 + 非阻断错误条 + 重试；
  - 封面：仅 `cover://` 设置 `<img src>`，null/其他 scheme 显示占位且不发请求，
    `onError` 切换占位；图片 alt 与书名关联，占位标记装饰性；
  - 软移除：每本书按钮 aria-label “从书库移除《书名》”；dialog 标题“从书库移除书籍？”，
    说明原文件不删除、进度/书签保留、可恢复；取消/Escape/关闭不调用；确认只传当前
    id，pending 禁用确认/取消/关闭；成功关闭、`setQueriesData` 立即移除 + 失效
    `["books"]` + 状态提示；`E_BOOK_NOT_FOUND` 关闭并刷新、`E_STORAGE` 留在弹层、
    其他兜底；失败不乐观删除；焦点进入取消按钮、关闭后回原移除按钮。
- **App/空书库**：保留 M4B 导入按钮、进度与汇总；空书库只显示导入入口与说明，
  有数据或无匹配时显示搜索/排序/视图控件。
- **测试**：`library-page.test.tsx` 16 个（默认参数/加载、空书库、网格↔列表不请求、
  防抖 trim、排序方向精确参数、keepPreviousData、旧请求不覆盖、无匹配与清除、初次
  失败重试、刷新失败保留、封面四类、移除弹层焦点/取消/Escape、确认 pending/移除、
  三类错误映射与路径不泄漏）；`query-client.test.tsx` 2 个；App 测试 1 个。总计 Vitest
  77 tests。

**修改文件：**

- `src/query/query-client.ts`、`src/query/query-client.test.tsx`
- `src/features/library/LibraryPage.tsx`、`BookCover.tsx`、`RemoveBookDialog.tsx`、
  `library-client.ts`、`library-errors.ts`、`EmptyLibrary.tsx`、
  `library-page.test.tsx`
- `src/app/App.tsx`、`src/app/App.test.tsx`
- `src/styles.css`
- `docs/tasks/TASK-M4C.md`（状态与本第 8 节）

未修改 `package.json`/锁文件、`src-tauri/`、`crates/`、数据库 schema、Rust command、
`src/bridge/` DTO/schema/命令名/错误码、`src/reader-engine/`、iframe 协议、
`SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、ADR、CSP/capability；
未新增依赖；未创建 commit。

**关键设计选择：**

- 查询参数集中在 `BookListQueryInput`，query key 由 `queryKeys.bookList` 统一生成，
  视图模式不进入 key；失效前缀保持 `["books"]`，M4B 导入完成刷新无需改接口。
- 搜索防抖使用本地 `useDebouncedValue(150)`；输入值立即反映在受控控件中，不因请求
  进行中清空。
- 软移除成功后先 `setQueriesData` 移除当前书籍再失效，确保列表立即消失；失败绝不
  乐观删除。
- 封面只接受 `cover://`，其他 scheme 按无效处理，不设置 `src`，避免任何网络请求。
- 组件注入 `LibraryCommandClient`，默认使用真实 `commands`；测试全部使用 fake。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | `tsc -b`、Biome、Vitest 9 文件 77 tests 通过 |
| `npm run build` | 0 | 主应用 Vite 构建成功（JS 365.02 kB） |
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 252 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.33 MiB |

**AC 验证：**

- FR-030–FR-033、FR-035：网格/列表、切换、两视图书名/作者/封面或占位。
- FR-037–FR-039：书名/作者搜索由后端 Unicode 包含查询负责，前端只透传参数。
- FR-042–FR-045：三种排序与 asc/desc 精确透传；视图切换不改变参数。
- FR-057–FR-058：软移除经二次确认调用 `softRemoveBook`，不删除原文件与个人数据。
- AC-030/AC-031/AC-032：书库视图、搜索排序、软移除 UI 与状态可观察。
- NFR-004：单次 `listBooks`，数组顺序原样渲染，无前端过滤/排序或 N+1。
- NFR-012：加载、无匹配、刷新与移除失败均有稳定中文说明与重试/清除入口。

**未验证项：**

- 未在真实 Tauri WebView 中运行 `listBooks`/`softRemoveBook`；测试使用注入的 fake
  client，真实 IPC 由 M4B/M4D 覆盖。
- 封面 `cover://` 缓存与协议未实现（M4D）；当前实际书库可能全部为占位封面。
- 未验证 1,000 本书的网格性能与虚拟化（M4D）。
- 未运行 `npm run test:e2e`；`tests/e2e/` 不在本任务允许修改路径，且既有
  `empty-library.spec.ts` 与新增书库控件/按钮计数断言冲突，留待 M4D 更新。
- 焦点陷阱未实现（任务只要求进入取消按钮并在关闭后恢复）。
- 长标题视觉换行由 CSS 覆盖，未做像素级溢出验证。

**风险与待决问题：**

- 工具栏与无匹配状态各有一个“清除搜索”按钮，测试需按数量/顺序定位；后续可统一
  为一个入口。
- 搜索防抖依赖真实 150 ms 计时；慢设备上可能出现短暂旧结果（keepPreviousData）。
- `cover://` 图片在真实 WKWebView 中的加载失败回退依赖 `onError`；M4D 接入协议后需
  复核。
- 软移除后 `setQueriesData` 对多个缓存 key 的过滤只在当前缓存项上生效，随后失效
  会重新拉取；极端并发下可能短暂显示旧列表。

**依赖变更：**

- 无新增依赖；未修改 `package.json`/锁文件。

## 9. Codex 审查

**结论：** 通过  
**审查日期：** 2026-11-14  

### 终轮复核

- 防抖测试已改为连续输入“中” → “中文” → “  中文  ”；每次输入后 80 ms 断言调用数
  不变，最终只新增一次 `query: "中文"` 请求。删除防抖实现或把延时缩短到 80 ms 时测试
  会失败。
- 列表视图已补充有效 `cover://` 图片、无封面 `<img>` 缺席和占位数量断言。
- 真实 Query、搜索排序、旧请求隔离、四类页面状态、软移除缓存与错误映射均符合任务。
- 未发现 UI 行为、依赖、Rust、数据库、公开 DTO、CSP/capability 或范围越界问题。

### 独立执行的检查

- `npm run check`：通过；10 files、77 tests，stderr 无 act/Warning 输出。
- `npm run build`：通过；主应用 production bundle 365.02 kB。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：252 passed、0 failed、1 ignored。
- `npm run tauri build`：通过；`Qurner.app` 7.33 MiB。

开发报告第 8 节将 `npm run check` 的测试文件数写为 9；独立复跑实际为 10。此项是报告
计数偏差，不影响代码或测试结果，以本节实际结果为准。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现 `BLOCKER` 或 `MAJOR` 问题。 | — |

**AC 结果：**

- FR-030–FR-033、FR-035：通过；网格/列表、切换和两视图封面/占位成立。
- FR-037–FR-039、FR-042–FR-045：通过；连续输入、trim、三种排序和方向参数均有验证。
- FR-057–FR-058、AC-030–AC-032：通过；软移除 UI、错误和恢复边界成立。
- NFR-004：通过；单次后端查询，无前端全量再过滤/排序或 N+1。
- NFR-012：通过；加载、无匹配、刷新和移除失败均有稳定中文反馈。

**返工或后续任务：**

- 无。
- 封面缓存/`cover://`、1,000 本虚拟化与真实 Qurner.app/e2e 集成由 M4D 验收。
