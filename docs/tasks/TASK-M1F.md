# TASK-M1F：M1 集成、性能与真实 EPUB 验收

**状态：** accepted  
**里程碑：** M1  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** `TASK-M1E` accepted

## 1. 目标

完成 EPUB Core 的集成收口：

- 提供可复现的只读 CLI 示例。
- 验证 EPUB `mimetype` 基础合规。
- 在 100 MB EPUB 上测量解析耗时和峰值内存。
- 用合成夹具和公有领域 EPUB 完成 M1 最终验收。

M1F 通过后，M1 里程碑才算完成。

## 2. 用户授权

用户已授权 M1。本任务不新增运行时依赖。

允许使用现有合成大文件和 `/private/tmp/qurner-m1p/books/` 公有领域样本做性能/兼容
测试，但不得提交书籍、测量产物或缓存。

## 3. 只读检查示例

新增 `epub-core/examples/inspect_epub.rs`：

```bash
cargo run -p epub-core --release --example inspect_epub -- <path>
```

输出一行或结构化 JSON，包含：

- `BookFingerprint`。
- EPUB version（如可从现有模型/适配层获得；否则省略）。
- 书名、作者数量。
- manifest、spine、TOC、cover 是否存在。
- `chapter_weights.len()`。
- warning code 列表。

禁止输出：

- 用户绝对路径。
- 正文、封面字节。
- 远程 URL。
- rbook/zip/quick-xml/lol_html 内部类型。

不存在或无效文件以稳定 `EpubErrorCode` 退出非零状态。

## 4. mimetype 基础校验

在现有流式预检中补充：

- ZIP 第一个条目必须是 `mimetype`。
- 该条目必须使用 Stored 压缩。
- 内容必须恰好为 `application/epub+zip`。
- local header 的偏移必须为 0。
- 只读取 local header 和 20 字节 mimetype 内容，不读取归档数据区。

失败统一返回 `InvalidZip`；错误不包含用户路径。

增加合法、缺失、错误内容、压缩方式错误、非首条目五类测试。

## 5. 性能与资源基线

参考设备：当前 Apple M5、16 GB、macOS 26。

使用现有约 100 MB 合成 EPUB，release 构建，运行示例两次并记录第二次：

- 从进程启动到输出 summary 的 wall time。
- `/usr/bin/time -l` maximum resident set size。
- 解析前后 SHA-256 一致。

门槛：

- wall time ≤ 10 秒。
- 峰值 RSS ≤ 512 MB。
- 不产生网络连接。

若超出门槛，停止并报告，不通过优化删除安全检查。

## 6. 集成验收

必须验证：

1. 6 个预期成功合成夹具完成完整解析。
2. 缺 OPF、缺 container、危险路径、重复条目、ZIP64、加密正文、固定版式、竖排、
   脚本和缺失 manifest 内容资源均返回预期错误。
3. 远程资源只产生 warning，不访问网络。
4. 4 本公有领域 EPUB（3 EPUB2 + 1 EPUB3）完整解析。
5. 解析前后原始文件指纹和内容不变。
6. 公开 JSON 不包含绝对路径、远程 URL 或第三方类型。
7. `cargo tree` 无 HTTP/网络运行时依赖。

## 7. 不属于本任务

- UI、Tauri、SQLite、Import Service 和书库。
- EPUBCheck/Java 集成。
- 新格式或新功能。
- CSP/iframe 阅读器。

## 8. 文件权限

### 允许修改

- `crates/epub-core/src/precheck.rs`
- `crates/epub-core/src/`
- `crates/epub-core/examples/`
- `crates/epub-core/tests/`
- 本任务顶部状态和第 10 节开发报告

### 禁止修改

- `Cargo.toml`
- `Cargo.lock`
- 其他 crate 的 `Cargo.toml`
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

## 9. 必跑命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p epub-core
cargo test --workspace
QURNER_TEST_EPUB_DIR=/private/tmp/qurner-m1p/books \
  cargo test -p epub-core --test public_domain -- --ignored --nocapture
npm run check
cargo build --release -p epub-core --example inspect_epub
/usr/bin/time -l cargo run -p epub-core --release --example inspect_epub -- \
  /private/tmp/qurner-m1p/large.epub
