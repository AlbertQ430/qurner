# TASK-M1C：TOC、封面与章节权重

**状态：** accepted  
**里程碑：** M1  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** `TASK-M1B` accepted

## 1. 目标

扩展 `parse_epub`，在 M1B 的 metadata/manifest/spine 基础上填充：

- EPUB2 NCX 和 EPUB3 nav 目录树。
- 封面引用。
- 每个 spine 项的文本权重。

完成后，`ParsedBook.toc`、`cover` 和 `chapter_weights` 应为真实数据；资源仍按需读取。

## 2. 用户授权

用户已授权 M1 开发。本任务允许：

- 新增 SDS 已列出的 `lol_html` 运行时依赖。
- 修改 `epub-core` 和其测试依赖。
- 使用公有领域 EPUB 做临时兼容性测试。

不得新增其他运行时依赖。

## 3. 目录归一化

### 3.1 层级

- 使用 rbook 解析出的内容目录。
- 删除 rbook 的合成 root `contents` 节点，把 root 的直接子项作为
  `ParsedBook.toc` 顶层，不能多出一层空目录。
- 保留原顺序和任意层级 children。
- EPUB2 NCX 与 EPUB3 nav 都必须覆盖。

### 3.2 href

- 本地 TOC href 统一为归档内相对资源路径加可选 fragment，例如
  `OEBPS/chapter1.xhtml#section-1`。
- 去掉不必要的前导 `/` 和 `./`，不进行会改变资源身份的错误 URL 解码。
- fragment 原样保留；base path 必须能构造 `ResourcePath`。
- `http:`、`https:`、`mailto:` 等外部 TOC 项跳过，并添加一条
  `EpubWarningCode::DegradedMarkup` warning，避免生成不可用导航目标。
- 非法本地路径返回 `UnsafeResourcePath`。

### 3.3 id 和 label

- rbook 提供稳定 id 时优先使用。
- 无 id 时按父路径和同级索引生成稳定 id，例如 `toc-0`、`toc-0-1`。
- label 去除首尾空白并合并连续空白。
- label 为空时使用 `未命名目录项`，不得使用用户绝对路径或文件系统路径。

## 4. 封面

- 使用 rbook 的 cover image 识别结果。
- `CoverReference.href` 使用规范化 `ResourcePath`。
- `media_type` 保留原 manifest MIME 类型。
- 只接受 `image/*` MIME；不是图片时视为无封面并记录 warning。
- 没封面时 `cover = None`，添加 `EpubWarningCode::MissingCover`。
- 本任务不解码图片、不生成缩略图、不写缓存文件。
- 不把封面图片字节保存进 `ParsedBook`。

## 5. 章节权重

对每个 spine 项：

1. 通过 rbook 读取对应 XHTML 字符串。
2. 使用 `lol_html` 流式处理 HTML/XHTML，只累计正文文本。
3. 跳过 `script`、`style`、`noscript`、`template` 和标签本身。
4. 按 Unicode 字符数计算，而不是 UTF-8 字节数。
5. 权重为 0 时使用资源字节长度作为回退。
6. 权重以 `u32` 饱和转换，不能 panic 或回绕。
7. 结果长度必须与 `spine` 等长。

单个章节读取失败时：

- 返回 `InvalidOpf`，不得静默产生错误的 0 权重。
- 不得加载非当前章节资源。

## 6. 不属于本任务

- 图片解码、封面缩略图和缓存。
- HTML 脚本/远程资源完整安全扫描；由 M1D 完成。
- 固定版式、竖排 CSS 和内容级脚本检测。
- SHA-256 文件流式计算。
- Tauri、SQLite、UI 或阅读器。

## 7. 公开接口

`parse_epub(path: &Path) -> Result<ParsedBook, EpubError>` 签名、schema version 和
metadata/manifest/spine 行为不变。只新增有效 toc、cover 和 chapter_weights 内容。

不得公开 rbook、lol_html 或 zip 类型。

## 8. 文件权限

### 允许修改

- `Cargo.toml`
- `Cargo.lock`
- `crates/epub-core/Cargo.toml`
- `crates/epub-core/src/`
- `crates/epub-core/tests/`
- 本任务顶部状态和第 10 节开发报告

### 禁止修改

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

## 9. 测试要求

- EPUB2 NCX：顶层目录数量、label、href 与顺序正确。
- EPUB3 nav：合成 root 被移除，两层嵌套结构保持。
- 外部 TOC href 被跳过并产生 warning。
- 空 label 使用确定性回退。
- 非法 TOC 路径返回 `UnsafeResourcePath`。
- `css-and-image.epub` 识别 PNG 封面。
- 无封面夹具返回 `None` 和 `MissingCover`。
- `chapter_weights.len() == spine.len()`。
- 中文与英文夹具权重为正，且中文按字符数而非 UTF-8 字节数。
- 空正文使用资源字节回退。
- `toc`、`cover`、`chapter_weights` JSON round-trip。
- 4 本公有领域 EPUB 在环境变量测试模式下仍通过。

