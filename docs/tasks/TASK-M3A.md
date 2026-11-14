# TASK-M3A：阅读会话与 EPUB 资源协议

**状态：** accepted  
**里程碑：** M3  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** TASK-M2D accepted

## 1. 目标

证明 `epub://<session-token>/<resource-path>` 可以按会话安全读取原始 EPUB 中
manifest 声明的单个资源，不把整本书载入内存，也不能读取会话书籍以外的文件。

## 2. 范围

### 必须完成

- 在 `epub-core` 提供受大小限制的单项资源读取 API。
- 在 Tauri 建立进程内阅读会话注册表和不可推算的 128 位协议令牌。
- 注册 `epub` 自定义协议，支持 GET/HEAD，并只返回当前会话 manifest 内资源。
- 对 URL 百分号编码、不安全路径、未知会话、跨会话和缺失资源做稳定拒绝。
- 返回 manifest MIME、`nosniff` 和 `no-store` 响应头。
- 使用 100 MB EPUB 证明只读取/解压请求的小章节，不读取整包。

### 不属于本任务

- `openBook`、`closeBook` 等公开 Tauri command、数据库查询和阅读历史。
- CSP 响应头、HTML 清洗、nonce、阅读脚本注入和 iframe 消息桥。
- 分页、滚动、Locator、目录 UI、阅读器 UI 或个人阅读数据。
- `cover://` 协议、封面生成、M4/M5 功能。
- 会话超时、持久化、恢复或跨进程共享。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收要求 |
|---|---|---|
| FR | FR-001, FR-007 | 只服务合法解析书籍的 manifest 资源 |
| FR | FR-006 | 协议不成为远程资源或任意文件读取通道 |
| FR | FR-012, FR-130 | 原 EPUB 只读；正文图片等资源可按需取得 |
| NFR | NFR-006 | 100 MB EPUB 的资源请求不触发整包读取 |
| NFR | NFR-010 | 不复制、移动或改写原始 EPUB |
| NFR | NFR-011 | 不新增网络服务、网络插件或出站请求 |
| NFR | NFR-012 | 无效会话、路径和资源返回稳定状态，不泄漏内部细节 |
| AC | AC-002, AC-130 | 本地资源隔离和正文图片读取前提成立 |

## 4. 设计约束

### 4.1 会话

- 会话由内部服务创建，先接受规范化原始文件路径与已解析的 `ParsedBook`；本任务不接
  数据库，也不暴露公开 command。
- 每次创建生成独立 `sessionId` 和协议令牌。协议令牌使用至少 128 位随机值，以小写
  URL-safe 单段字符串表示，不得等于书籍 ID、指纹或文件路径。
- JSON 日志不得记录协议令牌、绝对源路径、正文或书名；允许记录不可逆内部标识、阶段、
  状态码和耗时。
- 注册表线程安全，可并发解析多个会话；关闭单个会话后其所有 URL 立即失效。
- `sessionId` 与协议令牌职责分离。M3B 消息桥使用两者，但本任务不实现消息协议。

### 4.2 路径解析

- `epub://<token>/<resource-path>` 的 host/first segment 是令牌，其余路径是 EPUB 内
  资源路径。
- 仅允许一次严格 UTF-8 百分号解码。非法 `%` 序列、解码失败和二次编码不得回退为原始
  字符串继续读取。
- 解码结果必须通过 `epub-core::ResourcePath` 校验，并与当前会话 `ParsedBook.manifest`
  的 `href` 精确匹配。查询参数和 fragment 不得改变资源身份。
- 禁止目录列表、尾随通配、`META-INF/`、未列入 manifest 的条目和任何宿主文件路径。
- `%2e%2e`、`%2F`、绝对路径、反斜杠、NUL 及点段规范化后越界的请求必须拒绝。

### 4.3 资源读取

- 在 `epub-core` 增加不依赖 Tauri 的资源读取 API，调用方提供合法
  `ResourcePath` 和单资源字节上限。
- 必须在分配正文缓冲前检查 ZIP 声明大小；解压读取时也使用硬上限，防止伪造大小导致
  超大分配。失败不得留下临时文件。
- 单资源上限为 200 MB。超过上限返回稳定领域错误，不在协议层 panic。
- 每次请求只定位并读取目标条目，不遍历全部条目，不缓存整本解压内容，也不把原始
  EPUB 复制到应用数据目录。
