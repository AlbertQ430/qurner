# TASK-M3D：M3 技术门槛总验收

**状态：** accepted  
**里程碑：** M3  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** TASK-M3A、TASK-M3B、TASK-M3C accepted

## 1. 目标

用六类有效 EPUB、WebKit 自动化、安全攻击、Locator 往返、100 MB 按需读取和翻页性能
共同判定当前 `epub://` + iframe + CSS 多栏方案是否达到 M3 技术门槛，并形成可供 Codex
和用户决定 ADR 的真实报告。

## 2. 范围

### 必须完成

- 六类有效 EPUB 的解析、资源协议和 WebKit 阅读矩阵。
- 每类书至少验证一个 spine 文档、CSS/图片/字体资源及阅读引擎初始化。
- 复验脚本/事件/远程资源/外部导航隔离。
- 复验分页末页、无空白尾页、resize、模式切换、目录映射和 Locator。
- 1,000 本无关；本任务专注 100 MB EPUB 按需读取。
- 测量布局内 turn-page p95，门槛 150 ms。
- 编写 `docs/acceptance/M3_SPIKE_REPORT.md`，列出测试页面、夹具、环境、结果、失败、
  已知限制和浏览器差异。

### 不属于本任务

- 正式书库/阅读器 UI、文件选择器、openBook/session command 或进度持久化。
- M4/M5 功能开发。
- 最终 ADR 决策。DeepSeek 只提供报告和建议；ADR 由 Codex 在审查后编写，并由用户确认
  渲染路线。
- 在未获授权时引入浏览器驱动、HTTP server、阅读引擎或其他外部依赖。
- 用复杂真实书籍替代合成夹具；真实书籍属于后续兼容性回归。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收要求 |
|---|---|---|
| FR | FR-001, FR-005–FR-007 | 六类有效 EPUB 可读；不支持/危险内容明确阻断 |
| FR | FR-070–FR-073, FR-087 | 分页/滚动/模式/Locator 技术门槛通过 |
| FR | FR-120–FR-122 | 嵌套目录可解析到 spine/fragment |
| FR | FR-130 | CSS/图片/字体在 WebKit 中实际加载 |
| FR | FR-160–FR-161 | Locator 捕获/恢复及失败回退有证据 |
| AC | AC-002, AC-031, AC-070, AC-120, AC-130, AC-160 | M3 spike 覆盖 |
| NFR | NFR-005 | 布局内 turn-page p95 ≤ 150 ms |
| NFR | NFR-006 | 100 MB EPUB 只读取目标章节 |
| NFR | NFR-011 | 完整阅读夹具无出站网络请求 |
| NFR | NFR-012 | 失败有稳定可理解结果 |
| NFR | NFR-013 | 协议、架构、引擎边界可独立验证 |

## 4. 六类有效夹具

沿用 `crates/epub-core/tests/support/fixtures.rs` 的真实来源，至少覆盖：

| 编号 | 夹具 | 必须验证 |
|---|---|---|
| M3-F01 | `MinimalEpub2` | EPUB2/NCX、XHTML 加载、分页初始化 |
| M3-F02 | `MinimalEpub3` | EPUB3/nav、XHTML 加载、分页初始化 |
| M3-F03 | `NestedToc` | 两层 TOC、同一 spine 内 fragments、跨 spine 映射 |
| M3-F04 | `MissingMetadata` | 缺失 metadata 仍可阅读；无封面占位不影响正文 |
| M3-F05 | `CssAndImage` 扩展字体 | 内嵌 CSS、PNG、同源字体按需加载和渲染 |
| M3-F06 | `Bilingual` | 中文/English、emoji、多 spine、Locator 往返 |

夹具不得把用户私人书籍提交到仓库。允许在测试运行时生成；不得修改原始 EPUB 字节。

每个夹具必须同时经过：

1. Rust `parse_epub`、会话和 `protocol::handle`。
2. Playwright WebKit 的 XHTML/CSS/字体/图片与阅读 script 页面。
3. 章节/目录/href 的交叉核对。

## 5. 技术门槛

### 5.1 阅读与资源

