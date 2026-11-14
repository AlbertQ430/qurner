# TASK-M1A：EPUB 领域模型与 Rust 夹具工厂

**状态：** accepted  
**里程碑：** M1  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** `TASK-M0` accepted  

## 1. 目标

为 M1 建立稳定、可序列化的 EPUB 领域模型、资源路径安全类型、稳定错误类别，以及
可由后续解析器测试复用的 Rust EPUB 夹具工厂。

本任务不实现 ZIP/OPF/目录解析。完成后，epub-core 和 reader-model 应具备后续解析任务
所需的公开数据结构，并能生成最小、有效、损坏和恶意的合成 EPUB 测试文件。

## 2. 用户授权

用户在 2026-11-14 明确同意开始 M1。该授权允许 DeepSeek：

- 修改根 Cargo workspace 中的依赖声明和锁文件。
- 使用 SDS 已列出的 Rust crate：`serde`、`serde_json`、`thiserror`、`zip`、
  `tempfile`。
- 在 `epub-core` 中建立本任务定义的模型、错误、资源路径和测试支持代码。

如需新增 SDS 未列出的运行时依赖，必须停止并报告，不得自行安装。

## 3. 范围

### 必须完成

- `reader-model::BookFingerprint`：保存 32 字节 SHA-256 摘要，支持十六进制
  `Display`/`FromStr`，JSON 序列化为小写 64 字符字符串。
- `epub-core` 的版本化 `ParsedBook` 模型和稳定 schema 版本常量。
- 书籍元数据、manifest、spine、目录节点、封面引用和章节权重模型。
- 解析警告与稳定错误类别。
- 安全的相对资源路径 `ResourcePath`。
- 全部公开模型使用 `serde`，字段命名保持 camelCase 以匹配前端 DTO 约定。
- Rust 测试夹具工厂，生成后续 M1 任务可复用的临时 EPUB。
- 模型序列化、路径安全和 fixture 结构的自动化测试。

### 不属于本任务

- 读取或解析真实 EPUB。
- SHA-256 文件流式计算；本任务只定义摘要类型。
- XML、OPF、NCX、nav 或 XHTML 解析。
- 封面图片解码或缩略图。
- Tauri command、数据库、UI 或阅读器。
- 私有 EPUB 夹具。

## 4. 需求追踪

本任务是 M1 基础能力，不直接关闭用户可见 FR。

| 类型 | 编号 | M1A 支撑内容 |
|---|---|---|
| FR | FR-001 | 为 EPUB 2/3 解析提供统一模型 |
| FR | FR-005 | 定义不支持格式的稳定错误类别 |
| FR | FR-007 | 定义损坏 EPUB 的错误类别 |
| FR | FR-016 | 定义 SHA-256 指纹值类型 |
| FR | FR-020–FR-022, FR-120–FR-121 | 定义 metadata、cover 和目录模型 |
| NFR | NFR-013 | 模型位于不依赖 Tauri 的领域 crate |
| NFR | NFR-014 | `ParsedBook` 带 schema version |

## 5. 公开模型

以下类型是 M1B–M1D 的稳定输入；字段可按本任务要求使用 `Option`，但不得引入
Tauri/UI 类型。

```rust
pub const PARSED_BOOK_SCHEMA_VERSION: u32 = 1;

pub struct BookMetadata {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
}

pub struct ManifestItem {
    pub id: String,
    pub href: ResourcePath,
    pub media_type: String,
    pub properties: Vec<String>,
}

pub struct SpineItem {
    pub index: usize,
    pub idref: String,
    pub href: ResourcePath,
    pub linear: bool,
}

pub struct TocNode {
    pub id: String,
    pub label: String,
    pub href: String,
    pub children: Vec<TocNode>,
}

pub struct CoverReference {
    pub href: ResourcePath,
    pub media_type: String,
}

pub struct EpubWarning {
    pub code: EpubWarningCode,
    pub resource_href: Option<String>,
}

#[non_exhaustive]
pub enum EpubWarningCode {
    MissingMetadata,
    MissingCover,
    RemoteResourceBlocked,
    EncryptedFontFallback,
    DegradedMarkup,
}

pub struct ParsedBook {
    pub schema_version: u32,
    pub package_path: ResourcePath,
    pub metadata: BookMetadata,
    pub manifest: BTreeMap<String, ManifestItem>,
    pub spine: Vec<SpineItem>,
    pub toc: Vec<TocNode>,
    pub cover: Option<CoverReference>,
    pub chapter_weights: Vec<u32>,
    pub warnings: Vec<EpubWarning>,
}
```

约束：

- 使用 `#[serde(rename_all = "camelCase")]` 或等价的显式重命名，保证 JSON 字段为
  camelCase。
