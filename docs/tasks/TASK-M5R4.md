# TASK-M5R4：100 MB 大文本 EPUB 导入修复

**状态：** accepted

**里程碑：** M5 rework

**负责人：** DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-16

**依赖：** TASK-M5E-C blocked

## 1. 目标

结构有效、总大小约 100 MB、正文主要集中在单个 XHTML 的 EPUB 可以通过真实 Qurner.app
导入并打开，不再因为正文超过内容安全扫描的 16 MiB 上限而被拒绝。

## 2. 证据与根因

- `/tmp/qurner-acceptance/big/book-100mb.epub`：106,041,723 B，ZIP/XML 结构有效。
- 单章 `chapter1.xhtml` 106,040,153 B。
- `crates/epub-core/src/content.rs` 的 `MAX_SCAN_BYTES = 16 MiB` 让 `read_entry` 返回
  `LimitExceeded`，导入显示固定 EPUB 无效提示。
- 99 MB 资源型 EPUB 可导入、打开，说明问题与大文本 XHTML 的内容检查路径有关。

## 3. 范围

### 必须完成

- 支持 100 MB 级单个 XHTML 的内容安全扫描，不因正常正文大小误报 `LimitExceeded`。
- 保留固定版式、竖排、活动内容、远程资源等安全检查；不能简单关闭内容扫描。
- 处理过程内存受控，不把整本 100 MB 无界加载；维持现有资源按需读取边界。
- 合成大文本 EPUB 在真实 app 中导入成功并可打开首章。
- 记录真实 app 导入耗时，满足 SRS 的 100 MB 导入不超过 10 秒目标；失败必须如实报告。
- 原文 SHA-256 在导入前后不变。

### 不属于本任务

- M5E-C 全量重跑、1,000 本预置证据、断网补测。
- TXT/PDF、高亮、批注、备份或 M6。
- 数据库 schema、依赖升级、CSP/capability 或 iframe 协议变更。

## 4. 需求追踪

| 类型 | 编号 | 验收要求 |
|---|---|---|
| FR | FR-001–FR-014 | 有效 EPUB 可导入，原始文件只读 |
| NFR | NFR-002, NFR-006 | 支持单本 100 MB EPUB；导入不超过 10 秒 |
| NFR | NFR-010 | 原始电子书只读，不复制或改写 |
| AC | AC-002, AC-010, AC-130, AC-210 | 有效导入、书库记录、正文和打开链路成立 |

## 5. 文件权限

### 允许修改

- `crates/epub-core/src/`
- `crates/epub-core/tests/`
- `crates/library-core/tests/` 中必要的大文本导入回归
- `src-tauri/tests/` 中必要的命令/集成回归
- `docs/tasks/TASK-M5R4.md`

### 禁止修改

- SRS/SDS/PLAN/ADR/其他任务
- 数据库 schema、依赖/package/lock 文件、CSP/capability、iframe 协议
- 真实用户书库、原始 EPUB 或个人数据

## 6. 测试与真实验收

- 新增约 100 MB 单 XHTML 的合成 EPUB 回归，验证导入成功、章节可打开、SHA-256 不变。
- 超过明确总大小或单条目的安全上限仍返回 `LimitExceeded`。
- 固定版式、竖排、脚本、事件属性、iframe/object 和远程资源的拒绝/警告行为不得回退。
- 记录 release 构建峰值内存或可核验的流式读取证据，避免无界整包加载。
- 隔离 HOME 的真实 Qurner.app 导入并打开大文本章节，截图与计时证据。

## 7. 必跑命令

```bash
npm run check
npm run test:e2e
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri build
git diff --check
```

## 8. 禁止事项

- 不只提高常量到无界值，也不关闭安全检查。
- 不以 99 MB 资源型 EPUB 代替大文本 100 MB 证据。
- 不修改原始文件，不提交、推送或发布。

## 9. DeepSeek 开发报告

**完成摘要：**

修复 100 MB 单 XHTML EPUB 因内容扫描上限被拒绝的问题（`LimitExceeded`），并按首轮审查
意见修复 CSS 安全检查跨 64 KiB 片段边界被绕过的问题。

**第四轮返工复验（CSS 字符串上下文）：**

