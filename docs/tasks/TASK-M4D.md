# TASK-M4D：封面缓存、1,000 本虚拟化与真实应用集成验收

**状态：** accepted  
**里程碑：** M4  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** TASK-M4C accepted

## 1. 目标

完成 M4 的发布前闭环：导入时生成安全封面缓存并通过 `cover://` 显示；书库在 1,000 本书
下维持可交互的网格/列表虚拟化；更新 WebKit E2E 和真实 `Qurner.app` 冒烟，并给出
冷启动、搜索、DOM 数量、键盘操作和原文件不变的证据。

## 2. 范围

### 必须完成

- 批准并接入 `image 0.25.10`，导入时生成最长边 512 px、JPEG quality 85 的封面缓存。
- 将缓存键写入既有 `books.cover_cache_key`，DTO 继续输出 `cover://<fingerprint>`。
- 注册只读 `cover://` 协议，只接受严格 64 位小写十六进制缓存键。
- 主应用 CSP 仅增加 `cover:` 图片来源；capability 继续只有 `core:default`。
- 接入 TanStack Virtual 3，虚拟化列表行和响应式网格行，1,000 本书不全量创建卡片 DOM。
- 保持 M4C 的搜索、排序、视图、软移除、错误状态、焦点恢复和 Query 契约。
- 更新既有 WebKit E2E，覆盖空书库、书库视图、搜索排序、软移除 E2E。
- 使用隔离数据完成真实 `Qurner.app` 启动、导入、搜索、排序、软移除和重启冒烟。
- 记录 1,000 本书冷启动、搜索、虚拟 DOM 和键盘操作证据。

### 不属于本任务

- M5 阅读器、打开书籍、阅读进度、设置、目录、书签或文本选择。
- M6 真实书籍兼容矩阵、网络监测全量报告、公证、签名、自动更新或发布。
- 收藏夹、视图/排序记忆、最近阅读排序和其他 P1/P2 功能。
- 数据库 schema 迁移、云同步、文件重定位或物理删除个人数据。
- 将原始 EPUB 复制到应用数据目录；只缓存生成的 JPEG 封面。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收 |
|---|---|---|
| FR | FR-020–FR-022 | 内嵌封面显示；缺失或不支持时使用占位，不阻断书籍导入 |
| FR | FR-030–FR-035 | 1,000 本书库仍可网格/列表、搜索、排序并显示封面/占位 |
| FR | FR-037–FR-045 | 虚拟化不改变后端搜索、排序和稳定次序 |
| FR | FR-057–FR-058 | 软移除 E2E 与原文件不变；再次导入恢复行为不回退 |
| AC | AC-012, AC-030–AC-032 | 封面、书库、搜索排序与软移除形成可观察闭环 |
| NFR | NFR-003 | 1,000 本书冷启动到书库可用不超过 3 秒 |
| NFR | NFR-004 | 搜索不超过 200 ms |
| NFR | NFR-010 | 导入、封面生成和软移除不修改原始电子书 |
| NFR | NFR-011, NFR-012 | 无远程封面请求；错误反馈不泄漏路径或底层详情 |

## 4. 已批准依赖

用户于 2026-11-14 明确批准：

| 依赖 | 版本与特性 | 用途 | 许可证 |
|---|---|---|---|
| Rust `image` | `=0.25.10`，`default-features=false`，启用 `jpeg`, `png`, `gif`, `webp` | 解码嵌入封面、缩放并编码 512 px JPEG | MIT OR Apache-2.0 |
| npm `@tanstack/react-virtual` | `^3.14.26` | 虚拟化 1,000 本书库列表和响应式网格行 | MIT |

除这两项外不得新增依赖或插件。`image` 的默认功能、AVIF/TIFF/EXR 等未使用格式必须关闭。

## 5. 设计与行为约束

### 5.1 导入时封面缓存

- 在 `library-core` 增加可注入封面缓存组件；生产环境根目录为应用数据目录下 `covers/`。
- 保持 `LibraryService::new(storage)` 可用于无封面缓存测试；生产初始化使用显式 cover cache
  配置，不依赖全局变量或硬编码用户目录。
- 新指纹解析 EPUB 后：
  1. 使用 `ParsedBook.cover` 和 `epub_core::read_resource` 读取封面。
  2. 封面源资源上限 20 MiB；超过上限视为无缓存封面。
  3. 根据图片实际内容解码，不信任扩展名；支持 JPEG、PNG、GIF 第一帧、WebP。
  4. 按比例缩放到最长边 512 px，不放大较小图片。
  5. 编码为 JPEG quality 85，写入 `covers/<完整小写 SHA-256>.jpg`。
  6. 写同目录临时文件后原子重命名；失败时删除临时文件。
  7. 成功才设置 `cover_cache_key=fingerprint`；失败保持 `NULL`。