- `ParsedBook::new` 或构造路径必须写入当前 schema version。
- manifest 使用 `BTreeMap<String, ManifestItem>`，确保序列化顺序稳定。
- `chapter_weights` 在解析完成后必须与 `spine` 等长；M1A 只定义模型，由 M1C 填充。
- 对可能继续扩展的错误类别和警告类别使用 `#[non_exhaustive]`。
- 不暴露用户绝对路径；`package_path` 是 EPUB 内的规范化相对路径。

## 6. ResourcePath 安全规则

`ResourcePath` 只接受 EPUB 归档内的相对 POSIX 路径。

必须拒绝：

- 绝对路径，例如 `/etc/passwd`。
- Windows 盘符和反斜杠路径。
- 任一 `..` 路径段。
- NUL 字符。
- 空路径。
- 解析后逃出归档根目录的路径。

必须规范化：

- 删除重复 `/`。
- 删除 `.` 路径段。
- 保留普通大小写和 Unicode 文件名。
- 输出不带前导 `/`。

必须提供安全的字符串访问和 `Display`；路径段不得被静默 URL 解码。URL 解码与 HTML
资源解析在 M1C/M1D 处理。

## 7. 错误模型

定义 `EpubError` 和对应 `EpubErrorCode`。至少包含：

- 无效或损坏 ZIP。
- 缺少 `META-INF/container.xml`。
- 缺少或无效 OPF。
- XML 解析失败。
- 加密正文或 DRM。
- 固定版式。
- 竖排或从右到左。
- 原书脚本依赖。
- 不安全的资源路径。
- 超出条目数量、单条目大小或总解压大小限制。
- I/O 错误。

要求：

- 错误包含相对资源路径或结构位置时，只允许 EPUB 内路径，不记录用户绝对路径。
- 提供稳定的 code 映射，后续 Tauri 层可映射到 SRS/SDS 的 `AppError`。
- 不泄漏 ZIP crate 或 XML crate 的错误类型到公开 API。
- 测试至少覆盖两个错误变体的 code/Display。

## 8. Rust 夹具工厂

在 `crates/epub-core/tests/support/` 建立仅供测试使用的 fixture builder，不进入公共
API，不把生成的 `.epub` 提交到仓库。

至少支持生成：

1. 最小 EPUB2：mimetype、container、OPF v2、manifest、spine、NCX 和一章 XHTML。
2. 最小 EPUB3：nav、manifest、spine 和一章 XHTML。
3. 嵌套目录：至少两层目录节点。
4. 缺失书名/作者/封面。
5. 内嵌 CSS 和图片。
6. 中文和英文正文。
7. 缺少 OPF 的损坏文件。
8. 包含不安全归档路径的文件。
9. 包含重复 ZIP 条目的文件。

ZIP 要求：

- `mimetype` 必须是第一个条目。
- `mimetype` 使用 Stored，不压缩。
- 内容为 `application/epub+zip`。
- 所有文件写入 `tempfile::TempDir`。
- API 返回可读取的文件路径并保持 TempDir 生命周期。

测试必须检查生成文件可被 ZIP 读取、mimetype 顺序/压缩方式正确，以及每个声明类型都有
对应夹具。

## 9. 文件权限

### 允许修改或创建

- `Cargo.toml`
- `Cargo.lock`
- `crates/reader-model/Cargo.toml`
- `crates/reader-model/src/`
- `crates/epub-core/Cargo.toml`
- `crates/epub-core/src/`
- `crates/epub-core/tests/`
- 本任务第 11 节 DeepSeek 开发报告

### 禁止修改

- `src/`
- `src-tauri/`
- `tests/e2e/`
- `SRS.md`
- `SDS.md`
- `DEVELOPMENT_PLAN.md`
- `AI_COLLABORATION.md`
- `AGENTS.md`
- `docs/decisions/`
- `docs/tasks/` 中除本任务第 11 节外的内容
- 仓库外任何文件

## 10. 必跑命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p reader-model
cargo test -p epub-core
cargo test --workspace
npm run check
```

`npm run check` 不要求修改前端；它用于确认新增 Rust 代码和 workspace 配置没有破坏现有
M0 基线。

## 11. DeepSeek 开发报告

> DeepSeek 开始后把顶部状态改为 `in_progress`，完成后填写本节并改为
> `ready_for_review`。

**完成摘要：**

本轮返工按第 12 节审查意见修复三项，其余 M1A 实现保持不变：

- 从 `epub-core` 删除 `AppErrorCode` 与 `EpubErrorCode::app_error_code()`；只保留
  `EpubErrorCode`（12 类，`#[non_exhaustive]`，camelCase），SDS `AppError` 的完整
  映射留到后续 Tauri 命令层。
