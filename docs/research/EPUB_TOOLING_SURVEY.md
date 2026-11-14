# EPUB 工具调研

**日期：** 2026-11-14  
**状态：** 候选调研，尚未调整 SDS  
**目标：** 判断 M1 解析、测试校验和 M3 渲染是否有值得复用的成熟项目

## 结论

可以且应该复用成熟工具，但不应直接替换当前架构。

建议顺序：

1. M1 保留现有安全路径、领域模型和合成夹具基础。
2. 在 M1B 前执行一次限时 spike，对比“Rust `rbook` 适配层”与“自研 ZIP/XML
   解析器”。
3. 将 W3C `EPUBCheck` 作为测试期规范校验器，而不是产品运行时依赖。
4. M3 再评估 `foliate-js` 或 `epub.js` 的分页、CFI 和位置恢复能力。
5. 不引入 GPL 解析库，不把整个 EPUB 加载到 JavaScript 内存。

## 解析候选

| 项目 | 许可证 | 最近活动 | 优点 | 限制 | 建议 |
|---|---|---|---|---|---|
| [DevinSterling/rbook](https://github.com/DevinSterling/rbook) | Apache-2.0 | 活跃 | EPUB2/3、metadata、manifest、spine、TOC；资源按需读取；Rust 1.88+；0.7.13 | 尚未 1.0；需要外层 ZIP 限额、重复条目和兼容性检测；不是最终应用模型 | **首选 spike** |
| [redvelo/epub-stack](https://github.com/redvelo/epub-stack) | Apache-2.0 OR MIT | 活跃，0.3.0 | 类型化模型、CFI、注释、ResourceProvider、内容分析 | 项目采用度很低；依赖较重；0.3 API 不稳定 | 仅作为 CFI/定位参考 |
| [h0rv/epub-stream](https://github.com/h0rv/epub-stream) | MIT | 2026 年活跃，0.1.0 | 流式、显式 ZIP limits、低内存、no_std | 早期版本；ZIP64 明确不支持；嵌入式布局目标与 WKWebView 不同 | 观察，不作为 MVP 基础 |
| [NellowTCS/lexepub](https://github.com/NellowTCS/lexepub) | Apache-2.0 | 2026 年活跃，0.1.2 | 流式与低内存 | API 和生态非常早期 | 观察 |
| [danigm/epub-rs](https://github.com/danigm/epub-rs) | GPL-3.0-or-later | 较旧 | 有现成 EPUB API | 许可证不适合当前未授权公开源码的产品策略 | 不采用 |
| [zhangwfjh/epub-parser](https://github.com/zhangwfjh/epub-parser) | MIT | 低采用度 | metadata、TOC、cover、text | 维护和测试证据不足 | 不采用 |

### rbook 评估重点

- 是否能在我们的 9 类合成夹具上给出稳定的 metadata、manifest、spine 和 TOC。
- 是否只读取当前资源，不把 100 MB EPUB 整体载入内存。
- 能否在外层先完成 ZIP 条目数、大小、路径和重复名称检查，再交给 rbook。
- 能否把错误转换为稳定的 `EpubErrorCode`，不把第三方错误类型泄漏到公开 API。
- 是否支持常见 EPUB2/EPUB3 真实书籍；需要公有领域样本，不使用私人书籍。

## 测试与规范校验

| 项目 | 许可证 | 用途 | 建议 |
|---|---|---|---|
| [W3C EPUBCheck](https://github.com/w3c/epubcheck) | BSD-3-Clause | EPUB 规范一致性校验 | 对有效夹具和真实样本做测试期校验；当前机器没有 Java，需用户批准安装 JRE 或改用 CI |
| [lise-henry/epub-builder](https://github.com/lise-henry/epub-builder) | MPL-2.0 | 生成标准 EPUB | 可用于生成额外有效夹具，但不能生成重复条目和恶意路径；现有 Rust fixture writer 仍需保留 |

`epub-builder` 不能替代当前夹具工厂。我们的测试必须同时包含“规范有效”和“故意损坏/恶意”的两类文件。

## 渲染候选

| 项目 | 许可证 | 优点 | 风险 | 建议 |
|---|---|---|---|---|
| [johnfactotum/foliate-js](https://github.com/johnfactotum/foliate-js) | MIT | 模块化、浏览器渲染、分页与滚动、CFI、Range 锚定、无需整本加载 | 项目明确声明 API 不稳定；它警告 WebKit 的 iframe sandbox 限制，必须依赖 CSP | M3 优先 spike |
| [futurepress/epub.js](https://github.com/futurepress/epub.js) | Free BSD | 成熟度高，分页、连续滚动、CFI | 通常依赖 JSZip 封装整本归档；版本和 API 较旧 | M3 对照参考 |
| [readium/swift-toolkit](https://github.com/readium/swift-toolkit) | BSD-3-Clause | 原生阅读框架成熟 | 会要求放弃当前 Tauri/Rust 主体，迁移成本过高 | 不采用 |

`foliate-js` 和 `epub.js` 都不能替代 M3 的安全设计。无论采用哪一个，仍需要：

- 受控资源协议。
- CSP nonce 与脚本阻断。
- 远程资源阻断。
- 应用侧 Locator 持久化。
- 目录、进度和书签与 Qurner 数据模式映射。

## 推荐决策

在 M1B 前新增 `TASK-M1P` 限时解析 spike：

1. 使用 `rbook` 在临时工程或 feature branch 中读取现有有效夹具和 3 本公有领域 EPUB。
2. 在外层加入 ZIP central directory 预检，验证重复条目、路径、大小和条目数。
3. 将结果映射到当前 `ParsedBook` 模型。
4. 记录启动、解析耗时、内存、TOC 一致性和失败样本。
5. 形成 ADR：
   - 若 `rbook` 通过，M1B/M1C 采用薄适配层，不重写 OPF/NCX/nav。
   - 若 `rbook` 无法满足安全或兼容要求，继续自研最小解析器。

该 spike 必须保持产品运行时不访问网络，第三方依赖只通过锁文件打包。

## 首轮 Spike 发现

`rbook` 对现有合成夹具和 3 本 Gutenberg EPUB2 的解析结果良好，单独打开 100 MB
文件时峰值内存约 2 MB。但首轮外层预检把整个 ZIP 读入内存，实测峰值约 106 MB，
不能作为最终方案。

M1P 返工必须证明安全预检只读取 EOCD、central directory 和必要的
`encryption.xml`，并增加至少 1 本公有领域 EPUB3 样本。通过后才能形成采用
`rbook` 的 ADR。

## 调研来源

- GitHub API 与仓库 README/Cargo.toml，查询日期 2026-11-14。
- crates.io `cargo info`：`epub` 2.1.6、`rbook` 0.7.13、`epub-builder` 0.8.3、
  `epub-stream` 0.1.0、`lexepub` 0.1.2、`epub-parser` 0.3.4。
- W3C EPUBCheck 最新 release `v5.4.0`，发布于 2026-09-15。
