# TASK-M1B：流式预检与 rbook 适配层

**状态：** accepted  
**里程碑：** M1  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** `TASK-M1A` accepted、`TASK-M1P` accepted

## 1. 目标

为 `epub-core` 增加可安全打开 EPUB 的入口：先通过 seek/受限读取完成 ZIP 安全预检，
再使用固定版本 `rbook 0.7.13` 解析 metadata、manifest 和 spine，并映射为 M1A 的
`ParsedBook`。

本任务不实现目录归一化、封面、章节权重或指纹计算；这些分别留给 M1C/M1D。

## 2. 用户授权

用户已授权 M1 开发。ADR-0002 批准以下产品依赖：

- `rbook = "=0.7.13"`，`default-features = false`，仅启用 `threadsafe`。
- `zip` 从测试依赖提升为运行时依赖，用于独立的 ZIP 预检。
- `quick-xml` 用于安全解析 `META-INF/encryption.xml`。

不应新增其他运行时依赖；如确需新增，停止并报告。

## 3. 必须完成

### 3.1 流式 ZIP 预检

新增只读入口，例如：

```rust
pub fn parse_epub(path: &Path) -> Result<ParsedBook, EpubError>
```

在调用 `rbook::Epub::open` 前完成：

- 使用 `File::seek` 读取文件尾部，定位 EOCD。
- 通过 seek 逐条读取 central directory header 和名称，不读取完整文件。
- 拒绝 ZIP64；错误映射为 `EpubErrorCode::LimitExceeded`。
- 最多 100,000 个条目。
- 单条目解压大小最多 200 MB。
- 总解压大小最多 1 GB。
- 拒绝重复条目名称。
- 每个条目名称必须能通过 `ResourcePath::new`，否则返回
  `EpubErrorCode::UnsafeResourcePath`。
- 检查 central directory 偏移、长度和 header 边界，损坏时返回 `InvalidZip`。

预检 API 应便于测试统计实际读取字节数。

### 3.2 encryption.xml

- 只在 central directory 中存在 `META-INF/encryption.xml` 时读取该条目。
- 使用 `quick-xml`，正确处理命名空间、属性顺序、单引号和双引号。
- 提取 `CipherReference` 的 URI 并规范化为 `ResourcePath`。
- `.otf`、`.ttf`、`.woff`、`.woff2` 字体加密允许继续，并记录
  `EpubWarningCode::EncryptedFontFallback`。
- 加密正文、图片、CSS、目录或其他资源返回 `EncryptedContent`。
- 不能使用字符串搜索 `URI="` 的实现。

### 3.3 rbook 适配

预检通过后：

- 固定调用 `rbook 0.7.13` 打开 EPUB。
- 将书名、作者、语言和出版社映射到 `BookMetadata`。
- 将 manifest 映射到 `BTreeMap<String, ManifestItem>`。
- 将 spine 映射到 `SpineItem`，索引从 0 开始，`linear` 缺省为 true。
- 资源 href 必须解析为归档内 `ResourcePath`。
- `rbook` 类型不得出现在公开 API、错误或持久化 JSON 中。
- 初次 open 的错误按以下规则映射：
  - central directory 缺少 `META-INF/container.xml`：`MissingContainer`。
  - container 存在但 rbook 报 `InvalidResource`：`MissingOpf`。
  - 其他 package 结构错误：`InvalidOpf`。
  - 不得泄漏 `rbook` 错误文本或绝对路径。
- `toc`、`cover` 和 `chapter_weights` 在本任务保持为空；由 M1C 填充。

### 3.4 固定版式、竖排与脚本

M1B 应尽可能从 rbook 的 package/spine metadata 读取并保留这些信息，但完整内容级检测
由 M1D 完成。如果 rbook 已明确提供不支持声明，可返回对应 `EpubErrorCode`；不得执行
脚本或读取远程资源。

## 4. 不属于本任务

- TOC 树归一化。
- 封面选择/缩略图。
- 章节文本权重。
- SHA-256 流式计算。
- HTML/XHTML 内容清洗和脚本扫描。
- Tauri command、SQLite、UI 或阅读器。
- EPUBCheck/Java 集成。

## 5. 公开接口

```rust
pub fn parse_epub(path: &Path) -> Result<ParsedBook, EpubError>;
```

可增加 crate 内部的预检报告类型，但不得公开 `rbook`、`zip` 或 `quick-xml` 类型。

`ParsedBook` schema version 仍为 1。

## 6. 测试要求