- `EpubWarning.resource_href` 改为 `Option<ResourcePath>`。反序列化会拒绝绝对路径和
  `..` 路径；测试覆盖两种情况。
- `EpubError` 删除任意字符串 `location`、`location()` 和 `with_location()`，只保留
  稳定 code 和可选 `ResourcePath`；结构位置（如 `META-INF/container.xml`）也用
  `ResourcePath` 表示。
- ZIP 测试 writer 在写入前断言条目数不超过 `u16::MAX`，并新增 `should_panic` 测试。

原有能力保持：`BookFingerprint` 值类型、版本化 `ParsedBook`（schema v1）、
`ResourcePath` 安全规则、camelCase serde、9 类 Rust EPUB 夹具（mimetype 第一且
Stored，`TempDir` 生命周期）。自动化测试：workspace 共 40 个测试通过。

**修改文件：**

- `Cargo.toml`、`Cargo.lock`（workspace 依赖与锁文件）
- `crates/reader-model/Cargo.toml`、`crates/reader-model/src/lib.rs`、
  `crates/reader-model/src/fingerprint.rs`（新增）
- `crates/epub-core/Cargo.toml`、`crates/epub-core/src/lib.rs`、
  `crates/epub-core/src/model.rs`（新增）、`crates/epub-core/src/path.rs`（新增）、
  `crates/epub-core/src/error.rs`（新增）
- `crates/epub-core/tests/support/mod.rs`、`crates/epub-core/tests/support/zip_writer.rs`、
  `crates/epub-core/tests/support/fixtures.rs`（新增）
- `crates/epub-core/tests/model_serde.rs`、`crates/epub-core/tests/error_model.rs`、
  `crates/epub-core/tests/fixtures.rs`（新增）
- `docs/tasks/TASK-M1A.md`（顶部状态与本第 11 节）

未修改 `src/`、`src-tauri/`、`tests/e2e/` 和任何受保护文档，未创建 commit。

**公开模型与 crate 依赖：**

- 运行时依赖（SDS 已列）：`serde` 1（derive）、`thiserror` 2。
- 测试依赖：`serde_json` 1、`tempfile` 3、`zip` 2（仅测试读取夹具；关闭默认 features，
  只启用 `deflate`，供 M1B 读取真实 EPUB 时使用）。
- `ParsedBook`、`BookMetadata`、`ManifestItem`、`SpineItem`、`TocNode`、
  `CoverReference`、`EpubWarning` 均为 camelCase serde；`manifest` 使用
  `BTreeMap`；`EpubErrorCode` 与 `EpubWarningCode` 为 `#[non_exhaustive]`。
- `EpubWarning.resource_href` 与 `EpubError::resource_path()` 都是
  `Option<ResourcePath>`，公开 API 在类型层面保证只接受归档内相对路径。
