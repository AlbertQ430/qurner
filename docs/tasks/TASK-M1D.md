# TASK-M1D：流式内容指纹与源文件完整性

**状态：** accepted  
**里程碑：** M1  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** `TASK-M1C` accepted

## 1. 目标

为 EPUB 原始文件提供流式 SHA-256 内容指纹，返回 reader-model 的
`BookFingerprint`，并验证解析前后原始文件内容保持不变。

该能力供 M2 Import Service 计算判重键；不能在内存中读取完整文件。

## 2. 用户授权

用户已授权 M1 开发。本任务允许：

- 新增 SDS 已列出的 `sha2` 运行时依赖。
- 让 `epub-core` 依赖 workspace 中的 `reader-model`。
- 修改 `epub-core` 的错误、公开入口和测试。

不得新增其他运行时依赖。

## 3. 必须完成

### 3.1 公开入口

```rust
pub fn fingerprint_file(path: &Path) -> Result<BookFingerprint, EpubError>;
```

- 以只读方式打开文件。
- 使用固定大小缓冲区流式计算 SHA-256，建议 1 MiB。
- 不一次性 `read_to_end`、`read_to_string` 或映射完整文件。
- 返回小写十六进制可表示的 `BookFingerprint`。
- 不修改文件 inode、内容、修改时间或权限。

### 3.2 错误码

在 `EpubErrorCode` 增加：

- `FileNotFound`：文件不存在。
- 保留 `IoError`：存在但无法读取。

`fingerprint_file` 必须区分 `NotFound` 和其余 I/O 错误。不得泄漏绝对路径或 OS 错误
文本。其他调用方的错误映射保持不变。

### 3.3 大文件行为

- 读取请求每次不超过 1 MiB。
- 对多个 KiB/MiB 输入产生正确摘要。
- 空文件也应得到标准 SHA-256。
- 文件被截断或读取中断时返回 `IoError`。

### 3.4 源文件完整性

使用 `fixtures::large_epub()` 验证：

1. 解析前计算指纹。
2. 调用 `parse_epub`。
3. 再次计算指纹。
4. 两次指纹必须相同。
5. 文件长度与内容必须未变化。

该测试证明解析链路只读；不需要在产品 `parse_epub` 中重复计算两次哈希。

## 4. 不属于本任务

- 指纹数据库、重复判定和软移除恢复。
- fixed-layout、脚本、远程资源或竖排内容扫描。
- ZIP local header/CRC 的额外验证。
- UI、Tauri、SQLite 或导入事务。

## 5. 文件权限

### 允许修改

- `Cargo.toml`
- `Cargo.lock`
- `crates/epub-core/Cargo.toml`
- `crates/epub-core/src/`
- `crates/epub-core/tests/`
- 本任务顶部状态和第 9 节开发报告

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

## 6. 测试要求

- SHA-256 标准测试向量：空输入和 `abc`。
- 内容跨越多个缓冲区时摘要稳定。
- 只读测试读取器断言单次 `read` 请求不超过 1 MiB。
- 文件不存在返回 `FileNotFound`。
- 目录或不可读路径映射为 `IoError`（在测试环境可稳定构造时）。
- `large_epub` 解析前后指纹、长度和内容一致。
- 错误 Display 不包含绝对路径。

## 7. 必跑命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p epub-core
cargo test --workspace
QURNER_TEST_EPUB_DIR=/private/tmp/qurner-m1p/books \
  cargo test -p epub-core --test public_domain -- --ignored --nocapture
