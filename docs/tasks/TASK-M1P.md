# TASK-M1P：EPUB 解析器复用 spike

**状态：** accepted  
**里程碑：** M1  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** `TASK-M1A` accepted  

## 1. 目标

在 1 个工程日内验证 Rust `rbook` 是否能安全、稳定地承担 Qurner M1 的 EPUB allowlist
解析，并用证据决定 M1B 使用薄适配层还是自研 ZIP/XML 解析器。

该任务只做实验，不把 `rbook` 或其依赖加入 Qurner 产品 workspace。

## 2. 用户授权

用户在 2026-11-14 同意开展 GitHub 工具调研和 M1。该授权允许 DeepSeek：

- 在 `/private/tmp/qurner-m1p` 创建一次性 Rust 实验工程。
- 从 crates.io 下载 `rbook 0.7.13` 及其依赖。
- 下载 3 本公有领域 EPUB 测试文件到临时目录。
- 读取现有 Qurner 合成夹具源码，并复制到临时工程复用。

不得把实验依赖加入 Qurner 的 `Cargo.toml` 或 `Cargo.lock`，不得把下载书籍或实验
产物写入仓库。

## 3. 必须回答的问题

1. `rbook` 能否解析全部 7 个结构有效的现有合成夹具？
2. 书名、作者、manifest、spine 和嵌套 TOC 是否完整、顺序稳定？
3. 是否按需读取资源，而不是在打开时加载整个 100 MB EPUB？
4. 是否可在外层完成重复条目、不安全路径、条目数和解压大小预检？
5. 错误能否稳定映射到 `EpubErrorCode`，且不泄漏绝对路径？
6. 能否读取至少 3 本公有领域 EPUB，记录失败和兼容性差异？
7. 增加该依赖后的依赖数量、编译时间和二进制体积影响是否可接受？

## 4. 实验方法

### 4.1 临时工程

- 在 `/private/tmp/qurner-m1p` 创建独立 Cargo 工程。
- 依赖固定为 `rbook = "0.7.13"`，不启用默认不需要的写功能。
- 复用 `crates/epub-core/tests/support/` 中的 9 类夹具生成器。
- 不修改 Qurner 产品 workspace。

### 4.2 合成夹具

对 `FixtureKind::ALL` 运行：

- 7 个结构有效夹具必须尝试解析。
- 2 个损坏/恶意夹具必须验证外层安全预检能在进入第三方解析前拒绝或标记。
- 记录解析成功、失败 code、metadata、spine 数量、TOC 层级和资源读取结果。

### 4.3 公有领域书籍

尝试获取 3 本公开 EPUB，例如 Project Gutenberg 的公开书目。不得使用用户本地书籍。
记录：

- 书名、作者。
- spine 数量和 TOC 数量。
- 是否出现 panic、错误或不支持提示。
- 打开耗时和峰值内存。

网络失败时更换公开来源，不用私人文件替代。

### 4.4 100 MB 与按需读取

- 在 `/private/tmp` 生成一个约 100 MB 的有效 EPUB。
- 使用 `/usr/bin/time -l` 或等价工具记录打开、metadata/TOC 解析与单章读取的峰值 RSS。
- 对比“只打开清单”和“读取一章”的峰值内存，确认没有整本加载。

### 4.5 外层安全预检

在临时工程中验证以下检查可以独立于第三方解析器完成：

- 读取 ZIP central directory 并检测重复名称。
- 拒绝 `..`、绝对路径、反斜杠和 Windows 盘符条目。
- 检查条目数量、单条目解压大小和总解压大小。
- 检查 `META-INF/encryption.xml` 是否加密正文/图片/CSS，而不仅是字体。

可复用 `zip` crate、现有 `ResourcePath` 和 M1A fixture writer；此实验不承诺最终
实现代码进入产品。

## 5. 决策标准

建议采用 `rbook` 必须同时满足：