- `epub-core` 不导出任何跨 IPC 应用错误类型。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p reader-model` | 0 | 6 passed |
| `cargo test -p epub-core` | 0 | 33 passed：lib 单元 8、error_model 5、fixtures 13、model_serde 7 |
| `cargo test --workspace` | 0 | 40 passed，0 failed；doc-tests 0 |
| `npm run check` | 0 | `tsc -b` 通过；Biome 14 个文件无修复；Vitest 2/2 通过 |

**夹具覆盖：**

| 夹具 | 覆盖内容 |
|---|---|
| `minimal-epub2.epub` | mimetype、container、OPF v2、NCX、一章 XHTML |
| `minimal-epub3.epub` | mimetype、container、OPF v3、nav（`properties="nav"`）、一章 XHTML |
| `nested-toc.epub` | nav 中两层嵌套目录，两个章节 |
| `missing-metadata.epub` | 无 `dc:title`、`dc:creator`、封面 |
| `css-and-image.epub` | 内嵌 `style.css` 与 1×1 PNG 封面 |
| `bilingual.epub` | 中文第一章与英文第二章 |
| `missing-opf.epub` | container 引用缺失的 OPF |
| `unsafe-paths.epub` | `../evil.txt`、`/absolute.txt`、`C:\windows\evil.txt`、`OEBPS/../../escape.txt` |
| `duplicate-entries.epub` | 两个同名 `OEBPS/chapter1.xhtml` 条目 |

测试逐项检查：9 类夹具文件名唯一且生成成功；每个文件的第一个 local header 都是
Stored `mimetype` 且内容为 `application/epub+zip`；7 个结构有效夹具可被 `zip` crate
打开并读取；重复条目的 ZIP 中央目录含 6 条记录；条目数超过 `u16::MAX` 时
`build_zip` 立即 panic。

**未验证项：**

- 未实现解析器，未验证 OPF/NCX/nav 的语义解析；夹具 XML 内容只按字符串检查。
- 未生成 100 MB 或超限 ZIP 夹具，也未验证条目数量/解压大小上限行为（M1D）。
- 未实现 SHA-256 流式计算，只验证了指纹值类型。
- `zip` crate 读取器会按文件名去重，因此重复条目断言基于原始 ZIP 字节。
- 未在 Tauri 命令层实现 SDS `AppError` 映射（按返工要求留到后续任务）。

**风险与待决问题：**

- `zip` 2.6.1 的 `ZipArchive` 用 `IndexMap` 按名称合并重复条目；M1B 若依赖该 API
  做完整性检查，需要自行解析中央目录，不能假设 `len()` 等于原始条目数。
- 测试夹具 ZIP 全部使用 Stored；若后续需要 Deflate 夹具或损坏中央目录用例，需要扩展
  测试写入器。
- `serde`/`thiserror` 已是 SDS 列出的依赖；`zip`（关闭默认 features，仅启用
  `deflate`）与 `tempfile` 仅作为 dev-dependency，不进入产品运行时。
- 本次返工未创建 commit（按任务要求），`Cargo.lock` 变更留在工作区等待审查。

## 12. Codex 审查

> Codex 仅在任务进入 `ready_for_review` 后填写。

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p reader-model`：6/6 通过。
- `cargo test -p epub-core`：30/30 通过。
- `cargo test --workspace`：37/37 通过。
- `npm run check`：通过；14 个前端文件无 Biome 修复，Vitest 2/2 通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| `MAJOR`（已修复） | `crates/epub-core/src/error.rs` / `AppErrorCode` | 这是 SDS 的跨 IPC 应用错误模型，但只实现 14 个 code 中的 9 个，并放在 EPUB 领域 crate 中，造成不完整且错误的模块边界。 | 已删除 `AppErrorCode` 与 `app_error_code()`；`epub-core` 只导出 `EpubErrorCode`。 |
| `MAJOR`（已修复） | `crates/epub-core/src/model.rs` / `EpubWarning.resource_href`；`crates/epub-core/src/error.rs` / `with_location` | 任务要求资源路径或结构位置不得泄漏绝对路径，但两者都是任意 `String`，公开 API 无法执行该约束。 | 警告资源已改为 `Option<ResourcePath>`；`EpubError` 已删除任意字符串 location，只保留 code 和可选 `ResourcePath`。 |
| `MINOR`（已修复） | `crates/epub-core/tests/support/zip_writer.rs` | `entries.len() as u16` 在异常超大夹具下会静默截断，未来超限夹具可能生成失真 ZIP。 | 已在写入前断言条目数不超过 `u16::MAX`，并有 `should_panic` 回归测试。 |

**M1A 结果：**

指纹、模型序列化、路径规范化、错误分类和九类夹具均通过。返工后公开 API 满足安全
类型和模块边界要求，M1A 验收通过。

**返工或后续任务：**

1. 从 `epub-core` 删除 `AppErrorCode` 与 `app_error_code()`；完整 SDS `AppError`
   映射留到后续 Tauri 命令层。
2. 将 `EpubWarning.resource_href` 改为 `Option<ResourcePath>`，并测试绝对路径和
   `..` 路径无法反序列化。
3. 从 `EpubError` 删除 `location` 字段、访问器和 `with_location`；错误只保留
   `EpubErrorCode` 与可选 `ResourcePath`，并用 `META-INF/container.xml` 等
   `ResourcePath` 表示结构位置。
4. 在测试 ZIP writer 写入前断言条目数不超过 `u16::MAX`。
5. 不修改 `ParsedBook` 的 schema version；本次没有持久化数据。
6. 保持夹具类型、camelCase 序列化和现有测试能力不变。
7. 重新运行第 10 节全部命令，更新第 11 节报告并改为 `ready_for_review`。
8. 返工不得创建 commit，不得修改受保护文档。

### 复验记录

**复验日期：** 2026-11-14  
**结论：** 通过

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p reader-model`：6/6 通过。
- `cargo test -p epub-core`：33/33 通过。
- `cargo test --workspace`：40/40 通过。
- `npm run check`：通过，Vitest 2/2。
- `epub-core` 运行时依赖只有 `serde` 和 `thiserror`；`zip`、`tempfile` 与
  `serde_json` 保持在测试依赖中。
- 未发现 `AppErrorCode`、`app_error_code`、`with_location` 或未校验的
  `resource_href: Option<String>` 残留。
- Git 没有新增 commit。