## 10. 必跑命令与报告

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p epub-core
cargo test --workspace
QURNER_TEST_EPUB_DIR=/private/tmp/qurner-m1p/books \
  cargo test -p epub-core --test public_domain -- --ignored --nocapture
npm run check
```

## 11. DeepSeek 开发报告

> DeepSeek 开始后把顶部状态改为 `in_progress`，完成后填写本节并改为
> `ready_for_review`。

**完成摘要：**

`parse_epub` 在 M1B 的 metadata/manifest/spine 基础上填充 `toc`、`cover` 和
`chapter_weights`，签名、schema version 和既有字段行为不变。所有 `rbook`、`lol_html`、
`zip` 类型仍限制在 crate 内部。本轮返工按第 12 节修复五项：

- 章节权重只统计 `<body>` 内文本，继续跳过 `script`、`style`、`noscript`、`template`；
  非空 `<title>` 不再影响权重。
- 外部 TOC href 改用通用 URI scheme 判断（RFC 3986 scheme 语法），并把 `//host/path`
  视为外部；单字母 scheme 仍按 Windows 盘符交给 `ResourcePath` 拒绝。
- 整棵 TOC 的 id 全局去重：重复或缺失时按位置生成稳定 id，并与已用 id 冲突时追加后缀。
- 更新 `parse_epub` 的过期文档注释，明确 M1C 会读取 TOC、封面引用和章节文本。
- 封面 MIME 使用 ASCII 大小写不敏感的 `image/` 前缀判断，并补充单元与集成测试。

原有 M1C 能力保持：EPUB2 NCX / EPUB3 nav 归一化并移除合成 root；href 去掉前导 `/`
与 `./`、保留 fragment；无封面或非图片 MIME 产生 `MissingCover`；权重按 Unicode
字符数、0 时回退资源字节长度、长度与 spine 等长。

**修改文件：**

- `Cargo.toml`、`Cargo.lock`（lol_html 依赖）
- `crates/epub-core/Cargo.toml`
- `crates/epub-core/src/parse.rs`（TOC、封面、权重映射与单元测试）
- `crates/epub-core/tests/parse_epub.rs`（M1B 断言更新为真实 TOC/封面/权重）
- `crates/epub-core/tests/toc_cover_weights.rs`（新增）
- `crates/epub-core/tests/public_domain.rs`（输出每本书的 spine/toc/cover/权重统计）
- `docs/tasks/TASK-M1C.md`（顶部状态与本第 11 节）

未修改 `src/`、`src-tauri/`、`tests/e2e/`、`crates/reader-model/` 和受保护文档，
未创建 commit。

**TOC 归一化：**

- `minimal-epub2`（NCX）：`toc.len() == 1`，label `第一章`，href
  `OEBPS/chapter1.xhtml`，无合成 root。
- `minimal-epub3`（nav）：`toc.len() == 1`，`id` 回退为 `toc-0`。
- `nested-toc`：顶层 2 项，第一部分 children 1 项，`OEBPS/chapter1.xhtml#s1`
  fragment 保留，顺序与 nav 一致。
- 外部目录项：`https:`、`mailto:`、`javascript:`、自定义 `foo:` 与协议相对
  `//example.com` 均跳过并各产生一条 `DegradedMarkup` warning；单字母 `C:` 不视为外部，
  由 `ResourcePath` 返回 `UnsafeResourcePath`。
- 空 label：`   ` 归一化为空后回退 `未命名目录项`。
- 非法本地路径：`../escape.xhtml`、`/../escape.xhtml`、`C:\evil.xhtml` 均返回
  `UnsafeResourcePath`。
- 重复 id：两个 NCX `navPoint id="dup"` 被重新生成为 `toc-0`、`toc-1`；TOC 内全部 id
  全局唯一。

**封面与 warning：**

- `css-and-image`：`cover` 为 `OEBPS/cover.png`，media type `image/png`。
- 其余 5 个成功夹具：`cover = None` + `MissingCover`。
- 非图片封面 MIME（`text/plain` + `properties="cover-image"`）：`None` + `MissingCover`。
- 大写 `IMAGE/PNG` 封面：仍识别为封面；rbook 会把 manifest media-type 规范化为小写，
  因此 `CoverReference.media_type` 为 `image/png`。大小写判断由
  `is_image_media_type` 单元测试直接覆盖（`IMAGE/PNG`、`Image/Jpeg` 为真）。
- 加密字体夹具：同时存在 `EncryptedFontFallback` 与 `MissingCover`，测试按类别过滤。

**章节权重：**

