# TASK-M1E：内容兼容与安全检测

**状态：** accepted  
**里程碑：** M1  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** `TASK-M1D` accepted

## 1. 目标

在 `parse_epub` 返回前识别 MVP 不支持的 EPUB 结构，并阻断会执行代码或依赖远程资源的
内容：

- 固定版式 EPUB。
- 竖排或从右到左正文布局。
- 原书脚本和事件处理器。
- HTML/CSS 中的远程资源。

不得执行任何书内脚本，也不得发起网络请求。

## 2. 用户授权

用户已授权 M1 开发。本任务不得新增运行时依赖；使用现有 `quick-xml`、`lol_html`、
`rbook` 和 `ResourcePath` 完成。

## 3. 固定版式

读取当前 OPF package 文档，并拒绝：

- `<meta property="rendition:layout">pre-paginated</meta>`。
- `<meta name="fixed-layout" content="true">`。
- Apple `META-INF/com.apple.ibooks.display-options.xml` 中明确的
  `fixed-layout=true`。
- 等价大小写或属性顺序写法。

命中时返回 `EpubErrorCode::FixedLayout`。错误只携带 OPF 或 display-options 的
`ResourcePath`。

## 4. 竖排

扫描所有 manifest 中 `text/css` 资源，以及 XHTML/SVG 中：

- `<style>` 内容。
- `style` 属性。

拒绝以下 CSS 声明值：

- `writing-mode: vertical-rl`
- `writing-mode: vertical-lr`
- `writing-mode: sideways-rl`
- `writing-mode: sideways-lr`
- 对应的 `-webkit-writing-mode`

必须大小写不敏感，并允许声明值前后空白。命中时返回
`EpubErrorCode::VerticalWriting`，资源路径指向包含声明的 HTML/CSS 文件，而不是用户
绝对路径。

已由 M1B 拒绝的 `page-progression-direction=rtl` 保持现状。

## 5. 脚本和活动内容

扫描所有 XHTML、HTML 和 SVG manifest 资源。发现以下任一项即返回
`EpubErrorCode::ScriptDependency`：

- `<script>` 元素，不论内联或外部。
- HTML/SVG 事件属性，例如 `onload`、`onclick`、`onerror`。
- `javascript:` URL。
- `<iframe>` 或 `<object>` 等嵌入执行内容的结构。

MVP 采用保守策略：即使脚本可能不是阅读所必需，也拒绝整本书并明确提示不支持。

## 6. 远程资源

检测 HTML/CSS 中会加载资源的远程 URL：

- `http://` 和 `https://`。
- HTML 资源属性：`img src/srcset`、`link href`、`video/audio/source/track src`、
  `video poster`、SVG image/use href。
- CSS `url(...)` 和 `@import`。
- 远程字体、图片、样式和媒体。

普通 `<a href="https://...">` 是外部导航链接，不算远程资源；MVP 不打开它，也不在
本任务中产生 warning。`data:` URI 允许。

发现远程资源时不拒绝整本书，但：

- 在 `ParsedBook.warnings` 添加一条 `RemoteResourceBlocked`。
- warning 的 `resource_href` 指向包含该引用的本地 HTML/CSS 文件。
- 同一本地文件有多条远程引用时只报告一次。
- 不保存远程 URL，避免泄漏或误导。

## 7. 执行顺序和 API

`parse_epub` 签名和 `ParsedBook` schema version 不变。

建议顺序：

1. ZIP/加密预检。
2. rbook 打开与 metadata/manifest/spine 映射。
3. 固定版式检查。
4. 内容安全扫描。
5. TOC、封面和章节权重。

固定版式、竖排或脚本命中时直接返回错误，不继续读取章节内容。

## 8. 不属于本任务

- CSP 和 iframe 运行时隔离；M3。
- 修复或清洗 HTML。
- 执行脚本、下载远程资源或打开链接。
- ZIP local header/CRC 二次校验。
- UI、Tauri、SQLite 或导入事务。

## 9. 文件权限

### 允许修改

- `crates/epub-core/src/`
- `crates/epub-core/tests/`
- 本任务顶部状态和第 11 节开发报告

### 禁止修改

- `Cargo.toml`
- `Cargo.lock`
- `crates/epub-core/Cargo.toml`
- `src/`
- `src-tauri/`
- `tests/e2e/`
- `crates/reader-model/`
- `SRS.md`
- `SDS.md`
- `DEVELOPMENT_PLAN.md`
- `AI_COLLABORATION.md`
- `AGENTS.md`
- `docs/decisions/`
- 其他任务文件

## 10. 测试要求

- 现代 `rendition:layout` 与 legacy fixed-layout 均返回 `FixedLayout`。
- iBooks display-options 固定版式返回 `FixedLayout`。
- 外部 CSS、inline style、`<style>` 和大小写变体中的竖排均返回 `VerticalWriting`。
- `<script>`、`onload`、`onerror`、`javascript:`、`iframe` 均返回
  `ScriptDependency`。
- 远程图片、CSS import、字体、媒体和 `srcset` 产生去重后的
  `RemoteResourceBlocked` warning。