- 六个夹具的每个 spine XHTML 返回 200、正确 MIME、CSP nonce 和可信引擎标签。
- CSS、PNG、字体返回原始字节、正确 MIME、`nosniff`、`no-store`。
- 非 spine HTML 不注入阅读引擎。
- 六类夹具 WebKit 初始化都只发送一次 ready，并产生 init location。
- 相对资源 URL、中文/空格路径、fragment 均保持语义。

### 5.2 安全

- 原书 script、事件属性、`javascript:`、iframe/object/embed、base、meta refresh 被移除。
- remote HTTP/HTTPS 图片、CSS、脚本和字体请求数为 0。
- 外部导航、协议相对 URL、控制字符混淆和 ping 不产生请求或顶层导航。
- 伪造 source、origin、sessionId、token、乱序 init/ready 均被忽略。

### 5.3 分页、滚动与 Locator

固定 viewport 和基准 CSS，使用长文本夹具：

- pageCount > 1，首页/末页 clamp，末页有可见正文，无空白尾页。
- next/prev p95 ≤ 150 ms，至少 20 次交互；记录测量方法和 debug/release 类型。
- 窗口 resize、分页→滚动→分页、initialLocator、字体 ready 后保持 quote/textOffset。
- 跨 spine Locator 与文本缺失使用不同稳定错误。
- 滚动顶部/底部各只发送一次 spine-request。

### 5.4 嵌套目录映射

- 对 `NestedToc` 递归验证父项、`#s1/#s2` 子项和第二章分别映射到正确
  `spineIndex`、`spineHref`、fragment。
- 目录 href 必须按规范自行解析到归档内资源，不使用 URL 猜测或文件名模糊匹配。
- 映射测试可以位于 `epub-core` 测试支持或后续导航服务边界；不得把目录 UI 提前加入。

### 5.5 100 MB

- 复用 `fixtures::large_epub()`。
- 请求小 XHTML 时读取字节 <1 MB、峰值不加载 100 MB 大条目、不创建整书副本。
- 记录测试构建类型和读取量。

### 5.6 真实平台证据边界

- 自动化使用 Playwright WebKit + Tauri/Rust mock 协议测试。
- macOS Tauri WKWebView 的自动化驱动若不可用，报告必须明确写明；不得把 Playwright
  HTTP context 描述成真实 `epub://` 全链路。
- 可增加不进入产品包的测试导出/路由适配，但不得降低生产 CSP/协议安全或新增运行时
  网络服务；若需新依赖，先停止并申请。
- 不要求 M3D 在没有正式阅读 UI 的情况下伪造打包应用人工操作证据。

## 6. 报告要求

`docs/acceptance/M3_SPIKE_REPORT.md` 至少包含：

- 环境：macOS、CPU/内存、Rust、Tauri、WebKit/Playwright、构建类型。
- 六类夹具逐项结果与测试页面/命令。
- 安全攻击矩阵和网络请求计数。
- 分页页数、末页可见性、resize/mode/initial Locator 结果。
- turn-page 20 次测量的原始分页/百分位数据。
- 100 MB 读取字节和内存/耗时证据。
- 嵌套目录逐节点映射表。
- 已知差异、未验证项、失败复现和风险。
- `Gate proposal: PASS / FAIL` 及建议 ADR 结论。最终状态由 Codex/用户决定。

报告不得只写“通过”；每个门槛必须引用自动化测试或明确标注未验证。

## 7. 文件权限

### 允许修改

- `tests/e2e/`、`crates/epub-core/tests/support/`、必要的 Rust 集成测试
- `src/reader-engine/`、`src/bridge/`、`src-tauri/src/protocol/`
  （只修复 M3 门槛发现的缺陷）
- 测试专用导出脚本或 fixture adapter
- `package.json` scripts（只增加本地测试编排，不新增依赖）
- `src-tauri/resources/reader-engine.js` 构建产物
- `docs/acceptance/M3_SPIKE_REPORT.md`
- 本任务状态和第 10 节开发报告

### 禁止修改

- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`
- `docs/decisions/` 中的 ADR；最终 ADR 由 Codex 编写
- 数据库 schema、storage/library reader-source 语义
- 正式 UI、路由、Tauri capability 或 CSP 降级
- M4/M5 任务和其他任务文件

不得新增外部依赖。若现有工具不能完成某项证据，报告 `blocked` 或提出最小依赖申请，
不得把未证明项标为通过。

## 8. 必跑命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p epub-core
cargo test -p reader-model
cargo test -p qurner --lib
cargo test -p qurner --test m3_protocol
cargo test -p qurner --test m3_security
cargo test --workspace
npm run check
npm run test:e2e
npm run build
npm run tauri build
```