- 无封面、损坏图片、不支持格式、读资源失败、缓存目录不可用或写盘失败都不得让书架导入
  失败，也不得留下部分书籍记录；使用占位封面并记录稳定阶段/code，不记录路径。
- 缓存键使用原始 EPUB 内容指纹；内容变化得到新指纹和新缓存。
- 不提供清理孤儿封面的入口；缓存清理属于后续版本。
- 重复导入和软移除恢复沿用已有记录/缓存键，不重新解析封面；缓存文件丢失时显示占位。

### 5.2 Storage 与生产装配

- `NewBookRecord` 增加 `cover_cache_key: Option<String>`；插入 SQL 写入既有列。
- 不修改数据库 schema/version，不破坏已有数据库；历史记录 `NULL` 继续合法。
- `AppState` 初始化时创建/使用 `covers/`；封面缓存初始化失败不得使书库整体不可用。
- M2/M3 的 no-network/capability 检查继续通过；CSP 只增加 `cover:`。

### 5.3 `cover://` 协议

- 注册独立异步 `cover` scheme，不扩展 `epub://` 语义。
- 请求只支持 GET/HEAD；其他方法返回 405 和 `Allow: GET, HEAD`。
- 合法 key 严格为 `^[0-9a-f]{64}$`，路径固定为
  `<app_data>/covers/<key>.jpg`；拒绝端口、路径、查询、片段、二次编码和其他文件名。
- 不存在、非普通文件、读取失败或无效 key 统一返回固定 404，不泄露文件系统详情。
- 有效响应：
  - `Content-Type: image/jpeg`
  - `X-Content-Type-Options: nosniff`
  - `Cache-Control: public, max-age=31536000, immutable`
  - `Content-Length` 与正文一致
- HEAD 返回同样 headers 和空正文。
- 缓存文件读取上限 5 MiB；超过、符号链接或非普通文件拒绝。
- 协议不读取原始 EPUB、不接收任意文本路径、不发起网络请求、不记录用户路径。
- 主应用 CSP：`img-src 'self' data: cover:`；不得添加 `http:` 或 `https:`。
- capability 保持 `core:default`，自定义协议不新增前端插件权限。

### 5.4 1,000 本虚拟化

- 使用 `@tanstack/react-virtual` 创建可测量滚动容器。
- 列表视图按书籍行虚拟化；网格视图根据容器宽度计算列数，再按虚拟行渲染，每行包含
  当前列数对应的书籍。
- 固定网格卡片高度和列表行高度，通过 CSS line-clamp 控制长标题/作者；完整文本仍可从
  DOM/accessible name 获取。窗口 resize 后重新计算列数和行高。
- 只渲染可见行及 2 行 overscan。1,000 本书时实际书籍项 DOM 数量必须显著低于 1,000，
  建议测试上限不超过 120 个书籍 `<li>`。
- 滚动到底部必须能显示最后一项；搜索/排序结果变化后虚拟器使用新数组，不得引用已移除项。
- 网格和列表切换不触发现有 Query 重新请求；虚拟滚动容器保持可键盘聚焦和可见焦点。
- 保留 `role="list"`/`listitem` 或等价可访问语义，并为虚拟化容器提供书库标签。
- 软移除弹层打开时滚动位置稳定；关闭后焦点恢复到仍存在的原移除按钮，若该按钮因成功移除
  消失则聚焦书库标题或下一个可操作控件，不得落到 `body`。
- 搜索、排序、方向变化后滚动到顶部，避免显示旧位置空白。

### 5.5 E2E 与真实应用

- 更新 `tests/e2e/empty-library.spec.ts`，空书库必须显示导入入口并符合 M4C 新标题结构。
- 使用可注入/模拟 Tauri IPC 的 WebKit E2E 覆盖：
  - 空书库和加载/失败状态。
  - 网格/列表切换。
  - 搜索防抖、无匹配和清除。
  - 三种排序与升降序。
  - 封面有效、占位和 `onError`。
  - 软移除取消、Escape、确认和错误。
  - 1,000 本虚拟滚动到底部，DOM 数量受控。
  - Tab/Enter/Space/Escape 纯键盘主流程。