```

## 10. DeepSeek 开发报告

> DeepSeek 开始后把顶部状态改为 `in_progress`，完成后填写本节并改为
> `ready_for_review`。

**完成摘要：**

完成 EPUB Core 的 M1 集成收口：

- 新增只读示例 `epub-core/examples/inspect_epub.rs`，输出一行 JSON 摘要；失败只输出
  稳定 `EpubErrorCode` 并退出非零状态。
- 流式 ZIP 预检补充 `mimetype` 基础合规：必须为首条目、Stored、内容为
  `application/epub+zip`、local header 偏移为 0；只读取 local header 与 20 字节内容。
- 新增 `tests/m1_acceptance.rs` 汇总 M1 验收：6 个成功夹具、10 类错误场景、远程资源
  warning、公开 JSON 无敏感内容、源文件解析前后不变。
- 使用约 100 MB 合成 EPUB 在 release 模式测量两次耗时与峰值 RSS，并验证 SHA-256
  前后一致。
- 4 本公有领域 EPUB（3 EPUB2 + 1 EPUB3）与全部既有测试通过；`cargo tree` 无 HTTP/
  网络运行时依赖。

**修改文件：**

- `crates/epub-core/src/precheck.rs`（mimetype 校验、测试 ZIP builder 与回归测试）
- `crates/epub-core/examples/inspect_epub.rs`（新增）
- `crates/epub-core/tests/m1_acceptance.rs`（新增）
- `docs/tasks/TASK-M1F.md`（顶部状态与本第 10 节）

未修改 `Cargo.toml`、`Cargo.lock`、其他 crate 的 `Cargo.toml`、`src/`、`src-tauri/`、
`tests/e2e/`、`crates/reader-model/` 和受保护文档，未创建 commit。

**mimetype 校验：**

在 `precheck_reader` 中新增（失败统一 `InvalidZip`，不包含用户路径）：

- central directory 第一条必须是 `mimetype`；
- central 记录的压缩方式必须为 Stored、local offset 必须为 0、解压大小必须为 20；
- 读取 local header（30 字节 + 8 字节名称，可跳过 extra）验证签名、Stored、名称和
  20 字节大小；
- 只读取 20 字节 `application/epub+zip` 内容并比较，不读取归档数据区。

回归测试覆盖五类：合法、缺失、错误内容、压缩方式错误、mimetype 非首条目。原有
“预检读取字节 < 100,000”测试仍通过（新增 local header 与 20 字节读取）。

**100 MB 性能：**

测试环境：Apple M5 / 16 GB / macOS 26，合成 `large.epub`（104,859,703 B，含 100 MB
单条目），release 构建，运行两次并记录第二次：

| 指标 | 实测 | 门槛 |
|---|---:|---:|
| wall time（任务命令 `cargo run`） | 0.64 s | ≤ 10 s |
| wall time（直接运行 release 示例二进制） | 0.37 s | ≤ 10 s |
| maximum RSS（cargo run） | 3,325,952 B（约 3.17 MiB） | ≤ 512 MB |
| maximum RSS（示例二进制） | 3,293,184 B（约 3.14 MiB） | ≤ 512 MB |
| 解析前后 SHA-256 | `19682d92e4cb03d940e5b0a5ba04337c1dbc0c8f9dab4251001c5328e88473d9` 一致 | 一致 |

`/usr/bin/time -l` 的 `peak memory footprint` 分别为 2,179,384 B（cargo run）与
2,146,616 B（示例二进制）。示例同时计算 100 MB 文件指纹并完整解析，未观察到网络访问。

**合成与真实 EPUB 验收：**

- 6 个预期成功夹具（EPUB2/EPUB3/嵌套 TOC/缺失元数据/CSS+图片/双语）全部完整解析：
  manifest、spine、TOC 非空，`chapter_weights.len() == spine.len()` 且权重全为正。
- 错误场景（`tests/m1_acceptance.rs`）：
  - 缺 OPF → `MissingOpf`；
  - 缺 container → `MissingContainer`；
  - 危险路径 → `UnsafeResourcePath`；
  - 重复条目 → `InvalidZip`；
  - ZIP64 → `LimitExceeded`；
  - 加密正文 → `EncryptedContent`；
  - 固定版式 → `FixedLayout`；
  - 竖排 → `VerticalWriting`；
  - 脚本 → `ScriptDependency`；
  - manifest 缺失 CSS → `InvalidOpf`。
- 远程 `<img src="https://...">` 仅产生 `RemoteResourceBlocked` warning，解析成功，
  不访问网络。
- 公开 JSON（`ParsedBook` 序列化）不包含 `/Users/`、`http://`、`https://`、`rbook`、
  `ZipArchive`、`lol_html`、`quick_xml`。