- 普通外部 `<a>` 不产生远程资源 warning。
- 本地图片、CSS 和 data URI 不产生 warning。
- 错误/warning 不包含远程 URL或用户绝对路径。
- 4 本公有领域 EPUB 在环境变量测试模式下仍可解析。

## 11. 必跑命令与报告

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p epub-core
cargo test --workspace
QURNER_TEST_EPUB_DIR=/private/tmp/qurner-m1p/books \
  cargo test -p epub-core --test public_domain -- --ignored --nocapture
npm run check
```

**完成摘要：**

新增 `epub-core` 内容兼容与安全检测模块，在 `parse_epub` 返回前按顺序执行：

1. 流式 ZIP 预检与加密检查（M1B/M1D 既有）。
2. rbook 打开与 metadata/manifest/spine 映射。
3. 固定版式检查（OPF `rendition:layout`、legacy `fixed-layout` meta、
   iBooks `display-options.xml`）。
4. 内容安全扫描（竖排 CSS、脚本/活动内容、远程资源）。
5. TOC、封面与章节权重（M1C 既有）。

固定版式、竖排或脚本命中时直接返回错误，不继续读取章节内容；远程资源不拒绝整本书，
只为每个包含引用的本地文件添加一条去重的 `RemoteResourceBlocked` warning。未新增任何
运行时依赖，只使用现有 `zip`、`quick-xml`、`lol_html` 和 `ResourcePath`。不执行脚本、
不访问网络、不修改文件。

本轮返工按第 12 节修复三项：

- 对 HTML `style` 属性和 `<style>` 内容执行同一 CSS 远程资源检测；命中时为所在
  HTML 文件生成去重 warning。
- `css_has_remote_resource` 循环检查所有 `url(...)` 与 `@import`，不再只检查第一个
  `@import`。
- manifest 声明为 CSS/XHTML/SVG 的资源在归档中缺失时返回 `InvalidOpf`；只有可选的
  `META-INF/com.apple.ibooks.display-options.xml` 缺失允许跳过。

**修改文件：**

- `crates/epub-core/src/content.rs`（新增）
- `crates/epub-core/src/lib.rs`（注册模块）
- `crates/epub-core/src/parse.rs`（调用内容检查；新增权重/脚本相关单元测试）
- `crates/epub-core/tests/content_safety.rs`（新增）
- `crates/epub-core/tests/toc_cover_weights.rs`（适配新的脚本拒绝：外部 TOC 测试移除
  `javascript:` 项，权重跳过测试不再包含 `<script>`）
- `docs/tasks/TASK-M1E.md`（顶部状态与本第 11 节）

未修改 `Cargo.toml`、`Cargo.lock`、`crates/epub-core/Cargo.toml`、`src/`、`src-tauri/`、
`tests/e2e/`、`crates/reader-model/` 和受保护文档，未创建 commit。

**固定版式与竖排：**

- 固定版式命中（均返回 `FixedLayout`）：
  - `<meta property="rendition:layout">pre-paginated</meta>`，路径 `OEBPS/content.opf`；
  - `<meta content="TRUE" name="fixed-layout"/>`（属性顺序与值大小写变体）；
  - `META-INF/com.apple.ibooks.display-options.xml` 中
    `<option name="fixed-layout">true</option>` 或 `fixed-layout="true"` 属性形式，
    路径为该 display-options 文件。
- 竖排命中（均返回 `VerticalWriting`，路径指向包含声明的资源）：
  - 外部 `text/css` 中 `WRITING-MODE :  Vertical-RL`；
  - XHTML `style` 属性 `writing-mode: sideways-lr`；
  - `<style>` 内容中 `-webkit-writing-mode: vertical-rl`。
  - 大小写不敏感，允许冒号/值前后空白；`page-progression-direction=rtl` 仍由 M1B
    处理，不变。

**脚本和远程资源：**

- 脚本/活动内容命中（返回 `ScriptDependency`，路径为本地资源）：
  - `<script>` 元素（内联或外部）；
  - `onload`、`onerror` 等 HTML/SVG 事件属性；
  - 任意属性中的 `javascript:` URL（允许前导空白）；
  - `<iframe>` 与 `<object>`。
- 远程资源（`RemoteResourceBlocked` warning，每文件去重，`resource_href` 为本地文件）：
  - `img src`、`img srcset`、`link href`、`video/audio/source/track src`、
    `video poster`、SVG `image/use` 的 `href` / `xlink:href`；
  - CSS `url(...)` 与所有 `@import` 中的 `http://` / `https://`；
  - HTML `style` 属性和 `<style>` 内容中的远程 `url(...)`、`@import` 与字体引用。
- 不产生 warning：
  - 普通 `<a href="https://...">` 外部导航链接；
  - 本地图片、CSS `url(images/...)`、本地 `@import`；
  - `data:` URI。
- 错误与 warning 均不包含远程 URL、用户绝对路径或 OS 错误文本；warning 只保存
  `ResourcePath`。