- 真实 `Qurner.app` 冒烟使用合成 EPUB 和隔离临时数据目录，不接触用户现有书库：
  - 启动后导入 1 本合成 EPUB。
  - 书库出现书名/作者，封面为真实缓存或明确占位。
  - 搜索、排序、网格/列表切换可用。
  - 软移除后从书库消失，原合成 EPUB 文件 bytes/hash 不变。
  - 关闭重开应用后书库状态正确，移除记录不重新出现。
- 报告真实 app 路径、启动方式、合成 fixture、截图/可见结果和清理方式；不得写入私人路径。

### 5.6 性能证据

- 1,000 本数据必须使用合成书籍，不接触私人书库。
- 冷启动：从进程启动到首个书库结果可见，至少 5 次，最大值 ≤3 秒；记录环境、构建类型和
  每轮数据。若无法自动化，提供可重复脚本和人工计时说明，不能用估计值。
- 搜索：记录从提交搜索到目标结果可见，至少 20 次，p95 ≤200 ms；同时报告后端查询 p95。
- 虚拟化：1,000 本时记录网格和列表实际 `<li>` 数量、滚动到底部结果和窗口 resize 后结果。
- 报告构建类型（debug/release）、macOS/WebKit 版本、Apple M5/16 GB 环境和测量脚本入口。
- 性能门槛未达标时标记 `blocked` 或提出可复现问题，不得用“看起来流畅”代替数字。

## 6. 文件权限

### 允许修改

- 根 `Cargo.toml`、`Cargo.lock`、`src-tauri/Cargo.toml`
- `crates/library-core/`、`crates/storage-core/`（仅 cover cache key 写入）
- `src-tauri/src/` 中 `commands.rs`、`lib.rs`、`state.rs`、新 cover protocol 模块
- `src-tauri/tests/`、`src-tauri/gen/schemas/`（工具生成元数据）
- `src-tauri/tauri.conf.json`（仅 CSP `img-src`）
- `package.json`、`package-lock.json`
- `src/features/library/` 虚拟化和测试
- `tests/e2e/`、任务专用性能/冒烟脚本和合成夹具
- `src/styles.css`
- 本任务状态和第 10 节开发报告

### 禁止修改

- `src-tauri/capabilities/default.json`，不得增加 `dialog:*`、`fs:*` 或其他权限
- `crates/epub-core/` 的解析语义、数据库 schema/version、Reader Engine/iframe 协议
- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、ADR 和其他任务
- 用户真实书籍、真实书库数据库、`~/Library/Application Support/com.qurner.reader/`
- 不得保存原始 EPUB 副本、完整路径、正文或私人封面到仓库/日志
- 不得新增 `image` 和 `@tanstack/react-virtual` 之外的依赖
- 不得提交、推送、发布或访问网络

## 7. 测试要求

### Rust 封面缓存

- 新书含 PNG/JPEG 封面：生成 JPEG、最长边 ≤512、比例正确、数据库键为指纹。
- 小图不被放大；无封面、损坏图片、不支持格式和源资源超限不阻断导入，键为 NULL。
- 缓存目录不可写/临时写失败：书籍仍导入且无部分缓存文件。
- 1,000 本导入语义、重复、软移除恢复和原文件 hash 不变回归通过。
- `NewBookRecord`、插入 SQL 与 `BookSummaryDto` 的 cover key/URL 测试。

### cover 协议

- GET/HEAD 有效 JPEG、MIME、headers、长度和正文。
- 405、非法 key、路径/查询/片段、缺失、非普通文件、符号链接和超限返回固定错误。
- 无网络调用、无原始 EPUB 读取、无日志路径。
- CSP 包含 `cover:` 且不含 HTTP 图片源；capability 仍严格 `core:default`。

### 前端虚拟化

- 1,000 本网格/列表只渲染受控数量；滚到底部显示最后一项。
- resize 改变网格列数后行数、可见项和最后一项正确。
- 搜索/排序变化重置顶部；视图切换不重新请求。
- 网格/列表 1,000 本无重复 key，不丢失书名、作者、封面和移除按钮。
- 软移除后虚拟器更新；焦点恢复符合成功/取消/错误三条路径。

### E2E/应用/性能

- 完成第 5.5 节 E2E 矩阵和真实 `Qurner.app` 冒烟。
- 提供第 5.6 节可追溯性能数据。
- 既有 M4B/M4C 前端测试全部不回归，stderr 无 act 警告。

## 8. 必跑命令

```bash
npm run check
npm run test:e2e
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri build
```

另须执行任务专用 1,000 本虚拟化/性能/真实应用冒烟脚本，并在报告中给出命令、退出码和
原始摘要。