性能、100 MB 和任何辅助 exporter 的命令也必须原样记录。

## 9. 禁止事项

- 不提交、不推送、不创建 PR。
- 不安装依赖、不访问非本地网络、不修改用户书籍或个人数据。
- 不降低 CSP、sandbox、manifest 白名单或消息鉴权来让 WebKit 测试通过。
- 不把 Playwright/HTTP 结果描述为真实 Tauri WKWebView 结果。
- 不因 M3D 顺手实现 M4/M5 UI 或进度持久化。
- 不跳过失败测试，不在报告中隐藏已知浏览器差异。

## 10. DeepSeek 开发报告

> 仅在开始实现后填写。开始实现时将状态改为 `in_progress`。

**完成摘要：**

用六类有效 EPUB、Rust 协议矩阵、WebKit 自动化、安全矩阵、Locator 往返、100 MB 按需
读取和 20 次 turn-page 测量，完成 M3 技术门槛 spike，并输出
`docs/acceptance/M3_SPIKE_REPORT.md`。

- **六类 Rust 矩阵**（新增 `src-tauri/tests/m3_matrix.rs`，3 passed）：复用
  `crates/epub-core/tests/support/` 的 fixtures（`#[path]` 共享），对 MinimalEpub2、
  MinimalEpub3、NestedToc、MissingMetadata、CssAndImage、Bilingual 逐类验证
  `parse_epub`、会话、每个 spine 的 200/MIME/CSP nonce/唯一引擎标签/data-spine-*、
  非 spine HTML 不注入、CSS/PNG/字体/NCX 原始字节与 nosniff/no-store。
- **CssAndImage 扩展字体**：fixtures 增加 `font.ttf`（manifest `font/ttf`），
  parse 测试 manifest 期望 4→5；fixtures 移除 tempfile 依赖，改为 std 临时目录，
  以便 src-tauri 共享。
- **六类 WebKit 矩阵**（新增 `tests/e2e/m3-matrix.spec.ts`）：M3-F01–F06 各加载
  等价 XHTML、CSS、PNG、字体与阅读引擎，断言 ready 恰好一次、init location、
  spine 元数据、CSS 生效（text-indent 32px）、PNG naturalWidth≥1、font.ttf 请求 200、
  无出站请求；另含 20 次 turn-page 性能测试。
- **嵌套目录映射**（新增 `crates/epub-core/tests/toc_mapping.rs`，2 passed）：
  递归解析 NestedToc 父项/`#s1`/`#s2`/第二章到 spineIndex+spineHref+fragment，
  并交叉核对章节 `id="s1"/"s2"`。
- **100 MB**：`resource_read` 增加读取字节输出；大 EPUB 小章节读取 2,690 字节
  （<1 MB），未读大条目。
- **性能**：WebKit 内 20 次交替 next/prev 测量 postMessage→page-changed，
  p95 = 8 ms（门槛 150 ms），原始数据见报告。
- **安全**：汇总既有 `m3_security`（8）与 `reader-security`（5）覆盖：主动内容移除、
  0 远程请求、导航阻断、伪造消息忽略、CSP 保持。

**修改文件：**

- `src-tauri/tests/m3_matrix.rs`（新增）
- `crates/epub-core/tests/toc_mapping.rs`（新增）
- `crates/epub-core/tests/support/fixtures.rs`（去 tempfile、字体、allow(dead_code)）
- `crates/epub-core/tests/support/zip_writer.rs`（allow(dead_code)，供 src-tauri 共享）
- `crates/epub-core/tests/parse_epub.rs`（CssAndImage manifest 5）
- `crates/epub-core/tests/resource_read.rs`（打印 M3D_LARGE_READ）
- `tests/e2e/m3-matrix.spec.ts`（新增）
- `docs/acceptance/M3_SPIKE_REPORT.md`（新增）
- `docs/tasks/TASK-M3D.md`（状态与本第 10 节）

