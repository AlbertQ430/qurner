# ADR-0002：EPUB 解析器路线

**状态：** 已接受  
**日期：** 2026-11-14  
**决策人：** Codex，依据 M1P 实验并由用户持续授权 M1  
**相关任务：** `TASK-M1P`、`TASK-M1B`

## 背景

M1 需要解析 EPUB2/3 的 container、OPF、metadata、manifest、spine、NCX 和 nav，同时
要求：

- 不修改原始文件。
- 不执行脚本或访问网络。
- 重复条目、不安全路径、解压大小和加密资源在进入解析器前被拒绝。
- 100 MB EPUB 不被整体读入内存。
- 错误和路径不泄漏第三方类型或用户绝对路径。

`TASK-M1P` 对比现有工具后，验证 `rbook 0.7.13` 可作为 EPUB 语义解析器。

## 决策

采用以下组合：

1. `epub-core` 内部使用固定版本：

   ```toml
   rbook = { version = "=0.7.13", default-features = false, features = ["threadsafe"] }
   ```

2. 在 `Epub::open` 前执行独立 ZIP 安全预检：
   - 通过 seek 定位 EOCD 和 central directory。
   - 不读取完整归档。
   - 拒绝重复名称、不安全路径、超量条目和超量解压大小。
   - 对 ZIP64 明确返回不支持，不得绕过限制。
   - 解析 `META-INF/encryption.xml`，拒绝非字体加密资源。

3. `rbook` 类型不进入公开 API。适配层将 metadata、manifest、spine、TOC 和 cover
   映射到 M1A 的 `ParsedBook`、`ResourcePath` 和 `EpubErrorCode`。

4. 产品继续保留现有合成夹具、安全路径和指纹类型。

## 证据

- `rbook` 解析 6/6 个预期成功合成夹具；缺 OPF 夹具稳定报错。
- 不安全路径和重复条目在 rbook 前被预检拒绝。
- 3 本 Gutenberg EPUB2 与 1 本 EPUB3 均通过。
- EPUB3 样本解析为 version 3.0、spine 32、TOC 33。
- 100 MB EPUB：
  - precheck 峰值 RSS 约 1.8 MB。
  - rbook open 峰值 RSS 约 2.1 MB。
  - 端到端峰值 RSS 约 2.2 MB。
- `rbook` 为 Apache-2.0，无运行时网络依赖。

## 后果

- 不自行实现 OPF、NCX 和 nav 语义解析器。
- 仍需自行实现 ZIP 安全预检和适配层，这些属于产品安全边界。
- `rbook` API 变化被限制在 `epub-core` 内部；升级必须更新 ADR 并重跑兼容性测试。
- `rbook` 的 `InvalidResource` 需要结合预检上下文映射，不能只依赖错误文本。
- TOC root 节点必须在适配层移除或规范化。
- 当前预检拒绝 ZIP64。若未来需要支持，必须新增 ADR 和限制测试。