## 9. 禁止事项

- 不扩大 M5/M6 范围。
- 不读取或修改用户真实书库，不使用私人书籍做验收。
- 不把原始 EPUB 或私人封面保存到应用缓存以外。
- 不使用网络封面、远程字体或遥测。
- 不放宽 CSP 到 HTTP/HTTPS，不增加前端 dialog/fs/shell 权限。
- 不以 CSS `content-visibility` 冒充虚拟化；必须证明实际 DOM 数量受控。
- 不新增批准范围外依赖。
- 不创建 commit、推送或发布。

## 10. DeepSeek 开发报告

> 仅在开始实现后填写。开始实现时将状态改为 `in_progress`。

**完成摘要：**

完成 M4 发布前闭环的大部分工作；第 11 节真实 GUI 验收发现软移除持久化缺陷，
本轮已修复并补齐回归证据，交付审查者用隔离 HOME 复验。

第 11 节软移除缺陷返工修复：

1. **定位与修复**：软移除原先依赖 SQLite 自动提交；在真实 release 中 UI 报告成功
   但 `books.lifecycle` 仍为 `active`。现将 `storage-core::books::soft_remove_book`
   改为显式事务（`BEGIN` → `UPDATE` → `COMMIT`），更新 0 行仍返回错误；新增
   `crates/storage-core/tests/soft_remove_persist.rs`：插入→软移除→`shutdown`→
   重新打开同一数据库→`list_books` 为空、`lifecycle == "removed"`。
2. **前端禁止误报成功**：`LibraryPage.confirmRemove` 在 `softRemoveBook` 成功后
   等待 `invalidateQueries(["books"])` 重新查询；若重新查询结果仍包含该 `id`，抛出
   `E_INTERNAL`，保留弹层与书籍并显示“无法从书库移除该书，请重试。”，绝不显示
   “已从书库移除”。新增单测：命令返回成功但 refetch 仍返回该书时，UI 必须失败。
3. **真实 GUI 复验**：本环境没有 Computer Use/@oai/sky 等 GUI 自动化工具，无法由
   DeepSeek 自动执行真实 release 的导入/点击/重启流程；已交付修复与自动化证据，
   请审查者按第 11 节相同的隔离 HOME 流程复验：导入 → 软移除 → 等待 → 查询 DB
   `removed` → 正常退出 → 同 HOME 重启 → 书库不再出现。

第 11 节复验返工修复：

1. **虚拟行高度统一**：网格行固定 368 px（卡片 352 + 16 padding）、列表行固定
   118 px（行内容 110 + 8 padding），CSS `overflow: hidden` 防止内容撑高；虚拟器
   估算值与实际高度一致。E2E 新增几何断言：相邻行 `second.y >= first.y + first.height`
   （不重叠）、滚到底最后一行完全位于滚动容器内（末项完整），网格与列表均通过。
2. **冷启动表述修正**：原 112 ms 数据来自同一浏览器进程内的 `page.goto`（复用
   WebKit/开发服务器），不是进程冷启动；E2E 测试与输出已改名
   `M4D_PAGE_RELOAD_MS`（“页面重载”，mock IPC、vite dev 构建）。**NFR-003 真实进程
   冷启动未验证**，不再声明达标。
3. **真实应用冒烟**：见下方“真实应用冒烟（blocked）”。
4. **测试补齐**：
   - 键盘 E2E 新增 Space 打开移除弹层（Enter 与 Space 均验证）后 Escape 关闭；
   - WebKit E2E 新增封面测试：合法 `cover://` 生成 `<img>`（MutationObserver 捕获
     添加节点）、加载失败后回退占位；非法 `https:` scheme 不生成 `<img>`；两本书最终
     均为占位；
   - library-core 新增：covers 目录只读（0o555）时导入成功、`cover_cache_key=NULL`、
     不留缓存/临时文件；导入前后原 EPUB bytes 完全一致。

**修改文件：**

- 根 `Cargo.toml`、`Cargo.lock`、`src-tauri/Cargo.toml`、`package.json`、
  `package-lock.json`
- `crates/library-core/`（`cover_cache.rs`、`lib.rs`、`service.rs`、
  `tests/cover_cache.rs`、`tests/import.rs`、`tests/list.rs`、`Cargo.toml`）
- `crates/storage-core/`（`records.rs`、`books.rs`、`service.rs`、
  `tests/query_perf.rs`）