- 7 个有效合成夹具无 panic，metadata/spine/TOC 与预期一致。
- 3 本公有领域 EPUB 至少 2 本完整通过，第 3 本的失败原因可稳定分类。
- 打开 100 MB 文件不将全部资源载入内存，峰值 RSS 不超过 512 MB。
- 外层预检可以补齐安全限制，不需要 fork `rbook`。
- 运行时许可证为 Apache-2.0，无运行时网络访问。
- 依赖和编译时间对桌面应用可接受。

任一条不满足时，建议继续自研最小解析器，并把 `rbook` 仅作为参考实现。

## 6. 交付

只在任务第 9 节填写：

- 实验环境与依赖版本。
- 合成夹具结果表。
- 公有领域 EPUB 结果表。
- 100 MB 峰值内存和耗时。
- 外层安全预检结果。
- `cargo tree` 规模、`cargo build --release` 时间和临时二进制大小。
- 明确建议：采用 `rbook` 适配层，或继续自研。
- 未验证项和风险。

Codex 将根据报告形成 ADR，并决定 M1B 任务。

## 7. 文件权限

### 允许修改

- `/private/tmp/qurner-m1p/`
- 本任务顶部状态
- 本任务第 9 节开发报告

### 禁止修改

- Qurner 产品的全部 `Cargo.toml`、`Cargo.lock`、源码和测试
- `SRS.md`
- `SDS.md`
- `DEVELOPMENT_PLAN.md`
- `AI_COLLABORATION.md`
- `AGENTS.md`
- `docs/decisions/`
- 其他任务文件

## 8. 必跑命令

在 Qurner 仓库运行基线检查：

```bash
cargo test --workspace
npm run check
```

在临时工程记录：

```bash
cargo build --release
/usr/bin/time -l cargo run --release -- <fixture-or-book>
cargo tree
```

实验结束确认 Qurner `git status --short` 没有因 M1P 新增产品依赖或临时书籍。

## 9. DeepSeek 开发报告

> DeepSeek 开始后把顶部状态改为 `in_progress`，完成后填写本节并改为
> `ready_for_review`。

**完成摘要：**

在 `/private/tmp/qurner-m1p` 建立一次性 Cargo 实验工程（Rust 1.100.0，rbook 0.7.13
`default-features = false, features = ["threadsafe"]`，zip 2.6.1 仅用于读取加密清单），
复用 M1A 的 9 类合成夹具生成器。本轮返工把外层预检改为**基于 `File` 的 seek 与受限
读取**，不再用 `std::fs::read` 读取整个文件；补充 1 本 Gutenberg EPUB 3 样本；重新测量
precheck、rbook open、端到端（预检 + rbook + 首章）的峰值 RSS。未修改 Qurner 产品
`Cargo.toml`/`Cargo.lock`、源码或测试，未创建 commit。

预检实现（本轮修改）：
- 只读取文件尾部 `min(file_len, 65_557)` 字节定位 EOCD。
- 按 EOCD 的 central directory 偏移逐条读取 46 字节 header 与条目名；extra/comment
  使用 `SeekFrom::Current` 跳过。
- `META-INF/encryption.xml` 通过 `zip::ZipArchive` 按名单独读取，不读整个归档。
- 检测到 ZIP64（条目数 0xFFFF 或 central 偏移 0xFFFFFFFF）时明确拒绝，避免超限文件
  绕过大小检查。

**实验结果：**

7 个结构有效的合成夹具中，**6 个预期成功、1 个（`missing-opf.epub`）预期失败**；
另有 2 个恶意夹具应被外层预检拒绝。