- API 不返回 ZIP 库、`std::io` 绝对路径或第三方错误文本给上层。

### 4.4 协议响应

- 支持 `GET` 和 `HEAD`；其他方法返回 405，并带 `Allow: GET, HEAD`。
- 成功响应 `Content-Type` 使用 manifest MIME；至少增加
  `X-Content-Type-Options: nosniff`、`Cache-Control: no-store`。
- 未知、已关闭或跨会话令牌统一 404，响应不说明书籍是否存在。
- 非法 URL、未列入 manifest、缺失或不安全路径统一 404。
- 超过单资源上限返回 413；归档损坏或读取失败返回不含内部细节的 500。
- 400/404/405/413/500 响应体使用固定短文本，不包含源路径、ZIP 条目列表、SQL、会话
  令牌或第三方错误类型。
- 协议处理不得联网、打开外部应用、启动 shell 或访问原始 EPUB 以外的文件。

### 4.5 集成边界

- 协议 handler 必须拆成可独立测试的纯函数/服务，Tauri 注册层只负责提取请求和返回响应。
- `src-tauri` 使用现有依赖完成；不得新增 Tauri plugin。
- 若严格百分号解码必须新增外部 crate，先报告阻塞，不得自行加入依赖。可优先使用现有
  Rust/Tauri/URL 能力；若确实无法避免，应提交最小依赖申请。
- 不修改 `tauri.conf.json` 的 CSP；CSP 与本任务响应头的完整组合由 M3B 完成。

## 5. 文件权限

### 允许修改

- `crates/epub-core/`（新增资源读取 API 及测试）
- `src-tauri/src/protocol/`、`src-tauri/src/session/`（新增）
- `src-tauri/src/state.rs`、`src-tauri/src/lib.rs`
- `src-tauri/tests/` 及本任务测试支持代码
- 根 `Cargo.toml`、`Cargo.lock`、`src-tauri/Cargo.toml`（仅使用已授权依赖）
- 本任务状态和第 9 节开发报告

### 禁止修改

- `src/`、前端依赖、Playwright 配置
- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`
- `crates/storage-core/`、`crates/library-core/`、`crates/reader-model/`
- 数据库 schema、Tauri capability、`tauri.conf.json` 和其他任务文件

## 6. 测试要求

### 6.1 正常路径

- 同一会话可读取 XHTML、CSS、PNG/JPEG、字体和 manifest 其他资源，MIME 与 manifest
  完全一致。
- 两个独立会话可并发读取同名路径，各自只得到自己书籍的字节。
- HEAD 返回与 GET 相同状态和头，但正文为空。
- 中文、空格和非 ASCII 文件名经正确百分号编码后可以读取，且路径保持原义。

### 6.2 错误路径

- 未知、关闭后的、格式错误和跨会话令牌返回统一 404。
- `..`、`%2e%2e`、编码斜杠、绝对路径、反斜杠、NUL、目录和 `META-INF/` 返回 404。
- 不存在、未列入 manifest、查询参数映射到不存在资源的请求返回 404。
- 损坏条目/CRC 错误返回 500；超过 200 MB 声明或实际解压上限返回 413。
- POST/PUT/DELETE 返回 405 且不能产生读取副作用。
- 所有错误响应不包含测试临时绝对路径、源书名、正文、token、`zip`/`io`/`rbook` 类型或
  SQL 文本。

### 6.3 大文件与安全证据

- 构造至少 100 MB 的有效 EPUB，其中待请求 XHTML 很小；通过计数 `Read + Seek` 或等价
  机制断言只读取请求条目所需字节，不解压无关大资源。
- 证明协议只接受请求路径，不能通过编码、绝对路径或同一会话其他书籍绕过 manifest
  白名单。
- 静态检查没有新增 HTTP server、网络插件、shell 权限或 `fs` 通用读取插件。

### 6.4 自动化测试

- `epub-core` 单元/集成测试覆盖大小上限、声明大小伪造、CRC/损坏和无效路径。
- Tauri 测试直接覆盖协议解析与响应纯函数，并至少通过 mock runtime 验证 `epub`
  协议已注册、未知请求为稳定 404。
- 测试夹具只能使用合成或明确授权的 EPUB，不得写入用户私人书籍或真实应用数据目录。

## 7. 必跑命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p epub-core
cargo test -p qurner --test m3_protocol
cargo test --workspace
npm run check
npm run build
npm run tauri build
```