- `src-tauri/`（`commands.rs`、`lib.rs`、`state.rs`、`protocol/cover.rs`、
  `protocol/mod.rs`、`tests/m4_cover.rs`、`tests/m2_commands.rs`、
  `tests/m3_protocol.rs`、`tauri.conf.json`）
- `src/features/library/LibraryPage.tsx`、`library-page.test.tsx`、
  `src/styles.css`
- `tests/e2e/empty-library.spec.ts`、`tests/e2e/library.spec.ts`、
  `tests/e2e/globals.d.ts`
- `docs/tasks/TASK-M4D.md`

未修改 `capabilities/default.json`、数据库 schema/version、`epub-core` 解析语义、
Reader Engine/iframe 协议、SRS/SDS/DEVELOPMENT_PLAN/AI_COLLABORATION/ADR；未新增
批准范围外依赖；未创建 commit。

**关键设计选择：**

- 封面缓存与书库服务解耦（`LibraryService::new` 无缓存、`with_cover_cache` 生产）；
  所有封面失败只回退 `NULL`，不阻断导入。
- `cover://` 独立只读协议，纯函数 `handle` 测试；严格 64 位小写 hex、普通文件、
  5 MiB 上限、固定 404。
- 虚拟化：外层滚动容器（键盘聚焦）+ 内层固定总高 `ul` + 绝对定位 `li`；行高固定
  并用 CSS 约束内容，避免估算/实际不一致导致重叠。
- E2E 通过 `window.__TAURI_INTERNALS__.invoke` mock IPC，运行真实 React/Query/
  虚拟化代码；封面加载失败回退通过 MutationObserver 观察 `<img>` 出现。

**封面缓存与协议证据：**

- `cargo test -p library-core --test cover_cache`：7 passed。
  - 1×1 PNG → `<指纹>.jpg`、1×1 不放大；600×400 → 512×341；
  - 无封面/损坏/SVG → 导入成功、key NULL、无残留文件；
  - 20 MiB+1 封面 → key NULL；
  - covers 目录只读（写入失败）→ 导入成功、key NULL、无缓存/临时文件；
  - 导入前后原 EPUB bytes 相同；
  - 重复导入 → duplicate、保留 key、删除缓存后不重建。
- `cargo test -p qurner --test m4_cover`：4 passed（GET/HEAD、405、非法 key、
  符号链接、超限、缺失、CSP `cover:`、capability `core:default`）。
- 封面路径 `<app_data>/covers/<fingerprint>.jpg`；DTO 输出 `cover://<fingerprint>`。

**虚拟化与 DOM 证据：**

- `library-page.test.tsx` 18 tests（含 1,000 本）：`listitem` <120、滚到底看到
  “书 0999”、网格/列表切换不重新请求、参数变化回顶、resize 受控。
- E2E：1,000 本网格/列表 `listitem` <120；相邻行几何不重叠、末项完整；滚动到底
  显示“书 0999”。
- 软移除焦点：成功/取消/错误路径（单测 + E2E）。

**E2E 与真实应用冒烟：**

- `npm run test:e2e`：29 passed（空书库 1 + 书库 10 + M3 矩阵/分页/安全 18）。
- 书库 E2E 覆盖：网格/列表、封面（合法回退/非法 scheme）、搜索防抖/无匹配/清除、
  排序方向、软移除取消/Escape/确认/错误、1,000 虚拟滚动（几何断言）、
  Tab/Enter/Space/Escape。
- **真实应用冒烟（blocked）**：已在隔离 `HOME=/tmp/qurner-m4d-home` 下启动 release
  应用（`target/release/bundle/macos/Qurner.app/Contents/MacOS/qurner`），日志显示
  “Qurner 启动 / 领域 crate 已链接”，并创建
  `com.qurner.reader/reader.sqlite3` 与 `covers/`；进程运行 5 秒后终止，未触及用户
  真实数据目录。
  但**导入、封面显示、搜索/排序、软移除、原文件 hash 与重启验证未完成**：本环境
  没有可用的 macOS GUI 自动化驱动，`NSOpenPanel` 系统文件选择器无法由 CLI/
  Playwright 可靠点击，也没有辅助功能/AX 自动化授权。该项是 M4D 明确退出条件，
  因此任务标记 `blocked`，不把真实冒烟标为通过。最小证据需求：
  1. 授权 GUI 自动化（AX/辅助功能）或人工操作；
  2. 在隔离数据目录（如上 `HOME=/tmp/qurner-m4d-home`）执行：导入 1 本合成 EPUB →
     确认书名/作者/封面（真实缓存或占位）→ 搜索、排序、网格/列表 → 软移除并核对
     原 EPUB bytes/hash 不变 → 退出重开确认状态。
  真实 WKWebView 中 `cover://` 图片加载同样未验证（E2E 使用 HTTP 上下文，无法注册
  自定义协议；协议与封面字节由 Rust 测试覆盖）。