- 6 个预期成功的 M1A 合成夹具全部映射成功。
- `missing-opf.epub` 返回稳定 `MissingOpf`，无 panic。
- unsafe paths 和 duplicate entries 在 rbook 前被拒绝。
- 100 MB ZIP 的预检通过计数读取器验证只读取少量头部/目录数据，不读取完整文件。
- 超过单个条目或总解压大小限制时被拒绝。
- ZIP64 被拒绝。
- encryption.xml 使用单引号、双引号、不同命名空间前缀和属性顺序时均能识别。
- 加密字体产生 warning；加密正文拒绝。
- metadata、manifest 和 spine 映射与夹具预期一致。
- camelCase JSON round-trip 继续通过。
- 不产生或记录用户绝对路径。

真实公有领域 EPUB 测试如保留，只能通过环境变量提供路径；不得提交书籍。

## 7. 文件权限

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

## 8. 必跑命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p epub-core
cargo test --workspace
npm run check
```

还应运行 M1P 临时二进制或等价命令，使用 3 本公有领域 EPUB 做一次兼容性冒烟；书籍
路径不得写入仓库。

## 9. 必须报告的证据

- 预检实际读取字节数测试。
- 100 MB 夹具的预检/解析结果。
- ZIP64、重复、路径、大小、加密拒绝结果。
- manifest/spine/metadata 映射表。
- `cargo tree` 中新增运行时依赖。
- 临时工程或真实书籍兼容性冒烟结果。
- 未验证项和风险。

## 10. DeepSeek 开发报告

> DeepSeek 开始后把顶部状态改为 `in_progress`，完成后填写本节并改为
> `ready_for_review`。

**完成摘要：**

在 `epub-core` 增加安全打开入口 `parse_epub(path: &Path) -> Result<ParsedBook, EpubError>`，
固定顺序为：流式 ZIP 安全预检 → `META-INF/encryption.xml` 检查 → `rbook 0.7.13` 解析
→ 映射到 M1A 模型。所有 `rbook`、`zip`、`quick-xml` 类型都限制在 crate 内部，公开 API
只返回 `ParsedBook`、`ResourcePath` 和 `EpubError`。

本轮返工加固了 EOCD 边界，其余实现保持：

- 只接受满足 `eocd_abs + 22 + comment_len == file_len` 的 EOCD 候选；ZIP comment 中的
  伪 EOCD 签名不会被误用。
- 拒绝非零 disk number、非零 central directory disk number、跨磁盘条目计数不一致。
- 要求 `central_offset + central_size == eocd_abs`；截断或偏移错位的 central directory
  返回 `InvalidZip`。
- 预检仍使用 `File::seek` 读取文件尾部 EOCD 和 central directory 头部；拒绝 ZIP64、
  重复条目、不安全路径、超过 100,000 条目、单条目 200 MB 和总解压 1 GB。
- `encryption.xml` 使用 `quick-xml` 解析 `CipherReference` 的 `URI`，支持单/双引号、
  命名空间前缀和任意属性顺序；字体加密记录 `EncryptedFontFallback`，其他资源返回
  `EncryptedContent`。
- `rbook` 映射 metadata、manifest 和 spine；`linear` 缺省 true；RTL spine 返回
  `VerticalWriting`；`toc`、`cover`、`chapter_weights` 保持为空（M1C/M1D）。
- `missing-opf` 映射为 `MissingOpf`；缺 `META-INF/container.xml` 在进入 rbook 前返回
  `MissingContainer`；错误不含 rbook 文本或用户绝对路径。

**修改文件：**

- `Cargo.toml`、`Cargo.lock`（rbook、quick-xml 与运行时 zip 依赖）
- `crates/epub-core/Cargo.toml`
- `crates/epub-core/src/lib.rs`
- `crates/epub-core/src/precheck.rs`（新增）
- `crates/epub-core/src/encryption.rs`（新增）
- `crates/epub-core/src/parse.rs`（新增）
- `crates/epub-core/tests/support/fixtures.rs`（加密与 100 MB 夹具生成器）
- `crates/epub-core/tests/parse_epub.rs`（新增）
- `crates/epub-core/tests/public_domain.rs`（新增，`#[ignore]`，仅环境变量启用）
- `docs/tasks/TASK-M1B.md`（顶部状态与本第 10 节）

未修改 `src/`、`src-tauri/`、`tests/e2e/`、`crates/reader-model/` 和受保护文档，
未创建 commit。

**预检与读取字节：**