## 8. 禁止事项

- 不提交、不推送、不创建 PR。
- 不安装新依赖、不访问外部网络、不修改用户原始电子书或个人数据。
- 不实现 M3B–M5 功能，不通过前端 UI 掩盖协议或安全缺陷。
- 不跳过、删除或弱化测试，不把未验证内容写成通过。

## 9. DeepSeek 开发报告

> 仅在开始实现后填写。开始实现时将状态改为 `in_progress`。

**完成摘要：**

实现 `epub://<session-token>/<resource-path>` 的会话化资源读取；本轮按第 10 节复验
修复异步执行、响应回退与令牌熵三项问题。

基础实现：

- `epub-core` 新增 `read_resource` / `read_resource_from_reader`：
  - 只按 `ResourcePath` 定位并读取单个 ZIP 条目，不遍历条目、不缓存整本内容、不复制
    原始 EPUB；
  - 分配正文前检查 ZIP 声明大小，解压读取使用硬上限（`take(max+1)`），上限
    `MAX_RESOURCE_BYTES = 200 MB`；
  - 返回 `ResourceReadError`（`NotFound` / `LimitExceeded` / `Corrupt`），不携带 ZIP、
    `std::io` 路径或第三方错误文本。
- `src-tauri/src/session` 新增线程安全会话注册表：
  - 每次创建生成独立 `sessionId`（UUID v4）与协议令牌；
  - 令牌为 128 位随机值（`/dev/urandom` 16 字节，32 位小写十六进制）；
  - 保存规范化源路径与 `ParsedBook`；`close(session_id)` 后其 URL 立即失效。
- `src-tauri/src/protocol` 新增纯函数协议处理与 Tauri 注册：
  - 支持 `GET`/`HEAD`；其他方法 405 + `Allow: GET, HEAD`；
  - 严格一次 UTF-8 百分号解码；非法 `%` 序列、二次编码、`%2F` 编码斜杠、
    `..`/`%2e%2e`、绝对值、反斜杠、NUL、目录、`META-INF/`、未列入 manifest、缺失
    资源统一 404；
  - 查询参数/fragment 在解码前截断；
  - 成功响应使用 manifest MIME，并带 `X-Content-Type-Options: nosniff`、
    `Cache-Control: no-store`；超限 413；损坏/CRC 500；错误体为固定短文本；
  - manifest href 在 rbook 中可能是百分号编码形式；协议对 href 严格解码一次后与请求
    资源路径比较，支持中文/空格资源名；
  - Tauri 注册层只负责请求/响应转换。
- `AppState` 增加共享会话注册表；`build_app` 在注册协议时与状态共享同一实例。

第 10 节复验后修复（本轮）：

1. **异步协议执行**：注册从 `register_uri_scheme_protocol` 改为
   `register_asynchronous_uri_scheme_protocol`；handler 提取 method/uri 后把
   `handle(...)` 与响应转换放入 `tauri::async_runtime::spawn_blocking`，由
   `UriSchemeResponder` 回传响应。ZIP 打开、解压与资源读取不再在 WebView 回调内执行。
2. **响应构造失败固定 500**：`to_http_response` 不再以空 200 兜底；任何 header/body
   构造失败返回固定 500 响应。新增 `is_valid_media_type` 校验 manifest MIME（非空、
   长度 ≤ 255、含 `/`、仅可见 ASCII 且无控制字符）；非法 MIME 在读取资源前返回固定
   500。
3. **令牌熵**：`/dev/urandom` 失败时的 fallback 改为完整两个 UUID v4 拼接（64 个十六
   进制字符，约 244 位随机），`is_token` 接受 32 或 64 位十六进制；主路径仍为 128 位
   随机 32 位十六进制。

**修改文件：**

- `crates/epub-core/src/resource.rs`（新增）
- `crates/epub-core/src/lib.rs`（导出）
- `crates/epub-core/tests/resource_read.rs`（新增）
- `src-tauri/src/session/mod.rs`（新增；本轮修改 fallback token 与单测）
- `src-tauri/src/protocol/mod.rs`（新增；本轮异步注册、MIME 校验、固定 500）
- `src-tauri/src/state.rs`（共享会话注册表）
- `src-tauri/src/lib.rs`（协议注册）
- `src-tauri/tests/m3_protocol.rs`（新增；本轮新增 3 个回归测试）
- `docs/tasks/TASK-M3A.md`（顶部状态与本第 9 节）