| 输入 | 预期 | 解析结果 | metadata | spine | TOC | 峰值 RSS | 备注 |
|---|---|---|---|---:|---:|---:|---|
| `minimal-epub2.epub` | 成功 | ok（2.0） | 示例书籍 / 示例作者 | 1 | 2 | — | NCX 解析正常 |
| `minimal-epub3.epub` | 成功 | ok（3.0） | 示例书籍 / 示例作者 | 1 | 2 | — | nav 解析正常 |
| `nested-toc.epub` | 成功 | ok（3.0） | 嵌套目录示例 / 示例作者 | 2 | 5 | — | 两层嵌套正确，depth 2，顺序与 nav 一致 |
| `missing-metadata.epub` | 成功 | ok（3.0） | `None` / `[]` | 1 | 2 | — | 缺失字段返回 `None`，无占位值 |
| `css-and-image.epub` | 成功 | ok（3.0） | 图文混排示例 / 示例作者 | 1 | 2 | — | `cover_image()` 可识别 PNG |
| `bilingual.epub` | 成功 | ok（3.0） | Bilingual Sample / 示例作者 | 2 | 3 | — | 中英文顺序稳定 |
| `missing-opf.epub` | **失败** | error | — | — | — | — | `InvalidResource`（归档内 `/OEBPS/content.opf` 不存在），错误可稳定识别 |
| `unsafe-paths.epub` | 预检拒绝 | rejected | — | — | — | — | 检出 4 条不安全路径 |
| `duplicate-entries.epub` | 预检拒绝 | rejected | — | — | — | — | 检出重复名 `OEBPS/chapter1.xhtml` |

说明：TOC 数量包含 rbook 的根 `contents` 节点（root + 子项），嵌套夹具为
root + 2 个一级 + 2 个二级 = 5；夹具均为进程内小文件，未单独用 `/usr/bin/time` 测量。

**公有领域 EPUB：**

| 来源 | 书名 | 结果 | spine | TOC | 耗时 | 备注 |
|---|---|---|---:|---:|---:|---|
| Project Gutenberg #11 | Alice's Adventures in Wonderland（Lewis Carroll） | ok（2.0） | 14 | 17 | 0.20 s | 峰值 RSS 2,310,144 B |
| Project Gutenberg #1342 | Pride and Prejudice（Jane Austen） | ok（2.0） | 17 | 64 | < 0.01 s | 峰值 RSS 2,998,272 B |
| Project Gutenberg #84（EPUB3） | Frankenstein; or, The modern Prometheus（Mary Wollstonecraft Shelley） | ok（**3.0**） | 32 | 33 | < 0.01 s | **nav = `OEBPS/toc.xhtml`**；峰值 RSS 2,392,064 B |
| Project Gutenberg #84（EPUB2） | Frankenstein; or, The modern Prometheus（Mary Wollstonecraft Shelley） | ok（2.0） | 31 | 33 | < 0.01 s | 峰值 RSS 2,375,680 B |

- 4 本全部通过；EPUB3 样本 OPF `version="3.0"`，manifest 提供
  `<item properties="nav" href="toc.xhtml">`，rbook 正确解析出 nav/TOC（33 项，含 root，
  depth 1，Gutenberg 目录为平铺结构）。
- 未出现 panic、错误或“不支持”提示；样本保存在 `/private/tmp/qurner-m1p/books/`，
  未进入仓库。

**100 MB 与按需读取：**

`large.epub`：104,859,703 B，含 100 MB 单条目（Stored）。三种负载均为独立进程，
用 `/usr/bin/time -l` 测量峰值 RSS：

| 负载 | 峰值 RSS | 结果 |
|---|---:|---|
| `precheck large.epub`（仅外层预检，seek 读取） | **1,818,624 B** | entries=6，total_uncompressed=104,859,021 B，safe=true |
| `open large.epub`（rbook 打开 + metadata/manifest/spine/TOC + 首章） | **2,080,768 B** | 首章 119 B，`elapsed_ms=0` |
| `end2end large.epub`（预检 + rbook + 首章） | **2,195,456 B** | 预检通过后解析成功，`elapsed_ms=0` |

结论：预检与打开均未将 100 MB 条目载入内存，端到端峰值约 2.1 MB，远低于 512 MB 门槛。

**外层安全预检：**

在进入 rbook 前独立完成，无需 fork：

- 基于文件 seek 的 EOCD/central directory 解析：检查条目数、重复名、单条目解压大小、
  总解压大小；不依赖 `zip` crate 的按名去重。
- 路径规则：拒绝绝对路径、反斜杠、Windows 盘符、任一 `..` 段、NUL 和空名。
- 加密：读取 `META-INF/encryption.xml` 的 `CipherReference URI`；字体扩展名
  （`.otf/.ttf/.woff/.woff2`）允许并记录，其他资源拒绝。