未修改 `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、ADR、
数据库 schema、正式 UI/路由、Tauri capability/CSP；未新增依赖；未创建 commit。

**关键设计选择：**

- 六类夹具复用 `epub-core` 测试支持：src-tauri 通过 `#[path]` 共享 zip_writer 与
  fixtures；fixtures 改为 std 临时目录以避免新增 dev-dependency。
- 目录映射放在 epub-core 集成测试：按 OPF 目录解析 href，fragment 单独保留，再与
  spine href 精确匹配（不使用文件名模糊匹配）。
- WebKit 矩阵使用等价合成页面 + HTTP route 夹具；真实 `epub://` 由 Rust 协议测试覆盖，
  报告中明确区分。
- 性能在浏览器内测量 postMessage→page-changed，避免 Playwright IPC 开销；测量在
  pageCount>5 的非边界页交替翻页。

**六类夹具矩阵：**

| 编号 | 夹具 | Rust（m3_matrix） | WebKit（m3-matrix） |
|---|---|---|---|
| M3-F01 | MinimalEpub2 | spine 200、NCX 200、CSP、引擎标签 | ready=1、init、请求 3、无出站 |
| M3-F02 | MinimalEpub3 | nav 存在、spine 200、nav 不注入 | ready=1、init、请求 3 |
| M3-F03 | NestedToc | spine 200、`#s1/#s2`、toc 2 顶层 | ready=1、init、请求 3 |
| M3-F04 | MissingMetadata | title/authors 空、spine 200 | ready=1、init、请求 3 |
| M3-F05 | CssAndImage+字体 | CSS/PNG/font 原始字节与 MIME | 样式 32px、PNG 加载、font 200、请求 6 |
| M3-F06 | Bilingual | spine 2、中英文本 | ready=1、init、请求 3 |

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p epub-core` | 0 | 131 passed，0 failed，1 ignored |
| `cargo test -p reader-model` | 0 | 14 passed |
| `cargo test -p qurner --lib` | 0 | 17 passed |
| `cargo test -p qurner --test m3_protocol` | 0 | 13 passed |
| `cargo test -p qurner --test m3_security` | 0 | 8 passed |
| `cargo test -p qurner --test m3_matrix` | 0 | 3 passed |
| `cargo test -p epub-core --test toc_mapping` | 0 | 2 passed |
| `cargo test -p epub-core --test resource_read large_epub -- --nocapture` | 0 | 1 passed；`M3D_LARGE_READ bytes=2690 chapter_len=208` |
| `cargo test --workspace` | 0 | 248 passed，0 failed，1 ignored |
| `npm run check` | 0 | `tsc -b`、Biome、Vitest 6 文件 27 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 20 passed |
| `npm run build` | 0 | 引擎 + 主应用 Vite 构建成功 |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.14 MiB |

**性能与 100 MB 证据：**

- 性能：WebKit、Vite production engine（minify），20 次交替 next/prev 的
  postMessage→page-changed 原始数据（ms）：
  `[1,1,1,1,1,1,1,2,1,1,8,8,8,8,8,8,8,8,8,8]`；p50=8，p95=8，max=8，门槛 150 ms。
- 100 MB：`M3D_LARGE_READ bytes=2690 chapter_len=208`；读取 <1 MB，未加载大条目，
  未创建整书副本（debug 构建，CountingReader 统计）。
- 环境：macOS 26.6（25G72）、Apple M5、16 GB、rustc/cargo 1.100.0、Tauri 2.12.5、
  Node 26.13.0、Playwright 1.65.0（WebKit build 2388）。

**目录映射：**

| 目录项 | 资源 | fragment | spineIndex | spineHref |
|---|---|---|---:|---|
| 第一部分 | `OEBPS/chapter1.xhtml` | — | 0 | `OEBPS/chapter1.xhtml` |
| 第一节 | `OEBPS/chapter1.xhtml` | `s1` | 0 | `OEBPS/chapter1.xhtml` |
| 第二节 | `OEBPS/chapter1.xhtml` | `s2` | 0 | `OEBPS/chapter1.xhtml` |
| 第二部分 | `OEBPS/chapter2.xhtml` | — | 1 | `OEBPS/chapter2.xhtml` |

**未验证项与浏览器差异：**

- 真实 Tauri WKWebView 的 `epub://` 全链路自动化未执行；Playwright 使用 HTTP route
  夹具模拟，Rust mock 协议覆盖真实 `epub://` 响应。报告中已明确区分。