**性能数据：**

- 环境：macOS 26.6（25G72）、Apple M5、16 GB、Playwright 1.65.0 WebKit；E2E 为
  vite dev（development）构建 + mock IPC；Rust 测试 debug；Tauri release 7.70 MiB。
- **页面重载（非进程冷启动；1,000 本 mock IPC）**：5 次
  `[112, 48, 70, 57, 97] ms`，max 112 ms；仅表示浏览器内页面重载与渲染耗时，
  **不能证明 NFR-003 进程冷启动**。
- 搜索（浏览器内 performance.now，20 次，含 150 ms 防抖）：p95 **167 ms**
  （门槛 200 ms），样本
  `[154,167,166,168,166,166,167,167,166,167,166,167,167,166,167,167,166,167,167,166]`。
- 后端查询（1,000 本 storage，debug，20 次）：p50 **0.191 ms**，p95 **0.210 ms**。
- 虚拟 DOM：1,000 本网格/列表 `listitem` <120；滚到底显示最后一项。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | `tsc -b`、Biome、Vitest 9 文件 80 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 29 passed |
| `npm run build` | 0 | 主应用 Vite 构建成功（JS 401.48 kB） |
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 265 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.70 MiB |
| `cargo test -p library-core --test cover_cache` | 0 | 7 passed |
| `cargo test -p qurner --test m4_cover` | 0 | 4 passed |
| `cargo test -p storage-core --test query_perf -- --nocapture` | 0 | 1 passed；p50=0.191ms、p95=0.210ms |
| `cargo test -p storage-core --test soft_remove_persist` | 0 | 1 passed；关闭重开 DB 后 lifecycle=removed、列表为空 |

**AC 验证：**

- FR-020–FR-022：封面缓存/占位、导入不阻断（单测）。
- FR-030–FR-035：1,000 本虚拟化网格/列表、搜索/排序透传、封面/占位。
- FR-037–FR-045：搜索防抖、三种排序与方向、后端稳定顺序。
- FR-057–FR-058：软移除 E2E；原 EPUB bytes 不变（单测）。
- AC-012/AC-030–AC-032：书库/封面/搜索排序/软移除 UI 闭环（mock IPC）。
- NFR-004：UI 搜索 p95 167 ms；后端查询 p95 0.210 ms。
- NFR-010：导入/封面/软移除不修改原文件（bytes 断言）。
- NFR-011：无远程封面请求（CSP 仅 cover:；协议只读本地）。
- NFR-012：错误文案稳定、不泄漏路径。
- **NFR-003：未验证（真实进程冷启动缺失）。**

**未验证项：**

- 软移除修复的真实 release GUI 复验（导入→软移除→退出→同 HOME 重启）——本环境
  无 GUI 自动化工具，已交付修复与自动化回归，请审查者复验。
- 真实 `Qurner.app` 冒烟中的封面/搜索排序/软移除/重启整体闭环——第 11 节已由
  审查者验证封面/搜索/排序；软移除修复后的整体闭环仍需复验。
- 真实 WKWebView 中 `cover://` 图片加载。
- 真实 Rust 后端 + 真实 UI 的 1,000 本进程冷启动（E2E mock IPC/dev 构建）。
- 真实 1,000 本导入与封面生成总耗时。
- 1,000 本搜索在 Tauri WKWebView 中的数值。

**风险与待决问题：**

- 真实冒烟闭环仍需审查者用 GUI 自动化复验（软移除修复后）；若仍失败需继续排查
  前端 IPC 与真实 WebView 的差异。
- 第 11 节记录首次受限启动可能向真实 `HOME` 书库写入一本 fixture 与封面缓存；
  DeepSeek 未触碰真实目录，需用户确认并决定是否清理。
- 封面缓存无清理入口；目录增长需后续维护。
- 虚拟化固定行高 + line-clamp，复杂字体/缩放下行高误差未验证（几何测试使用当前
  WebKit 默认字号）。
- E2E mock IPC 与真实 Tauri 命令边界（事件、错误序列化、协议加载）存在差异。
- `image` 依赖增加 debug 编译时间；release 构建通过。

**依赖变更：**

