# ADR-0001：Qurner 应用标识

**状态：** 已接受  
**日期：** 2026-11-14  
**决策人：** 用户  
**相关任务：** `TASK-M0`

## 背景

初始 Bundle Identifier 为 `com.qurner.app`。Tauri 构建时明确警告该标识以
`.app` 结尾，会与 macOS 应用 Bundle 扩展发生概念冲突。M0 尚未产生正式个人阅读数据，
此时修改标识不会触发数据迁移。

## 决策

- 产品名保持 `Qurner`。
- Bundle Identifier 使用 `com.qurner.reader`。
- 应用数据目录使用：
  - `~/Library/Application Support/com.qurner.reader/`
  - `~/Library/Caches/com.qurner.reader/`
- 不再使用 `com.qurner.app` 创建新的应用数据。

## 影响

- Tauri 配置、Info.plist、构建产物和后续数据路径必须使用新标识。
- M0 旧测试产生的 WebKit 缓存不是用户数据，不要求迁移。
- 后续修改 Bundle Identifier 会影响应用身份和本地数据目录，必须重新形成 ADR。