未修改 `src/`、前端依赖、CSP、`tauri.conf.json`、capability、数据库 schema、
`crates/storage-core/`、`crates/library-core/`、`crates/reader-model/` 或其他任务文件；
未创建 commit。

**关键设计选择：**

- 协议 handler 拆为 `protocol::handle(registry, method, uri) -> ProtocolResponse`
  纯函数，Tauri 注册层只负责异步调度与 `http::Response` 转换，便于直接测试。
- 异步 handler 使用 `spawn_blocking`，避免为每个请求创建线程；`UriSchemeResponder`
  为 `Send`，可安全移入阻塞任务。
- `ResourcePath` 负责路径安全；协议层额外拒绝编码斜杠（`%2F`）、`META-INF/` 与
  查询参数映射的缺失资源。
- rbook 的 manifest href 会对非 ASCII/空格做百分号编码；协议对 href 解码一次后与请求
  资源路径比较，ZIP 读取使用解码后的原始条目名，因此中文/空格资源可读取，同时请求
  本身仍只解码一次、不回退。
- 令牌主路径读取 `/dev/urandom`；fallback 使用完整两个 UUID v4（244 位随机），
  `is_token` 同时接受 32/64 位十六进制，不新增密码学依赖。
- 会话注册表用两个 `Mutex` 映射（token→session、sessionId→token）；关闭会话同时移除
  两者，保证旧 URL 立即 404。
- 100 MB 证据使用 `read_resource_from_reader` + 计数 `Read + Seek` 包装器，断言读取
  字节 < 1 MB。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p epub-core` | 0 | 129 passed（含 resource_read 7 个；1 ignored 公版书） |
| `cargo test -p qurner --test m3_protocol` | 0 | 13 passed（本轮新增 3 个） |
| `cargo test --workspace` | 0 | 228 passed，0 failed，1 ignored |
| `npm run check` | 0 | Biome 14 个文件无修复；Vitest 2/2 通过 |
| `npm run build` | 0 | Vite 构建成功（dist 223.70 kB JS） |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 6.98 MiB |

100 MB 证据（`epub-core/tests/resource_read.rs::large_epub_reads_only_requested_small_resource`）：
请求 `OEBPS/chapter1.xhtml` 时读取字节数 < 1 MB，远小于 100 MB 大资源。

**AC 验证：**

- FR-001 / FR-007：只服务合法解析书籍 manifest 中的条目；未列入 manifest 的资源 404。
- FR-006：只读原始 EPUB 内条目；拒绝绝对路径、`..`、编码斜杠、反斜杠、NUL、
  `META-INF/`，不联网、不启动 shell、不打开外部应用。
- FR-012 / FR-130：原始 EPUB 只读；XHTML/CSS/PNG/JPEG/字体按需读取，MIME 与 manifest
  一致；中文/空格文件名经正确百分号编码可读。
- NFR-006：100 MB EPUB 只读取目标小章节所需字节（< 1 MB）。
- NFR-010：不复制、移动或改写原始 EPUB。
- NFR-011：无新增网络插件/服务；静态检查 Cargo.toml 与源码无插件与网络调用。
- NFR-012：未知/关闭/格式错误令牌统一 404；错误体固定短文本，不含临时路径、书名、
  正文、token、`zip`/`io`/`rbook`/SQL 文本；非法 MIME 与响应构造失败均返回固定 500。
- AC-002 / AC-130：两会话并发同名路径各自得到自己书籍的字节；跨会话路径 404。
- 异步执行：handler 在 `spawn_blocking` 中完成读取，不阻塞 WebView 回调（mock 测试
  覆盖纯函数与注册构建；真实 WebView 端到端留 M3B）。
- HEAD 返回与 GET 相同状态/头、正文为空；POST/PUT/DELETE/PATCH 返回 405 + Allow。

**未验证项：**

- 未在真实 macOS WKWebView 中加载 `epub://` 资源；Tauri 2 的协议注册表为 sealed，
  mock runtime 无法构造 `UriSchemeResponder`，因此以“注册调用源码存在 + mock app 构建
  成功 + 纯函数 404”作为注册证据；异步 responder 的端到端时序未在真实 WebView 验证。