- ZIP64 或不完整的 central directory 直接返回错误，不进入 rbook。
- 实测：`unsafe-paths.epub` 检出 4 条不安全路径并拒绝；`duplicate-entries.epub` 检出
  1 个重复名并拒绝；`large.epub` 通过；正文加密样本拒绝 `OEBPS/chapter1.xhtml`；
  字体加密样本允许并记录 `OEBPS/fonts/sample.otf`。

**依赖与构建影响：**

- `cargo tree`：48 个唯一依赖（含实验 crate 自身），70 行树；无 `reqwest`/`hyper`/
  `ureq`/`curl`/`tokio` 等网络依赖。
- 许可证：rbook 0.7.13 为 Apache-2.0。
- Clean release 构建：`cargo clean` 后 `cargo build --release` real 6.40 s。
- 实验二进制：1.3 MB（arm64，含 spike 与夹具代码，不代表最终应用增量）。
- 预检改造没有新增依赖；rbook 自带 zip 9.5.1，与外层预检使用的 zip 2.6.1 相互独立。

**建议：**

**建议采用 `rbook` 薄适配层**，M1B/M1C 不重写 OPF/NCX/nav 解析。6 条决策标准全部满足：

1. 7 个有效夹具中 6 个应成功者全部成功；1 个缺 OPF 的应失败夹具稳定报错。
2. 4 本公有领域 EPUB（3 本 EPUB2 + 1 本 EPUB3）全部完整通过。
3. 100 MB 端到端峰值 RSS 约 2.1 MB，未整本加载。
4. 外层预检（seek + 受限读取）可独立补齐重复条目、路径、大小、ZIP64 和加密限制，
   无需 fork。
5. Apache-2.0，无网络依赖。
6. 依赖 48 个、clean 构建 6.4 s、二进制增量可接受。

适配层要求：
- 预检必须先于 `Epub::open`，并且自身保持流式读取（本次已证明可行）。
- 重复条目不能让 rbook 的内部 zip 读取器静默去重；预检必须先拒绝。
- TOC 需去掉根 `contents` 节点或明确映射为产品 `TocNode` 层级。
- `EbookError`/`ArchiveError`/`EpubError` 变体映射到 `EpubErrorCode`，保留归档内
  `ResourcePath`，不泄漏 `rbook` 类型或用户绝对路径。
- 产品继续使用 M1A 的 `ParsedBook`、`ResourcePath`、`BookFingerprint` 和合成夹具。

**未验证项：**

- 只测量了打开清单与读取小章节，未测量实际读取 100 MB 图片条目的峰值内存。
- EPUB3 只覆盖 1 本 Gutenberg 样本；其他 EPUB3 出版社结构和嵌套 nav 未覆盖。
- 未用真实 DRM/字体加密书籍验证兼容性；加密预检使用合成 `encryption.xml`。
- 未覆盖 ZIP64、超 4 GB、10 万条目边界和损坏 central directory 的实测；
  当前预检会直接拒绝 ZIP64。
- 未运行 EPUBCheck（本机无 Java），未做规范一致性校验。
- 未比较自研解析器的实现成本，仅依据 rbook 结果给出建议。

**风险与待决问题：**

- rbook 0.7.x 尚未 1.0，API 可能变化；采用时需锁定 `=0.7.13` 并由适配层隔离。
- 错误映射并非全部一一对应：`InvalidResource` 既可能表示缺 OPF，也可能表示缺资源，
  适配层需要结合预检和上下文判断，不能只依赖错误文本。
- rbook 的 TOC 根节点语义与 `ParsedBook.toc` 不同，需要归一化并添加测试。
- 100 MB 内存结论只在当前 macOS/arm64 与 release 构建下测得；Debug 构建和真实书籍
  图片未验证。