- 中文“中文正文”：权重 4，等于 Unicode 字符数而非 12 个 UTF-8 字节。
- `正文` + `script`/`style`/`noscript`/`template` 内容：权重 2，脚本与样式文本被跳过。
- 非空 `<title>不应计入的标题文字</title>` + body `正文`：权重 2，title 不计入。
- 空 XHTML：权重回退为资源字节长度（等于该章节文件字节数）。
- 所有夹具与 4 本公有领域 EPUB 均满足 `chapter_weights.len() == spine.len()`；
  公版书权重全部非零。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p epub-core` | 0 | 74 passed，1 ignored（lib 25、error 5、fixtures 13、model 7、parse_epub 10、toc_cover_weights 14、public_domain 1 ignored） |
| `cargo test --workspace` | 0 | 81 passed，0 failed，1 ignored |
| `QURNER_TEST_EPUB_DIR=<临时目录> cargo test -p epub-core --test public_domain -- --ignored --nocapture` | 0 | 4 本通过：spine 14/17/32/31，toc 16/63/32/32，cover 均为 true，权重全部非零 |
| `npm run check` | 0 | Biome 14 个文件无修复；Vitest 2/2 通过 |

依赖：`cargo tree -p epub-core` 92 个唯一包，`Cargo.lock` 457 个包；新增运行时依赖仍仅
`lol_html 3.0.3` 及其传递依赖。

**未验证项：**

- 未实现封面图片解码、缩略图或缓存（不属于本任务）。
- 真实 EPUB 的 TOC 层级只覆盖 Gutenberg 平铺目录；嵌套与重复 id 由合成夹具覆盖。
- 未构造接近 `u32::MAX` 的超长章节验证饱和转换；代码使用
  `u32::try_from(weight).unwrap_or(u32::MAX)`。
- 未测试非 UTF-8 章节内容；读取失败统一映射为 `InvalidOpf`。
- 100 MB 打开与按需读取未在 M1C 重新测量（M1P 数据仍适用）。
- 未运行 EPUBCheck。

**风险与待决问题：**

- TOC id 混合 rbook 提供的 id 与按位置生成的 id；若 rbook id 重复或缺失，生成规则为
  `toc-N` / `toc-N-M`，冲突时追加 `-1`、`-2` 后缀。顺序稳定，但可能与原始 id 风格
  不同。
- 外部目录项会连同其子树一起跳过（只记录 warning）；若真实书籍把本地章节挂在外部
  父项下，会丢失这些导航目标。
- 通用 scheme 判断会把含冒号的相对文件名（如 `chapter:1.xhtml`）视为外部；EPUB 中
  此类路径很少，但需要在 M1D 用真实样本验证。
- href 不做 URL 解码，读取资源时必须使用相同 percent-encoded 路径。
- 权重只统计 `<body>` 内文本，但保留正文空白；若需要排除纯空白需在后续任务调整。
- `lol_html 3.0.3` 为新增运行时依赖；升级 mini 版本需要重跑权重与安全回归。

## 12. Codex 审查

> Codex 仅在任务进入 `ready_for_review` 后填写。

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p epub-core`：69 passed，1 ignored。
- `cargo test --workspace`：76 passed，1 ignored。
- 4 本公有领域 EPUB 专项测试通过；TOC、封面和权重均非空。
- `npm run check`：通过，Vitest 2/2。
- 新增运行时依赖仅为 `lol_html 3.0.3`。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| `MAJOR`（已修复） | `crates/epub-core/src/parse.rs` / `count_text_characters` | `text!("*")` 会统计 `<head><title>` 等非正文文本。 | 已只统计 `<body>` 内文本；非空 title 回归测试通过。 |
| `MAJOR`（已修复） | `crates/epub-core/src/parse.rs` / `is_external_toc_href` | 未识别 `javascript:`、未知 scheme 和 `//host/path`。 | 已使用通用 URI scheme 判断和协议相对地址规则；三类测试通过。 |
| `MAJOR`（已修复） | `crates/epub-core/src/parse.rs` / TOC id | rbook id 未做全局唯一性检查。 | 已使用全树分配器；重复 NCX id 回归测试通过。 |
| `MINOR`（已修复） | `crates/epub-core/src/parse.rs` / `parse_epub` 文档注释 | 注释与 M1C 实现不一致。 | 已更新为读取 TOC、封面引用和章节文本权重。 |
| `MINOR`（已修复） | `crates/epub-core/src/parse.rs` / `map_cover` | MIME 前缀判断区分大小写。 | 已改为 ASCII 大小写不敏感，并有单元及集成测试。 |

**M1C 结果：**

TOC 归一化、封面映射、章节权重、外部 URI 和依赖边界均满足任务要求，M1C 验收通过。

**返工或后续任务：**

1. 修复正文权重范围、通用外部 URI 识别和 TOC id 全局唯一性。
2. 修正过期注释和封面 MIME 大小写判断。
3. 添加上述五类回归测试。
4. 重新运行任务第 10 节全部命令，更新第 11 节并改为 `ready_for_review`。
5. 不修改受保护文档，不创建 commit。

### 复验记录

**复验日期：** 2026-11-14  
**结论：** 通过

- `cargo test -p epub-core`：74 passed，1 ignored。
- `cargo test --workspace`：81 passed，1 ignored。
- 4 本公有领域 EPUB 专项测试通过；TOC、封面和权重均正常。
- 非空 `<title>` 不贡献权重、外部 URI 跳过、重复 TOC id 去重、大写 MIME 测试通过。
- `npm run check`：通过，Vitest 2/2。
- 没有新增 commit，未修改受保护文档。