- 未测试真实 `tauri build` 产物在 WebView 中的端到端资源加载（M3B 将接入阅读页面）。
- 100 MB 计数证据在 debug 测试构建测量；release 字节读取模式相同，未单独复测。
- `/dev/urandom` 失败分支未在缺少该设备的平台触发；`fallback_token` 由单元测试直接
  验证长度、十六进制与唯一性。
- 未测试会话数量上限、长时间运行或大量并发会话的注册表压力。
- 未测试字体文件在 WebView 中的实际渲染，仅验证 MIME 与字节读取。
- 未实现 Range 请求、缓存或条件请求（任务未要求）。
- 未覆盖 ZIP 加密条目（加密 EPUB 在解析阶段已拒绝）与 ZIP64 条目读取。

**风险与待决问题：**

- `to_http_response` 失败时返回固定 500，但调用方无法得知是哪个响应字段导致失败；
  日志仅能记录纯函数阶段（当前未记录该分支）。
- MIME 校验较宽松（含 `/` 且可见 ASCII 即接受）；带参数或非标准子类型的 MIME 会原样
  作为 `Content-Type`，由 M3B 的 CSP/渲染层进一步约束。
- 每次请求都重新打开 ZIP 并解压目标条目；无资源缓存，频繁图片/字体请求有重复解压
  开销，M4 视性能需要再评估。
- `HEAD` 仍实际读取资源以判定 200/404/413/500，不能完全避免解压成本。
- 会话注册表无上限与超时；M3B 负责关闭与生命周期。
- `/dev/urandom` 在非 Unix 平台不可用，fallback 依赖 UUID v4（244 位随机）；当前目标
  平台 macOS。
- 协议未设置 CORS/CSP，完整响应头组合留 M3B。
- 异步 handler 依赖 `tauri::async_runtime` 已初始化；若运行时不可用，请求会得不到
  响应（生产 Tauri 启动路径保证已初始化）。

**依赖变更：**

- 无新增外部 crate、Tauri plugin 或网络依赖。
- `epub-core` 使用既有 `zip`；`src-tauri` 使用既有 `uuid`、`epub-core` 依赖。
- 根 `Cargo.toml`、`Cargo.lock`、`src-tauri/Cargo.toml` 未修改。

## 10. Codex 审查

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p epub-core`：129 passed，1 ignored。
- `cargo test -p qurner --test m3_protocol -- --nocapture`：13 passed。
- `cargo test --workspace`：228 passed，0 failed，1 ignored。
- `npm run check`：通过；Biome 14 个文件无修复，Vitest 2/2。
- `npm run build`：通过。
- `npm run tauri build`：通过；生成 6.98 MiB `Qurner.app`。

**本轮确认：**

- 未知、关闭、格式错误和跨会话令牌均为统一 404。
- 路径白名单、编码斜杠、`..`、反斜杠、NUL、`META-INF/` 和未声明资源均被拒绝。
- XHTML、CSS、PNG/JPEG、字体及非 ASCII 路径的 MIME 和字节读取正确。
- GET/HEAD、405 `Allow`、413 和损坏条目 500 的主要路径通过。
- 100 MB EPUB 的小章节读取量低于 1 MB，未整包读取或解压。
- 没有新增网络插件、shell/fs 权限或外部 crate。
- `epub` 协议使用异步 responder，并将 ZIP 与资源读取移入 `spawn_blocking`。
- 非法 MIME 与任何响应构造失败均返回固定 500，不再回退为空 200。
- fallback 令牌使用完整两个 UUID v4（64 hex，约 244 位随机），主路径仍为 128 位随机
  令牌。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现 `BLOCKER`、`MAJOR` 或影响当前 AC 的 `MINOR` 问题。 | — |

**AC 结果：**

- FR-001、FR-006、FR-007、FR-012、FR-130：通过。
- NFR-006、NFR-010：通过。
- NFR-011：通过静态网络/插件检查。
- NFR-012：通过；非法 MIME、路径和读取错误均返回稳定状态。
- 会话令牌：主路径 128 位；fallback 约 244 位，满足要求。

**后续任务：**

TASK-M3A 通过。真实 WKWebView 加载与时序验收由 M3B/M3D 承担。M3B 启动前需要用户
批准 Zod 依赖或确认替代方案。