npm run check
```

## 8. 必须报告的证据

- 公开 API 和错误码变化。
- 单次最大读取字节断言。
- 标准 SHA-256 测试向量。
- 100 MB 文件解析前后指纹与内容一致性。
- 新增依赖和构建结果。

## 9. DeepSeek 开发报告

> DeepSeek 开始后把顶部状态改为 `in_progress`，完成后填写本节并改为
> `ready_for_review`。

**完成摘要：**

新增 `epub-core::fingerprint_file(path: &Path) -> Result<BookFingerprint, EpubError>`，
以只读方式打开原始文件，使用固定 1 MiB 缓冲区流式计算 SHA-256，返回 reader-model 的
`BookFingerprint`；不读取完整文件、不映射文件、不修改 inode/内容/修改时间/权限。

- 错误码：`EpubErrorCode` 新增 `FileNotFound`（`fileNotFound` / “书籍文件不存在”）；
  文件不存在返回 `FileNotFound`，目录、权限或读取中断返回 `IoError`；错误不包含绝对
  路径或 OS 错误文本。
- `epub-core` 新增对 workspace `reader-model` 和 `sha2 0.10` 的运行时依赖。
- 测试覆盖标准 SHA-256 向量、跨缓冲区一致性、单次读取上限、文件不存在/目录/读取
  失败、以及 100 MB EPUB 解析前后指纹与字节完全一致。

**修改文件：**

- `Cargo.toml`、`Cargo.lock`（sha2 workspace 依赖）
- `crates/epub-core/Cargo.toml`（reader-model、sha2）
- `crates/epub-core/src/lib.rs`（导出 `fingerprint_file`）
- `crates/epub-core/src/error.rs`（新增 `FileNotFound`）
- `crates/epub-core/src/fingerprint.rs`（新增）
- `crates/epub-core/tests/fingerprint.rs`（新增）
- `docs/tasks/TASK-M1D.md`（顶部状态与本第 9 节）

未修改 `src/`、`src-tauri/`、`tests/e2e/`、`crates/reader-model/` 和受保护文档，
未创建 commit。

**API 与错误码：**

```rust
pub fn fingerprint_file(path: &Path) -> Result<BookFingerprint, EpubError>;
```

- 读取缓冲：`BUFFER_SIZE = 1024 * 1024`（1 MiB），每次 `Read::read` 请求不超过该值。
- 摘要：`sha2::Sha256` 流式 `update`，`BookFingerprint::from_sha256` 接收 32 字节摘要；
  `Display` 输出小写 64 字符。
- 错误：`FileNotFound` 仅用于 `io::ErrorKind::NotFound`；其他 open/read 错误统一
  `IoError`。既有调用方（parse_epub、precheck、encryption）的错误映射未改变。
- `EpubError` 仍然只保存 code + 可选 `ResourcePath`，不保存路径字符串或 OS 错误。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p epub-core` | 0 | 84 passed，1 ignored（lib 31、error 5、fingerprint 4、fixtures 13、model 7、parse_epub 10、toc_cover_weights 14、public_domain 1 ignored） |
| `cargo test --workspace` | 0 | 91 passed，0 failed，1 ignored |
| `QURNER_TEST_EPUB_DIR=<临时目录> cargo test -p epub-core --test public_domain -- --ignored --nocapture` | 0 | 4 本公有领域 EPUB 全部通过 |
| `npm run check` | 0 | Biome 14 个文件无修复；Vitest 2/2 通过 |

依赖：`sha2 0.10.9` 已在锁文件中，`Cargo.lock` 仍为 457 个包；`cargo tree -p epub-core`
从 92 增至 103 个唯一包（新增 sha2 及其传递依赖）。

**指纹与完整性验证：**

- 标准向量：
  - 空输入：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
  - `abc`：`ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`
  - 通过内存 `Cursor` 与真实文件两种入口验证。
- 跨缓冲区：2 MiB + 123 字节输入，流式结果与一次性 `Sha256` 摘要一致。
- 单次读取上限：3 MiB + 17 字节输入，记录读取器断言 `max_request <= 1 MiB`，
  共多次读取。
- 错误：
  - 不存在的临时路径 → `FileNotFound`，Display 不含该路径；
  - 临时目录（macOS 上 open 成功、read 返回 EISDIR）→ `IoError`；
  - 模拟读取失败 → `IoError`。
- 100 MB 完整性（`fixtures::large_epub()`，104,859,703 字节）：
  - 解析前指纹 == 解析后指纹；
  - 解析前后文件长度相同；
  - 解析前后完整字节逐字节相等；
  - 测试实际运行约 11.7 s（含两次 100 MB 指纹与读取）。
- 空文件与 `abc` 文件指纹与标准向量一致。

**未验证项：**

- 未测试权限拒绝（`chmod 000`）在 CI/沙箱环境下的稳定行为；目录错误已覆盖。
- 未测试指纹计算过程中文件被外部并发截断/替换的竞态；仅模拟读取错误。
- 未测试超过 4 GB 的大文件、稀疏文件或网络/外接卷。
- 指纹数据库、判重和软移除恢复不在本任务。
- 100 MB 测试使用合成夹具，未对真实 100 MB EPUB 计算。

**风险与待决问题：**

- 每次调用分配一个 1 MiB 缓冲区；M2 批量导入时按文件顺序调用，内存峰值可控。
- 指纹在阻塞线程执行；M2 Import Service 必须调度到阻塞线程，避免阻塞 UI。
- 指纹计算与 `parse_epub` 之间若文件被外部修改，指纹与解析内容可能不一致；
  M2 导入流程需要定义读取顺序或二次校验策略。
- `File::open` 对目录的行为依赖平台（macOS 已在测试中映射为 `IoError`），
  其他平台需在对应 CI 覆盖。
- 新增直接依赖 `sha2 0.10`（crypto 库，Apache-2.0/MIT），升级需重跑标准向量。

## 10. Codex 审查

> Codex 仅在任务进入 `ready_for_review` 后填写。

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p epub-core`：84 passed，1 ignored。
- `cargo test --workspace`：91 passed，1 ignored。
- 标准空输入/`abc` SHA-256 向量、跨缓冲、单次 1 MiB 上限、文件不存在和读取失败
  测试全部通过。
- 100 MB EPUB 解析前后指纹、长度和完整字节一致。
- 4 本公有领域 EPUB 专项测试通过。
- `npm run check`：通过，Vitest 2/2。
- 新增运行时依赖只有 `reader-model` 与 `sha2 0.10.9`。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|

**M1D 结果：**

流式指纹、错误分类和源文件完整性均满足任务要求，M1D 验收通过。

**返工或后续任务：**

无。并发修改竞态由 M2 Import Service 的读取顺序或二次校验处理。