- manifest 声明的 `text/css`、`application/xhtml+xml`、`text/html`、`image/svg+xml`
  条目缺失时返回 `InvalidOpf` 并携带该资源的相对路径；匿名/可选 display-options 缺失
  仍跳过。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p epub-core` | 0 | 112 passed，1 ignored（lib 38、content_safety 21、error 5、fingerprint 4、fixtures 13、model 7、parse_epub 10、toc_cover_weights 14、public_domain 1 ignored） |
| `cargo test --workspace` | 0 | 119 passed，0 failed，1 ignored |
| `QURNER_TEST_EPUB_DIR=<临时目录> cargo test -p epub-core --test public_domain -- --ignored --nocapture` | 0 | 4 本公有领域 EPUB 全部通过（spine 14/17/32/31，TOC、封面与权重保持非空） |
| `npm run check` | 0 | Biome 14 个文件无修复；Vitest 2/2 通过 |

未新增运行时依赖，`Cargo.toml`/`Cargo.lock` 无变化。

**未验证项：**

- CSS 注释中的 `http(s)` URL 仍可能触发远程 warning（保守策略，不区分注释）。
- CSS 检测基于声明/字符串扫描，不是完整 CSS 解析；CSS 变量、转义或复杂层叠可能漏检
  竖排/远程引用。
- XHTML/SVG 使用 `lol_html` 的 HTML 解析语义，不是严格 XML；命名空间属性（如
  `xlink:href`）按 HTML 解析结果处理。
- 未检测 `<frame>`、`<embed>`、`<canvas>`、CSS `behavior`/`-moz-binding` 等其他活动
  内容；任务只要求 script/事件属性/javascript:/iframe/object。
- 未用真实含脚本或远程资源的 EPUB 验证；仅合成夹具与 4 本 Gutenberg 样本。
- 缺失资源只覆盖 manifest 中声明为 HTML/CSS/SVG 的条目；图片、字体等缺失仍由后续
  读取按需报错。
- 未实现 CSP/iframe 运行时隔离（M3）。

**风险与待决问题：**

- “含脚本即拒绝整本书”是任务要求的保守策略，会拒绝部分可读但带脚本的书籍。
- CSS 注释或自定义属性中出现 `writing-mode:`/远程 URL 可能误报；当前只对脚本/竖排
  返回错误，远程误报仅产生 warning，不阻断书库。
- manifest 声明缺失即 `InvalidOpf` 会让部分结构不完整但可读的书籍失败；这是任务要求
  的严格策略，M2 导入汇总需要给出可理解的原因。
- 内容扫描需要读取所有 HTML/CSS 资源，会在权重计算前额外读取一遍章节；M2 导入流程
  可考虑复用已读取内容以减少 IO。
- 远程资源去重按本地文件；同一远程资源被多个文件引用会各产生一条 warning（按任务
  要求）。
- HTML 事件属性检测按 `on*` 前缀，可能把非事件自定义属性（如 `once`）误判为活动内容；
  保守可接受，后续可按完整事件属性名白名单收紧。

## 12. Codex 审查

> Codex 仅在任务进入 `ready_for_review` 后填写。

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p epub-core`：107 passed，1 ignored。
- `cargo test --workspace`：114 passed，1 ignored。
- 4 本公有领域 EPUB 专项测试通过。
- `npm run check`：通过，Vitest 2/2。
- 未新增运行时依赖，也未新增 commit。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| `MAJOR`（已修复） | `crates/epub-core/src/content.rs` / `scan_markup` | 远程资源检测未覆盖 HTML `style` 属性和 `<style>` 内容。 | 已使用同一 CSS 检查扫描两处，并增加对应 warning 测试。 |
| `MAJOR`（已修复） | `crates/epub-core/src/content.rs` / `css_has_remote_resource` | 只检查第一个 `@import`。 | 已循环检查全部 `@import` 与 `url(...)`，本地 import 后接远程 import 的测试通过。 |
| `MAJOR`（已修复） | `crates/epub-core/src/content.rs` / `check_content` | manifest 内容资源缺失时静默跳过。 | 已返回带相对路径的 `InvalidOpf`；缺 CSS/XHTML 测试通过。 |

**M1E 结果：**

固定版式、竖排、脚本、全部 HTML/CSS 远程资源与 manifest 完整性检查均满足任务要求，
M1E 验收通过。

**返工或后续任务：**

1. 修复内联/`<style>` 远程 CSS 扫描、全部 `@import` 循环和 manifest 缺失资源错误。
2. 增加对应三组回归测试。
3. 重新运行第 11 节全部命令，更新报告并改为 `ready_for_review`。
4. 不新增依赖，不修改受保护文档，不创建 commit。

### 复验记录

**复验日期：** 2026-11-14  
**结论：** 通过

- `cargo test -p epub-core`：112 passed，1 ignored。
- `cargo test --workspace`：119 passed，1 ignored。
- 4 本公有领域 EPUB 专项测试通过。
- `style` 属性、`<style>`、后续远程 `@import` 和 manifest 缺失资源测试全部通过。
- `npm run check`：通过，Vitest 2/2。
- 没有新增依赖或 commit，未修改受保护文档。