- 问题：`content:"/*"` 字符串中的 `/*` 被状态机误判为注释起点，其后
  `writing-mode:vertical-rl` 被忽略；审查探针 `parse_epub` 接受该竖排书。
- 修复：增加 **单/双引号字符串状态** 与 **反斜杠转义**：
  - 任意状态遇到 `'`/`"` 进入字符串状态（记录引号与返回状态），字符串内 `/*`/`*/`
    不再解释；`\` 转义下一个字符（含引号）；字符串可跨片段，状态跨块保持。
  - 从 `url(`/`@import`/scheme 状态进入的字符串继续检测 `http://`/`https://` 前缀
    （`url("https://...")`、`@import 'https://...'` 仍产生远程警告）。
  - 字符串结束回原状态继续解析（`url("...")` 后回到 `AfterUrlParen`）。
- 回归：`content:"/*"` 与 `content:"*/"` 后的竖排仍拒绝；转义引号字符串；跨 64 KiB
  字符串边界；单/双引号 `url`/`@import` 远程警告。内容测试 24 项通过；独立探针：
  字符串注释标记 → `VerticalWriting`、注释 URL → `warnings=3`、106 MB → `OK`（134 ms）。

**第三轮返工复验（UTF-8 与 CSS 注释）：**

- **BLOCKER 修复**：`Idle` 缓冲改为 **仅缓存 ASCII 标记字符**（非 ASCII 字符清空缓冲），
  长度裁剪只发生在 ASCII 上，`String::drain` 不再可能落在 UTF-8 字符内部；合法中文 CSS
  标识符（`a中` + 20 ASCII + `中{color:red}`）不再 panic。
- **MAJOR 修复**：注释不再覆盖待匹配状态。新增 `pending_slash`（延迟判定 `/*`）与
  `comment_return`（记录 `/` 之前的语境界）；`url(/**/https://...)` 注释结束后回到
  `AfterUrlParen` 继续检测，`writing-mode:/**/vertical-rl` 注释后回到 `AfterColon`，
  `@import/**/"https://..."` 注释后继续 scheme 检测。
- 回归测试：
  - 中文 CSS 标识符不 panic 且无警告；
  - `url(/**/https://...)` → `RemoteResourceBlocked`；
  - `@import/**/"https://..."` → `RemoteResourceBlocked`；
  - `writing-mode:/**/vertical-rl` → `VerticalWriting`；
  - 原有超长空白/跨片段/相邻 style 隔离测试保留。
  内容测试 19 项通过；独立 release 探针：中文 CSS `OK`（无 panic）、注释 URL
  `warnings=3`、注释竖排 `VerticalWriting`、106 MB 文本 `OK`（157 ms）。

**第二轮返工复验（有界流式 CSS 状态）：**

- 首轮尾部窗口（1024 B）仍无法覆盖“`writing-mode:` 后超过 1024 字节空白再接
  `vertical-rl`”的场景；审查探针复现 `vertical_detected=false`。
- 修复：以 **按 `<style>` 元素隔离的有界流式 CSS 状态机** `CssStreamScanner` 取代尾部窗口：
  - 逐字符状态（Idle / AfterWritingMode / AfterColon / ValuePrefix / AfterUrlParen /
    SchemePrefix / AfterImport），`writing-mode` 冒号后跨任意空白/注释保持待匹配状态，
    不缓存空白内容（O(1) 内存）。
  - `url(` / `@import` 后跨空白/引号/注释检测 `http://`/`https://` 前缀；`/* ... */`
    注释状态跳过。
  - `chunk.last_in_text_node()` 时合并结果并重置状态，多个 `<style>` 元素互不串状态。
  - 其余安全检查（script/iframe/object、`on*`、`javascript:`、固定版式、`style` 属性
    CSS）保持原逻辑。
- 新增测试：`writing-mode:` + 2000 空格 + `vertical-rl`（跨 64 KiB）拒绝；`url(` + 2000
  空格 + `https://...` 警告；相邻两个 `<style>` 状态隔离；原有跨片段/安全测试保留。
  内容测试 15 项通过。

**首轮返工复验（CSS 跨片段）：**

- `lol_html` 会把 `<style>` 文本切分为任意片段；此前逐片段独立匹配，`writing-mode:
  vertical-rl` 或 `url(https://...)` 恰好跨 64 KiB 边界时会被漏检（审查探针已复现）。
- 修复：`ScanState` 增加 **有界尾部窗口** `css_carry`（最多 1024 字节，字符对齐）；
  每个 `<style>` 文本片段先拼接 carry 再执行 `css_has_vertical_writing` /
  `css_has_remote_resource`，随后把更新后的尾部窗口写回。状态有界（每 style 元素 ≤1 KiB），
  不随文档大小增长；其他检查（script/iframe/object/on*/javascript:/固定版式）不变。
- 新增跨 64 KiB 边界回归：`ChunkedReader` 以 64 KiB 分块投递，竖排关键字/远程 `url(...)`
  起始于边界前，分别断言 `VerticalWriting` 错误与 `RemoteResourceBlocked` 警告。
- 测试：content 12/15/19/24 项（四轮）；全 workspace 317 passed。

- **根因**：`content.rs` 的 `read_entry` 将整个 XHTML 读成 `String` 且以
  `MAX_SCAN_BYTES = 16 MiB` 拒绝更大条目；106 MB 单章 EPUB 在内容安全检查阶段失败。
- **修复**：
  - 小资源（OPF/CSS/display-options）继续使用 16 MiB 的 `read_entry`（安全不变）。
  - 新增标记流式扫描：`MAX_MARKUP_SCAN_BYTES = 200 MiB`（与 precheck 单条目上限一致，
    仍为明确上限）、`scan_markup_reader` 以 64 KiB 分块写入 `lol_html`，不把整章读入
    `String`；`scan_markup_entry` 从 ZIP 条目直接流式读取，超过上限仍返回 `LimitExceeded`。
  - 保留原有全部安全检查：`script`/`iframe`/`object`、`on*` 事件属性、`javascript:` URL、
    远程 `img/link/video/audio/source/track/poster/use`、`style` 竖排、CSS 竖排、
    OPF/display-options 固定版式；远程资源仍只产生 `RemoteResourceBlocked` 警告。
  - 抽取 `resolve_entry_name` 复用百分号解码查找逻辑。
- **测试**：
  - 新增约 100 MB 单 XHTML 合成 EPUB 回归：`parse_epub` 成功、spine=1、导入前后
    SHA-256 不变（复用 `fingerprint_file`）。
  - 超过 200 MiB 的流式标记返回 `LimitExceeded`（`std::io::repeat` 流，测试内存受控）。
  - 安全检查回归：script/iframe/object/onclick/javascript:/竖排 均返回固定错误；远程
    img 产生警告；普通正文通过。
  - 既有 `content_safety` 21 项与其它 epub-core 测试全部通过。
- **真实应用**：
  - 隔离 HOME（`/tmp/qurner-acceptance/final/home`，合成书库）导入 106,041,723 B 文本型
    EPUB `book-100mb.epub` 成功；“百兆大书”记录写入 DB。
  - 计时：Enter 触发 `T=23:49:29.315` → DB `imported_at=15:49:30Z`、导入完成
    `23:49:31.56`，**导入耗时约 1.2 秒 ≤ 10 秒**（注：此前鼠标点击 Open 未触发，实际以
    Enter 触发为计时起点；方法为触发时刻与 DB 记录/结果出现时间差）。
  - SHA-256 前后均为 `d6abec99c237664b2264b820fd2af94bf18c75eaeaa851c6e207b38d7e6ccfb7`，
    文件大小不变；原文只读。
  - 打开首章约 23 秒后显示正文“大文件”（首屏 106 MB XHTML 渲染较慢；属观察项）。
- **内存受控**：release 探针对 106 MB 文件 `parse_epub` 耗时 137 ms；扫描使用 64 KiB
  缓冲区与流式 rewriter，不整章 `String`；precheck 单条 200 MiB / 总量 1 GiB 上限保留。

**修改文件：**

- `crates/epub-core/src/content.rs`（流式扫描、常量与测试）
- `docs/tasks/TASK-M5R4.md`

未修改数据库/schema、依赖/锁文件、CSP/capability、iframe 协议、SRS/SDS/PLAN/ADR/其他任务、
真实书库；未提交 commit。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 16 文件 169 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 45 passed |
| `npm run build` | 0 | 生产前端构建成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 317 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 重建成功 |
| `git diff --check` | 0 | 通过 |

**真实应用验收：**

- 截图：`/tmp/qurner-acceptance/shots3/32-m5r4-imported2.png`（导入结果）、
  `33-m5r4-reader.png`（打开中）、`34-m5r4-reader2.png`（首章正文）。
- SHA-256：`/tmp/qurner-acceptance/final/sha-before.txt` 与 `sha-after.txt` 一致。
- DB：`books` 中“百兆大书”`imported_at=2026-11-16T15:49:30Z`。
- 导入耗时约 1.2 秒（计时方法见上）。

**未验证项：**

- 打开 106 MB 单章首屏约 23 秒（任务仅要求可打开；未设定打开耗时门槛）；滚动/分页在该
  大章的性能未单独测量。
- M5E-C 全量重跑、1,000 本预置证据、断网补测不在本任务范围。
- 超 200 MiB 单条目的真实 ZIP 拒绝由 precheck 既有上限覆盖（本次用流式单元测试验证扫描
  上限），未生成 200 MiB+ 真实文件。

**风险与待决问题：**

- 200 MiB 扫描上限与 precheck 单条目上限一致；若后续需要更大单章，应同时调整 precheck、
  扫描上限和渲染策略（另开任务）。
- 首屏渲染 100 MB 文本接近交互可接受边界，建议后续按需分段/分页渲染（P1）。

**依赖与禁止路径：**

- 未修改 SRS/SDS/DEVELOPMENT_PLAN/AI_COLLABORATION/ADR/其他任务、数据库 schema、依赖、
  CSP/capability、iframe 协议、真实书库/原文。
- 未启动 M5E-C 重跑/M6；未提交/推送。

## 10. Codex 审查

**结论：** 返工

**审查日期：** 2026-11-17

**独立执行的检查：**

- `npm run check`：通过；Biome、TypeScript 和 14 个 Vitest 文件 169 项测试通过。
  仍有 `ReaderShell` 的 1 条 `act(...)` 警告，属于既有 MINOR，不影响本任务结论。
- `npm run test:e2e`：沙箱内 WebKit 启动即 `Abort trap: 6`；经授权在沙箱外重跑，
  Playwright WebKit 45/45 通过。
- `npm run build`：通过，生产前端和 reader engine 构建成功。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，303 passed、0 failed、1 ignored。
- `npm run tauri build`：通过，生成 `target/release/bundle/macos/Qurner.app`（7.92 MiB）。
- `git diff --check`：通过。
- 复核 `/tmp/qurner-acceptance/big/book-100mb.epub`：106,041,723 B，ZIP 结构有效；
  `chapter1.xhtml` 为 106,040,153 B；SHA-256 为
  `d6abec99c237664b2264b820fd2af94bf18c75eaeaa851c6e207b38d7e6ccfb7`，与导入前记录一致。
- 复核真实应用截图及隔离数据库：导入结果显示 1 本成功；`books` 中存在 active 的
  “百兆大书”记录，指纹与文件一致，`imported_at=2026-11-16T15:49:30Z`，数据库
  `PRAGMA integrity_check` 为 `ok`；首章正文截图可见。
- 使用同版本 `lol_html 3.0.3` 和真实 `parse_epub` 做 64 KiB 边界最小复现：
  `vertical_detected=false`、`remote_warning_detected=false`、
  `parse_epub=accepted_vertical_book`。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `crates/epub-core/src/content.rs:372` | `lol_html` 的文本处理器收到的是任意切分的文本片段，但 `style` 内的竖排检测和远程资源检测逐片段独立执行。64 KiB 流式边界切开 `writing-mode:vertical-rl` 或 `url(https://...)` 后会漏检；真实 EPUB 探针确认竖排书可被 `parse_epub` 接受。这违反任务第 6 节“竖排……和远程资源的拒绝/警告行为不得回退”及 AC-002/FR-005。 | 保留跨文本片段所需的有限尾随状态，或对 `<style>` 文本使用有界流式 CSS 词法状态；新增恰好跨 64 KiB 边界的竖排拒绝和远程资源警告回归。 |

**AC 结果：**

- AC-002：**不通过**。有效大文本 EPUB 可导入，但跨流式边界的竖排内容可绕过拒绝规则。
- AC-010、AC-130、AC-210：**通过**。真实应用导入、首章正文显示、打开链路和原文摘要不变
  均有独立复核。
- NFR-002、NFR-006：**通过**。106 MB 单章真实应用导入约 1.2 秒，低于 10 秒；扫描路径
  使用固定 64 KiB 缓冲区，未把 XHTML 读成完整 `String`。
- NFR-010：**通过**。导入前后文件大小和 SHA-256 不变，原始文件内容未被改写。

**返工范围：**

- 仅修复 `crates/epub-core/src/content.rs` 的跨片段 CSS 安全检测，并补充上述边界回归。
- 重新运行本任务第 7 节全部命令；无需重做 M5E-C 全量验收或扩大扫描上限。

### 第二轮审查（跨片段修复复验）

**结论：** 返工

**审查日期：** 2026-11-17

**独立执行的检查：**

- `npm run check`：通过；14 个 Vitest 文件 169 项测试通过，仍保留既有
  `ReaderShell` `act(...)` MINOR 警告。
- `npm run test:e2e`：沙箱外 Playwright WebKit 45/45 通过。
- `npm run build`：通过。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：305 passed、0 failed、1 ignored。
- `npm run tauri build`：通过，`Qurner.app` 为 7.92 MiB。
- `git diff --check`：通过。
- 使用真实 `parse_epub` 独立构造 64 KiB 边界，并在 `writing-mode:` 与其值之间放置
  2048 个合法 CSS 空白：探针输出 `vertical_detected=false`、
  `parse_epub=accepted_vertical_book`。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `crates/epub-core/src/content.rs:347` | 1024 字节尾部窗口不能替代跨片段 CSS 词法状态。CSS 允许属性冒号与值之间存在任意长度空白；当 `writing-mode:` 在边界前、随后超过 1024 字节空白、再接 `vertical-rl` 时，属性名被尾部窗口丢弃，竖排书仍被接受。这仍违反 AC-002/FR-005 和任务第 6 节的竖排拒绝要求。 | 改为有界、按 `<style>` 元素隔离的流式 CSS 状态：识别到 `writing-mode` 冒号后跨任意空白保持待匹配状态；style 结束时重置。补超过 1024 字节空白的跨块回归，以及相邻多个 `<style>` 不串状态的回归。 |

**AC 结果：**

- AC-002：**不通过**。跨片段竖排绕过后仍可导入。
- AC-010、AC-130、AC-210、NFR-002、NFR-006、NFR-010：维持上一轮通过结论。

**返工范围：**

- 仅修改 `crates/epub-core/src/content.rs` 及其单元测试。
- 除新增的空白跨越与 style 隔离回归外，保持当前 100 MB 与既有安全检查路径不变。

### 第三轮审查（状态机复验）

**结论：** 返工

**审查日期：** 2026-11-17

**独立执行的检查：**

- `npm run check`：通过；14 个 Vitest 文件 169 项测试通过，仍保留既有
  `ReaderShell` `act(...)` MINOR 警告。
- `npm run test:e2e`：沙箱外 Playwright WebKit 45/45 通过。
- `npm run build`：通过。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：308 passed、0 failed、1 ignored。
- `npm run tauri build`：通过，`Qurner.app` 为 7.92 MiB。
- `git diff --check`：通过。
- 上一轮 2048 字节空白探针：现返回 `VerticalWriting`，原缺陷已修复。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| BLOCKER | `crates/epub-core/src/content.rs:288` | `buffer` 以字节长度截断，并用 `String::drain` 从可能落在 UTF-8 字符内部的偏移删除。合法 CSS 选择器 `a中` + 20 个 ASCII 字符 + `中{color:red}` 会让真实 `parse_epub` 在 `content.rs:290` panic，进程以 101 退出。 | 检测缓冲只保存 ASCII 标记字符，或按字符边界实现有界环形状态；新增长中文 CSS 标识符回归，确保合法 EPUB 不会被 panic。 |
| MAJOR | `crates/epub-core/src/content.rs:264` | 注释状态会覆盖待匹配状态。`<style>body{background:url(/**/https://example.com/x.png)}</style>` 的真实 `parse_epub` 返回成功但 `remote_warning=false`；旧整串检测会从 `url(` 到 `)` 识别该远程 URL，远程资源警告发生回退。 | 修正 `comment_return` 在待匹配状态中的推进，补 CSS 注释插入 `url(...)`、`@import` 和属性值后的跨片段回归。 |

**AC 结果：**

- AC-002：**不通过**。合法 EPUB 可因非 ASCII CSS 标识符触发解析 panic，远程资源警告也未完全保持。
- AC-010、AC-130、AC-210、NFR-002、NFR-006、NFR-010：维持此前通过结论。

**返工范围：**

- 仅修改 `crates/epub-core/src/content.rs` 及其单元测试。
- 保留当前 100 MB 流式读取、跨任意空白与 `<style>` 隔离设计，修复 UTF-8 缓冲和注释状态两个缺陷。

### 第四轮审查（字符串语义复验）

**结论：** 返工

**审查日期：** 2026-11-17

**独立执行的检查：**

- `npm run check`：通过；14 个 Vitest 文件 169 项测试通过，仍保留既有
  `ReaderShell` `act(...)` MINOR 警告。
- `npm run test:e2e`：沙箱外 Playwright WebKit 45/45 通过。
- `npm run build`：通过。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：312 passed、0 failed、1 ignored。
- `npm run tauri build`：通过，`Qurner.app` 为 7.92 MiB。
- `git diff --check`：通过。
- 上一轮中文标识符 panic 和 `url(/**/https://...)` 漏报均已修复。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `crates/epub-core/src/content.rs:264` | 状态机没有 CSS 字符串上下文，会把字符串字面量中的 `/*` 误判为注释起点。合法样式 `<style>p:before{content:"/*";}p{writing-mode:vertical-rl}</style>` 的真实 `parse_epub` 返回成功，后续竖排声明未被拒绝；旧整串扫描可检测该竖排声明。 | 在状态机中增加单/双引号字符串状态及反斜杠转义处理，字符串内不得识别注释、`writing-mode` 或远程资源；补字符串内 `/*`、`*/` 与转义引号的跨片段回归。 |

**AC 结果：**

- AC-002：**不通过**。有效 CSS 字符串可遮蔽后续竖排声明，竖排书籍仍可被导入。
- AC-010、AC-130、AC-210、NFR-002、NFR-006、NFR-010：维持此前通过结论。

**返工范围：**

- 仅修改 `crates/epub-core/src/content.rs` 及其单元测试。
- 保持当前 UTF-8 安全缓冲、注释后继续检测、跨任意空白和 100 MB 流式路径不变。

### 第五轮审查（最终验收）

**结论：** 通过

**审查日期：** 2026-11-17

**独立执行的检查：**

- `npm run check`：通过；14 个 Vitest 文件 169 项测试通过。
- `npm run test:e2e`：沙箱外 Playwright WebKit 45/45 通过。
- `npm run build`：通过。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：317 passed、0 failed、1 ignored。
- `npm run tauri build`：通过，`Qurner.app` 为 7.92 MiB。
- `git diff --check`：通过。
- 独立复验上一轮字符串绕法：`content:"/*"` 后的
  `writing-mode:vertical-rl` 返回 `VerticalWriting`。
- 使用 release 构建的 `inspect_epub` 直接解析
  `/tmp/qurner-acceptance/big/book-100mb.epub`：成功，1 个 spine、1 个 chapter weight，
  指纹为 `d6abec99c237664b2264b820fd2af94bf18c75eaeaa851c6e207b38d7e6ccfb7`。

**问题：**

本轮未发现任务范围内的 BLOCKER 或 MAJOR。

**AC 结果：**

- AC-002：**通过**。有效大文本 EPUB 可导入；固定版式、竖排、脚本、嵌入内容和远程资源
  检查保持，跨片段、超长空白、CSS 注释与字符串语境均有回归。
- AC-010、AC-130、AC-210：**通过**。此前真实应用导入、首章正文显示、打开链路和原文
  摘要不变证据有效；最终 release 解析复核通过。
- NFR-002、NFR-006：**通过**。100 MB 单章路径使用 64 KiB 有界流式缓冲，真实导入约
  1.2 秒，满足 10 秒门槛。
- NFR-010：**通过**。原始 EPUB 大小与 SHA-256 未变化。

**后续：**

- 重开 `TASK-M5E-C`，补齐 100 MB 大文本与 1,000 本正确预置打开证据；本轮不启动该任务。