- 预检拒绝 ZIP64，需在 M1B 明确这是产品限制还是实现 TODO，并在错误码中体现。
- 预检与 rbook 各自携带一套 ZIP 解析实现（zip 2.6.1 / 9.5.1），需保证两者对条目名
  编码和大小字段的解释一致。
- 实验工程与下载书籍仅在 `/private/tmp/qurner-m1p/`，任务结束不进入仓库。

## 10. Codex 审查

> Codex 仅在任务进入 `ready_for_review` 后填写。

**结论：** 通过  
**审查日期：** 2026-11-14

**独立复核：**

- 直接运行实验二进制：6 个应成功的合成夹具均解析成功，缺 OPF 夹具稳定返回
  `InvalidResource`；unsafe paths 和重复条目均被预检拒绝。
- 3 本 Gutenberg EPUB 均可解析，metadata、spine 和 TOC 与报告一致。
- `open large.epub`：maximum resident set size 约 2.1 MB，`rbook` 单独打开不会加载
  100 MB 资源。
- `precheck large.epub`：peak memory footprint 105,972,096 B。预检把整个文件读入
  内存，不满足端到端按需读取。
- Qurner 产品 `Cargo.toml`、`Cargo.lock` 和源码中没有新增 `rbook` 依赖。

**问题：**

| 严重级别 | 位置 | 问题 | 处理 |
|---|---|---|---|
| `MAJOR`（已修复） | `/private/tmp/qurner-m1p/src/main.rs` / `run_precheck` | 首轮预检使用 `std::fs::read` 读取完整文件，100 MB 样本峰值约 106 MB。 | 已改为 seek/受限读取；复验 precheck 约 1.8 MB、rbook open 约 2.1 MB、端到端约 2.2 MB。 |
| `MAJOR`（已修复） | `/private/tmp/qurner-m1p/books/` | 首轮三本真实样本全部是 EPUB2。 | 已增加 Gutenberg EPUB3 样本；独立复验为 version 3.0、spine 32、TOC 33。 |
| `MINOR`（已修复） | 第 9 节“实验结果” | 首轮未明确区分预期成功和预期失败夹具。 | 已明确为 6 个预期成功、1 个缺 OPF 预期失败、2 个恶意输入预检拒绝。 |

**ADR 决策输入：**

采用 `rbook = 0.7.13` 作为 M1 的 EPUB allowlist 解析器，通过薄适配层映射到
`ParsedBook`。安全预检先于 `Epub::open`，并保持 seek/受限读取。

**返工或后续任务：**

1. 只修改 `/private/tmp/qurner-m1p`，将预检改为流式/seek 读取。
2. 用 `/usr/bin/time -l` 分别记录预检、rbook open、端到端流程的真实峰值。
3. 增加至少 1 本公有领域 EPUB 3 样本。
4. 更新第 9 节结果表、未验证项和建议，状态改为 `ready_for_review`。
5. 不修改 Qurner 产品依赖、源码或测试，不创建 commit。

### 复验记录

**复验日期：** 2026-11-14  
**结论：** 通过

- 7 个结构夹具：6 个预期成功全部通过，1 个缺 OPF 稳定返回 `InvalidResource`。
- 恶意路径与重复条目夹具均在进入 rbook 前被拒绝。
- 4 个公有领域样本全部通过：3 本 EPUB2、1 本 EPUB3。
- EPUB3 复验：version 3.0、spine 32、TOC 33、无 panic。
- `precheck large.epub`：maximum resident set size 1,835,008 B。
- `open large.epub`：maximum resident set size 2,097,152 B。
- `end2end large.epub`：maximum resident set size 2,211,840 B。
- Qurner 产品源码和锁文件没有新增 rbook，实验仅存在于 `/private/tmp/qurner-m1p`。

**后续实现约束：**

- M1B 必须复刻 seek/受限读取预检，不得读取完整文件。
- `encryption.xml` 必须使用 XML 解析器处理命名空间、属性顺序和单双引号，不能沿用
  spike 中只匹配 `URI="` 的字符串逻辑。
- TOC 根节点需要归一化。
- `rbook`、`zip`、`quick-xml` 错误必须在适配层转换为 `EpubErrorCode`。