- 100 MB 合成归档（`large.epub`，104,859,703 B，含 100 MB 单条目）：
  - 预检读取字节数 **65,670**（尾部 65,557 B + central header/name）；
  - 测试断言 `< 100,000`，未读取归档数据区；
  - `parse_epub` 成功，metadata/manifest/spine 正常。
- 拒绝场景（单元测试与集成测试）：

| 场景 | 结果 |
|---|---|
| ZIP comment 内含伪 EOCD 签名 | 定位真正的 EOCD，正常解析 |
| 非零 disk number / central directory disk number | `InvalidZip` |
| 跨磁盘条目计数不一致 | `InvalidZip` |
| central directory 长度被截断（size - 4） | `InvalidZip` |
| central directory 偏移错位（offset + 4） | `InvalidZip` |
| ZIP64 标记（EOCD 条目数 0xFFFF） | `LimitExceeded` |
| 重复条目名 | `InvalidZip` |
| `../evil.txt`、`/absolute.txt`、`C:\evil.txt`、`OEBPS/../x` | `UnsafeResourcePath` |
| 单条目解压大小 200 MB + 1 | `LimitExceeded` |
| 总解压大小 1.1 GB | `LimitExceeded` |
| 缺 `META-INF/container.xml` | `MissingContainer` |
| 缺 OPF | `MissingOpf` |

**rbook 映射：**

| 夹具 | title | authors | language | publisher | manifest | spine | spine[0].href |
|---|---|---|---|---:|---:|---:|---|
| `minimal-epub2` | 示例书籍 | 示例作者 | zh-CN | — | 2 | 1 | OEBPS/chapter1.xhtml |
| `minimal-epub3` | 示例书籍 | 示例作者 | zh-CN | — | 2 | 1 | OEBPS/chapter1.xhtml |
| `nested-toc` | 嵌套目录示例 | 示例作者 | zh-CN | — | 3 | 2 | OEBPS/chapter1.xhtml |
| `missing-metadata` | — | — | — | — | 2 | 1 | OEBPS/chapter1.xhtml |
| `css-and-image` | 图文混排示例 | 示例作者 | zh-CN | — | 4 | 1 | OEBPS/chapter1.xhtml |
| `bilingual` | Bilingual Sample | 示例作者 | zh-CN | — | 3 | 2 | OEBPS/chapter1.xhtml |

所有成功夹具的 `package_path` 为 `OEBPS/content.opf`，schema version 为 1，spine 索引从
0 连续递增，manifest/spine href 均为不带前导 `/` 的 `ResourcePath`，`linear` 为 true。
`missing-opf.epub` 返回 `MissingOpf`；`unsafe-paths` 和 `duplicate-entries` 在进入
rbook 前被拒绝。

**安全/兼容性：**

- 加密 XML 三种书写风格（双引号、单引号、不同前缀与属性顺序）均识别字体
  `OEBPS/fonts/sample.woff2` 并产生 `EncryptedFontFallback` warning；
  `OEBPS/chapter1.xhtml` 加密返回 `EncryptedContent`，错误只携带归档内相对路径。
- RTL spine（`page-progression-direction="rtl"`）返回 `VerticalWriting`。
- 公有领域 EPUB 冒烟（`QURNER_TEST_EPUB_DIR` 环境变量，路径未写入仓库）：
  Gutenberg #11（EPUB2，spine 14）、#1342（EPUB2，spine 17）、#84（EPUB2，spine 31）、
  #84 EPUB3（spine 32）全部由 `parse_epub` 解析并断言 manifest/spine 非空；
  EOCD 加固后再次通过，说明常见真实 EPUB 满足 central directory 与 EOCD 连续。