- 源文件完整性：解析前后指纹与完整字节一致（`m1_acceptance` 小夹具；
  M1D 已有 100 MB 指纹/字节一致性测试）。
- 4 本公有领域 EPUB 通过环境变量专项测试（spine 14/17/32/31，TOC 16/63/32/32，
  cover 均为 true，权重全部非零）。
- `cargo tree -p epub-core` 未发现 `reqwest`/`hyper`/`ureq`/`curl`/`tokio` 等网络依赖。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p epub-core` | 0 | 122 passed，1 ignored（lib 42、content_safety 21、error 5、fingerprint 4、fixtures 13、m1_acceptance 6、model 7、parse_epub 10、toc_cover_weights 14、public_domain 1 ignored） |
| `cargo test --workspace` | 0 | 129 passed，0 failed，1 ignored |
| `QURNER_TEST_EPUB_DIR=<临时目录> cargo test -p epub-core --test public_domain -- --ignored --nocapture` | 0 | 4 本公有领域 EPUB 全部通过 |
| `npm run check` | 0 | Biome 14 个文件无修复；Vitest 2/2 通过 |
| `cargo build --release -p epub-core --example inspect_epub` | 0 | release 示例构建成功 |
| `/usr/bin/time -l cargo run -p epub-core --release --example inspect_epub -- <large.epub>` | 0 | 0.64 s，maximum RSS 3,325,952 B，SHA 一致 |

**未验证项：**

- 100 MB 性能使用合成 EPUB，并非真实 100 MB 书籍；仅在当前 Apple M5 参考设备测量。
- `cargo run` 的 wall time 与 RSS 包含 cargo 自身开销；已额外记录直接运行示例二进制的
  更准确数据。
- 未用网络抓包或 `dtrace` 验证零网络；以依赖树、代码审计和静态测试替代。
- 未运行 EPUBCheck（无 Java）。
- mimetype 校验不验证 CRC 或数据区其他条目；仅验证首条目 local header 和 20 字节内容。
- 公有领域样本仍为 4 本 Gutenberg 书（3 EPUB2 + 1 EPUB3），未覆盖更多出版社或 DRM/
  固定版式书籍。
- 示例使用已有的 dev-dependency `serde_json` 输出 JSON；未新增任何依赖，示例不进入
  产品运行时。

**风险与待决问题：**

- 严格 mimetype 顺序/Stored/offset 校验会拒绝不符合 OCF 规范的 EPUB（任务要求）。
- 示例的 JSON 字段名与 `ParsedBook` camelCase 一致，但它是示例接口，不承诺作为稳定
  CLI 契约；后续如需正式 CLI，应新增任务定义。
- 性能数据只代表合成 100 MB 文件和当前机器；真实书籍的 TOC/文本解析可能更慢，仍需
  M2/M6 使用真实书籍复核。
- 大量小文件的书（例如 32 章）会重复读取章节用于安全扫描与权重；M2 导入流程可考虑
  合并读取。
- 错误码与 warning 不包含路径/URL，但示例成功输出包含书名等元数据；如未来进入日志
  需按 SDS 的隐私规则处理。

## 11. Codex 审查

> Codex 仅在任务进入 `ready_for_review` 后填写。

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p epub-core`：122 passed，1 ignored。
- `cargo test --workspace`：129 passed，1 ignored。
- 4 本公有领域 EPUB 专项测试通过。
- 合法、缺失、错误内容、压缩方式错误和非首条目 mimetype 测试全部通过。
- release 示例直接运行 100 MB 文件：0.71 s，maximum RSS 3,325,952 B。
- 示例输出不含绝对路径、远程 URL 或第三方内部类型。
- `npm run check`：通过，Vitest 2/2。
- `cargo tree -p epub-core` 无 HTTP/网络运行时依赖。
- 没有新增 commit。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|

**M1 结果：**

M1 EPUB Core 已完成集成、兼容性、安全、性能和真实书籍验收。M1 里程碑通过。

**返工或后续任务：**

无阻塞项。真实 100 MB 书籍、更多出版社样本、EPUBCheck 和完整 CRC/local header
校验留待后续里程碑或专项任务。