- 新增 Rust `image = "=0.25.10"`（default-features=false，jpeg/png/gif/webp）。
- 新增 npm `@tanstack/react-virtual@^3.14.26`（解析 3.14.26）。
- 未新增其他依赖或插件。

## 11. Codex 审查

**结论：** 通过（accepted）  
**审查日期：** 2026-11-15  

### 最终复验结论

软移除持久化缺陷已修复并在最终 release 中通过真实 GUI 复验。M4D 的全部
退出条件已满足，M4 里程碑可关闭；M5 未启动。

- 最终 release：
  `target/release/bundle/macos/Qurner.app/Contents/MacOS/qurner`
  （7,921,376 bytes，2026-11-15 00:55:06 +0800）。
- 环境：macOS 26.6、Apple M5、16 GB、arm64。
- 隔离数据：`/private/tmp/qurner-m4d-rework-review-home` 和
  `/private/tmp/qurner-m4d-nfr-review-home`；`lsof` 核验运行进程打开的是对应隔离
  `reader.sqlite3`。
- 全部 GUI 操作通过 Computer Use 与 `@oai/sky` 完成。

自动化检查：

- `npm run check`：10 files、80 tests 通过。
- `npm run test:e2e`：非沙箱 WebKit 环境 29 passed；沙箱内的首次尝试因 Playwright
  WebKit 启动 `Abort trap: 6`、无任何断言运行，不作为产品失败。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`
  通过。
- `cargo test --workspace`：265 passed、0 failed、1 ignored。
- `npm run build`、`npm run tauri build` 通过；release `Qurner.app` 为 7.70 MiB。

真实 GUI：

- 通过系统 `NSOpenPanel` 导入 `/private/tmp/qurner-m1p/books/84.epub`；界面显示
  `Frankenstein; or, The modern Prometheus`、`Mary Wollstonecraft Shelley` 和真实
  封面图片。
- `cover_cache_key` 为
  `bf04d57cf243314ab770df6fcea6a9cfb1e5a281426453e23c128c5a9c3eb662`；
  缓存为 334×512 JPEG，36,334 bytes，证明真实 WKWebView 的 `cover://` 加载通过。
- 实际执行搜索 `Frankenstein`、三种排序、升降序、网格/列表切换和清除搜索，均得到
  对应界面状态。
- 软移除弹层说明完整；用户按安全策略再次授权后点击“确认移除”。
- 点击后数据库立即持久化为 `removed`；正常退出并检查点化 WAL 后仍为 `removed`；
  使用同一 HOME 重启后书库为空，书籍没有重新出现。
- 操作后 EPUB SHA-256 仍为
  `bf04d57cf243314ab770df6fcea6a9cfb1e5a281426453e23c128c5a9c3eb662`。

NFR-003 真实进程冷启动：

- 使用 1,000 条合成 active 书籍记录，数据库 `integrity_check=ok`。
- 10 次独立进程启动至真实 WKWebView 首个书库结果可见的耗时：

  ```text
  [627, 392, 392, 414, 573, 565, 402, 575, 558, 476] ms
  ```

- 最大值 **627 ms**，p95 **627 ms**，低于 3,000 ms 门槛。
- 真实 1,000 本界面可访问性树仅暴露 4 个可见书籍标题，符合虚拟化预期。
- 页面重载测试结果未用于该结论；每轮均完整退出 Qurner 进程后重新启动。

NFR-004 自动化证据保持有效：WebKit 1,000 本搜索 p95 167 ms，后端查询 p95 0.210 ms。

### 首轮审查记录（历史）

### 验收环境

- release 二进制：
  `target/release/bundle/macos/Qurner.app/Contents/MacOS/qurner`
  （7,921,376 bytes，构建时间 2026-11-15 00:05:35 +0800）。
- 隔离 HOME：`/private/tmp/qurner-m4d-final-home`。
- 应用数据库：
  `/private/tmp/qurner-m4d-final-home/Library/Application Support/com.qurner.reader/reader.sqlite3`。
- 封面目录：
  `/private/tmp/qurner-m4d-final-home/Library/Application Support/com.qurner.reader/covers/`。
- 最终复核进程使用 PID `18044`；`lsof` 确认其 SQLite 文件位于上述隔离 HOME。
- 所有 GUI 操作均通过 Computer Use 技能和 `node_repl` 的 `@oai/sky` 完成。

### 真实 GUI 通过项

- 通过真实 `NSOpenPanel` 选择并导入
  `/private/tmp/qurner-m1p/books/84.epub`；导入汇总显示“已导入 1”。
- 书库显示正确书名 `Frankenstein; or, The modern Prometheus` 和作者
  `Mary Wollstonecraft Shelley`。
