# ADR-0003：EPUB 阅读渲染与分页路线

**状态：** 已接受  
**日期：** 2026-11-14  
**决策人：** 用户  
**相关任务：** `TASK-M3A`–`TASK-M3D`

## 背景

M3 需要验证 Qurner 能否在 macOS WKWebView 中安全地渲染可重排 EPUB，并提供稳定的
分页、连续滚动和阅读位置恢复。候选路线必须在离线、保留原书排版、不执行原书脚本和
100 MB 按需读取约束下成立。

## 决策

建议采用：

1. Tauri 自定义 `epub://<session-token>/...` 协议，只服务当前会话 manifest 资源。
2. `<iframe sandbox="allow-scripts">`，不授予 same-origin、forms、popups 或顶层导航。
3. 每响应 CSP nonce，只注入构建好的可信阅读 IIFE；运行时再次清洗主动内容和外部 URL。
4. TypeScript 阅读引擎使用 WKWebView CSS 多栏分页，连续滚动使用原生纵向文档滚动。
5. `ReaderLocator` v1 使用 spineIndex、spineHref、UTF-16 textOffset、章节内进度和
   exact/prefix/suffix quote；跨章节由导航层根据 `spine-request` 串联。
6. 父窗口与 iframe 使用严格 Zod 版本化消息，所有响应经过 source、origin、sessionId
   和 token 校验。

## 证据

- 六类有效 EPUB 通过 Rust 解析、会话和协议矩阵。
- Playwright WebKit 20 项测试通过，包含六类等价页面、安全攻击、分页、滚动、Locator、
  字体和性能。
- 嵌套目录正确映射到 spine/fragment。
- 100 MB EPUB 小章节读取 2,690 字节，未加载大条目。
- 20 次 turn-page 独立复测 p95 9 ms，低于 150 ms 门槛。
- 没有新增外部依赖，打包产物与 release 构建成功。

## 已知限制与条件

- 自动化使用 Playwright WebKit + HTTP 夹具；Rust mock 覆盖真实 `epub://` 响应，但尚未
  在真实 Tauri WKWebView 中自动化加载自定义协议。
- M4 首个 UI 集成任务必须完成真实 `Qurner.app` 的 `epub://` iframe 冒烟，包括脚本、
  相对 CSS/图片/字体、CSP 和 postMessage 握手；失败时停止 M4 并重新评估。
- M6 前必须增加真实复杂 EPUB 回归，重点检查表格、大图片、绝对定位、固定宽度元素、
  字体和空白尾页。
- 伪 TTF 只验证请求和 CSS 路径，不验证自定义字形渲染。
- 当前跨章节只发送 `spine-request`，实际章节加载属于 M5。

## 后果

- M3 已关闭，可以进入 M4。
- 后续若 CSS 多栏或 Locator v1 在真实书写场景失败，必须停止上层功能并通过新 ADR
  评估替代分页方案。
- Locator 数据语义保持版本 1；改变 offset 单位或 quote 规则必须迁移。