- 字体渲染：WebKit 仅验证 CSS 应用、PNG 加载、font.ttf 请求 200；伪 TTF 不产生
  自定义字形，未验证真实字体字形渲染。
- 复杂真实书籍（图片、表格、绝对定位、固定宽度元素）的分页空白/溢出未验证。
- pageCount 亚像素 ±1；`caretRangeFromPoint` 依赖 WebKit；跨章节只发
  `spine-request`；大文本节点内 Locator 精度允许一个节点误差。
- 未测试滚动模式方向键。

**Gate Proposal：** PASS

建议：保留 `epub://` + sandbox iframe + 受控注入 + CSS 多栏分页架构；在 M4/M5 前
补充真实 WKWebView 自动化/冒烟；M6 前做真实书籍兼容回归；Locator v1 文本方案继续，
跨章节由导航层串联。最终 ADR 由 Codex/用户决定。

**风险与待决问题：**

- CSS 多栏在复杂原书样式下可能产生空白页或横向溢出，需真实书籍复核。
- resize/字体 layout 可能多次触发，前端以最后一次为准。
- 引擎约 104 kB 无缓存；每次文档加载重新执行。
- 跨 spine 仅报 `E_LOCATOR_SPINE_MISMATCH`，父桥需在 M5 串联章节。
- Playwright 与 WebKit 版本差异可能影响性能数据；需在 Tauri WKWebView 复核。

**依赖变更：**

- 无新增外部依赖；fixtures 移除 tempfile 使用 std 临时目录。
- `package.json`/`Cargo.toml` 未新增依赖。

## 11. Codex 审查

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p epub-core`：131 passed，0 failed，1 ignored。
- `cargo test -p reader-model`：14 passed。
- `cargo test -p qurner --lib`：17 passed。
- `cargo test -p qurner --test m3_protocol`：13 passed。
- `cargo test -p qurner --test m3_security`：8 passed。
- `cargo test -p qurner --test m3_matrix -- --nocapture`：3 passed；六类夹具全部打印。
- `cargo test -p epub-core --test toc_mapping -- --nocapture`：2 passed。
- 100 MB 按需读取：`M3D_LARGE_READ bytes=2690 chapter_len=208`。
- `cargo test --workspace`：248 passed，0 failed，1 ignored。
- `npm run check`：Biome 28 个文件，Vitest 6 文件 27 tests 通过。
- `npm run test:e2e -- --reporter=line`：Playwright WebKit 20 passed。
- 独立性能复测：20 次 turn-page p95 = 9 ms，max = 10 ms。
- `npm run build`：通过；reader-engine 104.25 kB。
- `npm run tauri build`：通过；生成 7.14 MiB `Qurner.app`。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现阻断 M3 技术路线的 `BLOCKER` 或 `MAJOR`。 | — |

**AC 结果：**

- FR-001、FR-005–FR-007：通过；六类 Rust 矩阵与安全矩阵成立。
- FR-070–FR-073、FR-087：通过；WebKit 分页、滚动、模式、resize、字体及 Locator
  自动化通过。
- FR-120–FR-122：通过；嵌套目录映射到 spineIndex/spineHref/fragment。
- FR-130：通过 CSS/PNG 加载；字体请求与 CSS 生效通过，伪 TTF 不验证真实字形。
- FR-160–FR-161：通过；Locator round-trip、恢复与失败回退通过。
- NFR-005：通过；独立 p95 9 ms ≤150 ms，测量为 WebKit HTTP 夹具内事件时延。
- NFR-006：通过；100 MB 文件小章节读取 2,690 字节。
- NFR-011：通过当前安全矩阵；无出站请求。
- AC-002、AC-031、AC-070、AC-120、AC-130、AC-160：通过 M3 spike 范围。

**Gate 结论与 ADR 建议：**

Codex Gate：**PASS**。

保留 `epub://` + sandbox iframe + 受控脚本注入 + CSS 多栏分页 + Locator v1 方向。
真实 Tauri WKWebView 的 `epub://` 端到端冒烟必须作为 M4 首个 UI 集成门槛；复杂真实
EPUB 兼容性继续作为 M6 发布门槛。用户已于 2026-11-14 接受 ADR-0003，M3 正式关闭。