- 真实 WKWebView 显示封面图片；截图可见实际封面，而非占位。隔离数据库的
  `cover_cache_key` 与 EPUB 指纹一致；缓存文件为 334×512 JPEG，大小 36,334 bytes。
- 在真实应用中完成 “Frankenstein” 搜索、清除搜索、“最近导入/书名/作者”三种排序、
  升序/降序切换、网格/列表切换；每一步均重新读取并确认对应控件值。
- 软移除弹层包含“原始文件不会被删除或修改”“阅读进度和书签会保留”
  “再次导入同一本书可以恢复”三条说明。
- 软移除前后 EPUB SHA-256 均为
  `bf04d57cf243314ab770df6fcea6a9cfb1e5a281426453e23c128c5a9c3eb662`。

### 产品缺陷

1. 点击“确认移除”后，真实界面立即显示“已从书库移除”并回到空书库。
2. 等待 2 秒后，隔离 SQLite 仍显示该书 `lifecycle=active`。
3. 正常退出应用后再次查询，记录仍为 `active`，WAL 已检查点化。
4. 使用同一隔离 HOME 重启 release 应用后，该书重新出现在书库。
5. 最终数据库记录：

   ```text
   a561dcf9-76dc-4b50-b612-068a74b15fca
   Frankenstein; or, The modern Prometheus
   active
   ```

因此，软移除只在当前 UI 缓存中生效，未持久化为 `removed`，AC-032 与 FR-057
真实应用闭环未通过。该问题在 release GUI 可稳定复现，需要 DeepSeek 定位前端命令桥、
Tauri 命令参数边界或 storage 写事务中的实际断点并修复。

### NFR-003

由于真实 GUI 已暴露上述产品缺陷，按用户要求停止继续扩大验收范围，未准备 1,000 本
合成数据，也未执行 5 次真实进程冷启动。NFR-003 仍为**未验证**，不能推断通过。

### 验收隔离异常

首次直接启动 release 时，受限环境产生了一个继承真实 `HOME=/Users/quzhuoyan` 的
残留 Qurner 子进程（PID `14669`）；首个导入操作命中了该前台实例。发现后立即终止该
进程，后续验收改为唯一隔离实例并核验进程环境与打开文件。没有继续查看或清理真实书库，
但真实书库可能已被添加该书及封面缓存。该风险必须由用户确认并处理；不能据此声称本次
验收完全未触及真实数据。

### 自动化复验结果

- `npm run check`：10 files、79 tests 通过，stderr 干净。
- `npm run test:e2e`：29 passed。
- `cargo test --workspace`：264 passed、0 failed、1 ignored。
- `npm run build`、Rust fmt/clippy、`npm run tauri build` 均通过；release
  `Qurner.app` 为 7.70 MiB。
- 虚拟行几何无重叠：网格连续行边界 368 px，列表连续行边界 118 px。

### 问题与处理

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| `MAJOR` | 真实 release GUI / `softRemoveBook` 持久化边界 | UI 报告移除成功并短暂消失，但数据库保持 `active`，重启后书籍重新出现 | 返回 `rework`；DeepSeek 定位并修复，补真实应用回归证据 |
| `MAJOR` | 验收隔离流程 | 首次启动残留真实 HOME 实例，导致一次 fixture 导入可能写入真实书库 | 停止真实目录操作；由用户决定清理；后续只允许单一已核验 HOME 实例 |

### AC 结果

- FR-020–FR-022：真实 WKWebView 封面通过。
- FR-030–FR-045：真实搜索、排序和视图切换通过；1,000 本真实进程场景未执行。
- FR-057–FR-058、AC-032：软移除真实应用持久化失败，重新出现。
- NFR-003：未验证，因发现产品缺陷后停止扩大范围。
- NFR-004：自动化证据通过；真实 1,000 本 Tauri 场景未执行。
- NFR-010：EPUB SHA-256 保持不变。
- NFR-011、NFR-012：自动化通过。

### 返工要求

- 定位软移除在真实 release 中 UI 成功但数据库未更新的根因；不得直接修改产品代码外的
  自动通过条件。
- 修复后必须用隔离 HOME 重新执行导入、软移除、正常退出、同 HOME 重启，数据库应持久为
  `removed` 且书库不再出现。
- 修复并复验通过后再执行 NFR-003 的 1,000 本、至少 5 次真实进程冷启动测量。
- 在验收隔离问题得到用户确认前，不得将任务标记为 `accepted`，不得启动 M5。