- 所有错误 Display 不含 `/Users/`，公开 API/JSON 不出现 `rbook` 类型。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p epub-core` | 0 | 55 passed，1 ignored（lib 20、error 5、fixtures 13、model 7、parse_epub 10、public_domain 1 ignored） |
| `cargo test --workspace` | 0 | 62 passed，0 failed，1 ignored |
| `npm run check` | 0 | Biome 14 个文件无修复；Vitest 2/2 通过 |
| `QURNER_TEST_EPUB_DIR=<临时目录> cargo test -p epub-core --test public_domain -- --ignored` | 0 | 4 本公有领域 EPUB 全部通过 |
| `cargo tree -p epub-core --prefix none \| sort -u \| wc -l` | 0 | 57 个唯一包；`Cargo.lock` 448 个包，未新增依赖 |

**未验证项：**

- 未检测固定版式（`rendition:layout`）：rbook 0.7.13 未暴露该 package 属性。
- 未检测原书脚本依赖（M1D 内容级扫描）。
- 100 MB 峰值 RSS 未在 M1B 重新测量；M1P 已测得预检约 1.8 MB、端到端约 2.2 MB，
  M1B 使用同一 seek 策略。
- 真实 DRM/加密书籍未测试；加密路径使用合成 `encryption.xml`。
- ZIP64 仅用伪造 EOCD 标记验证，没有真实 ZIP64 文件。
- 未运行 EPUBCheck，未做规范一致性校验。

**风险与待决问题：**

- `rbook` 0.7.x 未 1.0；已锁定 `=0.7.13`，升级必须更新 ADR 并重跑兼容性与安全测试。
- 严格 `central_offset + central_size == eocd_abs` 会拒绝在 central directory 与 EOCD
  之间包含 archive extra data record 或数字签名的 ZIP；当前 Gutenberg 样本均通过，
  若将来遇到合法但带额外记录的书籍，需要新增 ADR 决定放宽规则。
- 依赖树同时存在预检用 `zip 2.6.1` 和 rbook 内部的 `zip 9.5.1`；两者对条目名编码
  和大小字段的解释需保持一致。
- OCF 对 `CipherReference URI` 的相对基准存在歧义；当前按归档根解析并去除前导 `/`，
  需要用真实加密 EPUB 样本在 M1D 复核。
- `InvalidResource → MissingOpf` 只在 `Epub::open` 阶段映射；打开后读取资源失败需要
  M1C/M1D 区分“缺资源”和“缺 OPF”。
- 预检只校验 central directory，不校验 local header、CRC 或重复 local 数据；这些由
  rbook 读取阶段和后续内容级检测处理。
- 新增运行时依赖：`rbook 0.7.13`（Apache-2.0）、`quick-xml 0.42.0`、`zip 2.6.1`。

## 11. Codex 审查

> Codex 仅在任务进入 `ready_for_review` 后填写。

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p epub-core`：53/53 通过。
- `cargo test --workspace`：60/60 通过。
- `QURNER_TEST_EPUB_DIR=/private/tmp/qurner-m1p/books cargo test -p epub-core --test public_domain -- --nocapture`：4 本公有领域 EPUB 全部通过。
- `npm run check`：通过，Vitest 2/2。
- 运行时依赖树只包含 ADR 批准的 `rbook 0.7.13`、`quick-xml 0.42` 和
  `zip 2.6.1` 新增链。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| `MAJOR`（已修复） | `crates/epub-core/src/precheck.rs` / EOCD 扫描 | 仅从尾部反向查找最后一个 `PK\x05\x06`，没有验证候选 EOCD 的 comment length 是否使其恰好结束于文件末尾，也没有拒绝多磁盘或 central directory 与实际 EOCD 不连续的异常结构。 | 已实现 EOCD 结束位置、disk number、跨磁盘计数和 central directory 连续性校验，并增加伪 EOCD、多磁盘、截断/错位测试。 |
| `MINOR`（已修复） | `crates/epub-core/tests/public_domain.rs` | 未设置 `QURNER_TEST_EPUB_DIR` 时测试直接返回并显示为 passed。 | 已使用 `#[ignore = "requires QURNER_TEST_EPUB_DIR"]`；普通检查显示 1 ignored，专项测试使用 `--ignored` 通过。 |

**M1B 结果：**

流式读取、大小/重复/路径检查、加密 XML、rbook 映射、EOCD 边界和真实书籍兼容性
均通过，M1B 验收通过。

**返工或后续任务：**

1. 修正 EOCD 候选验证和 ZIP 多磁盘/central directory 连续性检查。
2. 添加伪 EOCD-in-comment、多磁盘标记、central directory 截断/偏移错位测试。
3. 将公有领域测试改为显式 ignored，并修正报告中的测试计数。
4. 重新运行第 8 节全部命令、真实书籍专项测试和依赖检查。
5. 不修改受保护文档，不创建 commit，完成后改为 `ready_for_review`。

### 复验记录

**复验日期：** 2026-11-14  
**结论：** 通过

- `cargo test -p epub-core`：55 passed，1 ignored；伪 EOCD、多磁盘、目录截断/错位
  测试全部通过。
- `cargo test --workspace`：62 passed，1 ignored。
- `QURNER_TEST_EPUB_DIR=/private/tmp/qurner-m1p/books cargo test -p epub-core --test public_domain -- --ignored --nocapture`：通过。
- 100 MB 流式预检实际读取 65,670 字节，未读取数据区。
- `npm run check`：通过，Vitest 2/2。
- 运行时依赖符合 ADR-0002；没有新增 commit。
