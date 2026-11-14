# TASK-M3B：阅读 iframe 安全边界与消息桥

**状态：** accepted  
**里程碑：** M3  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** TASK-M3A accepted

## 1. 目标

让 `epub://` 返回的书籍文档只能在隔离 iframe 中运行应用可信阅读脚本，原书脚本、
事件、远程资源和外部导航不可执行；父窗口与 iframe 使用 Zod 校验的版本化消息信封
完成可信握手。

## 2. 范围

### 必须完成

- 增加 `zod` 运行时依赖及严格消息 schema。
- 为每个 HTML 响应生成随机 CSP nonce，并注入唯一可信阅读脚本标签。
- 通过保留路径 `epub://<token>/__qurner/reader-engine.js` 提供应用构建的 IIFE。
- 在 HTML 运行时再次移除脚本、事件属性、危险 URL、嵌入框架、`base` 和
  `meta refresh`。
- 设置主应用 CSP、iframe 响应 CSP 和 sandbox 边界。
- 实现父窗口/iframe 双向握手、来源检查、会话检查、超时和错误消息。
- 用 Rust、Vitest 和 Playwright WebKit 验证安全边界及消息拒绝路径。

### 不属于本任务

- 分页、连续滚动、页数、方向键、Locator 和章节切换；由 M3C 实现。
- 目录侧栏、阅读器控制条、书签、设置、进度或书库 UI。
- 正式 `openBook`/`closeBook` command、数据库读取或阅读历史。
- 插图放大、脚注、外部浏览器打开或 DRM 处理。
- 完整六类 EPUB 的 M3 总验收和 ADR；由 M3D 完成。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收要求 |
|---|---|---|
| FR | FR-005, FR-006, FR-007 | 不支持内容不执行；无远程请求和脚本自动执行 |
| FR | FR-120–FR-125 | 为后续目录消息保留章节 `spineHref` 标识，不实现 UI |
| NFR | NFR-011 | 阅读 iframe 不发起 HTTP/HTTPS/未知协议请求 |
| NFR | NFR-012 | iframe 握手失败、消息非法和资源失败有稳定错误 |
| NFR | NFR-013 | 消息与文档安全变换位于可测试边界 |
| AC | AC-002, AC-120 | 运行时安全隔离成立；章节消息具备稳定标识 |

## 4. 设计约束

### 4.1 可信脚本

- 可信阅读脚本源码位于 `src/reader-engine/`，使用 TypeScript，构建为无框架 IIFE。
- 构建产物由 `epub://<token>/__qurner/reader-engine.js` 提供；该路径必须在该会话有效
  时返回、会话关闭后 404，且内容不得来自书籍 ZIP。
- 书籍 manifest 若声明 `__qurner/` 路径，协议必须拒绝服务该书籍资源，不能覆盖或伪装
  内置脚本。
- 脚本响应的 MIME 为 `text/javascript; charset=utf-8`，带
  `X-Content-Type-Options: nosniff` 和 `Cache-Control: no-store`。
- 脚本只从 URL host 读取协议令牌；`sessionId` 从 Rust 注入的非敏感 data 属性读取。
  nonce 不得作为长期凭据，也不得写入日志。

### 4.2 HTML 变换与 CSP

- 每个 `<xhtml/html>` 响应生成独立 CSPRNG nonce，至少 128 位随机值；nonce 只存在于
  当前响应头和注入标签。
- 注入的脚本标签严格为：

```html
<script nonce="<nonce>"
  src="epub://<session-token>/__qurner/reader-engine.js"
  data-session-id="<session-id>"></script>
```

- 同一文档只能存在一个 nonce 脚本；不得注入内联业务脚本或 `unsafe-inline` 脚本。
- 运行时使用 HTML 流式改写再次移除：
  - `<script>`、`<iframe>`、`<object>`、`<embed>`；
  - `on*` 事件属性；
  - `javascript:`、`vbscript:`、`data:text/html` URL；
  - `<base>` 和 `http-equiv="refresh"`；
  - iframe 内部可导航的外部链接。文字保留，危险 `href` 移除并提供可识别标记。
- CSS `@import` 和远程图片/字体可保留源码文本，但由 CSP 阻断请求；不得通过服务端补全
  或代理远程资源。
- HTML 响应 CSP 至少包含 SDS 6.2 的完整策略，并增加 `base-uri 'none'` 和
  `form-action 'none'`。
- 主应用 `tauri.conf.json` CSP 允许当前主界面、Tauri IPC/localhost、`epub:` iframe，
  不授予远程 HTTP/HTTPS。不得使用通配 `*`。
- iframe 必须使用 `sandbox="allow-scripts"`，不得授予 `allow-same-origin`、
  `allow-forms`、`allow-popups` 或 `allow-top-navigation`。

### 4.3 消息协议

严格统一信封：

```ts
type ReaderMessage = {
  protocolVersion: 1
  sessionId: string
  sessionToken: string
  type:
    | 'reader:init'
    | 'reader:ready'
    | 'reader:turn-page'
    | 'reader:error'
  payload: Record<string, unknown>
}
```

本任务 payload：

```ts
'reader:init'      -> {}
'reader:ready'     -> { engineVersion: string }
'reader:turn-page' -> { direction: 'prev' | 'next' }
'reader:error'     -> { code: string; message: string }
```

- Zod 对象全部使用 strict 行为，拒绝未知字段、错误类型、错误 `protocolVersion`、空
  或格式错误的 sessionId/token 以及不适用 payload。
- 父窗口发送前校验并只向当前 iframe `contentWindow` 发送，接收时验证
  `event.source === iframe.contentWindow`。
- iframe 只接受 `event.source === window.parent`、预期父 origin（由注入配置或
  Rust/构建常量提供）以及匹配的 sessionId/token。
- 初始握手必须在固定超时内完成；超时、重复 ready、乱序 init、错误 token 和关闭后的
  消息不得改变 DOM 或再次发送成功响应。
- 本任务不实现分页行为；合法 `turn-page` 只验证消息契约，不改变正文位置，并发送固定
  `reader:error` payload `{ code: "E_NOT_IMPLEMENTED", message: "分页尚未启用。" }`。
  不得伪造翻页成功；M3C 再接入真实引擎。

### 4.4 依赖

- 用户已明确批准添加 `zod`。使用与当前 TypeScript/Vite 兼容的稳定版本，并由 lockfile
  锁定。
- 除 `zod` 外不得新增运行时依赖。可以继续使用已有的 `lol_html`、Vite、Vitest 和
  Playwright WebKit。
- 不新增 Tauri plugin、文件系统、shell、HTTP 或远程字体能力。

## 5. 文件权限

### 允许修改

- `package.json`、`package-lock.json`（仅 `zod`）
- `src/bridge/`、`src/reader-engine/`
- `vite.reader.config.ts` 或等价独立构建配置
- `tests/e2e/` 及专用 WebKit 测试页面/夹具
- `src-tauri/src/protocol/`、`src-tauri/src/lib.rs`
- `src-tauri/resources/`（阅读脚本构建产物或打包元数据）
- `src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`（仅既有 `lol_html`/构建依赖）
- `src-tauri/tests/`
- 本任务状态和第 9 节开发报告

### 禁止修改

- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`
- `crates/storage-core/`、`crates/library-core/`、`crates/reader-model/`
- 数据库 schema、M3C/M3D 任务文件
- 书库/导入 UI、正式阅读器 UI、路由和无关前端组件
- Tauri capability 中的 shell/http/fs/dialog 权限

## 6. 测试要求

### 6.1 Rust 协议测试

- 每个 HTML 响应 nonce 不同、长度/格式满足要求，CSP 头包含精确 nonce。
- 注入脚本 URL、sessionId 和唯一脚本标签正确；原书已有 script/event/embed/base/
  meta-refresh 被移除。
- 相对 CSS、图片和字体 URL 不被破坏；外部链接在 iframe 中不可导航。
- `__qurner/reader-engine.js` 在会话有效时返回构建 IIFE，会话关闭后 404；书籍
  manifest 声明保留路径不能覆盖内置脚本。
- 非 HTML 资源不发生 HTML 注入，但仍带 `nosniff` 和 `no-store`。
- CSP/header 非法或响应构造失败仍返回固定 500，不泄漏 token、路径或正文。

### 6.2 TypeScript/Vitest 测试

- 表单/负向 schema 表覆盖未知字段、缺失字段、错误版本、错误类型和不适用 payload。
- 父窗口 bridge 不向 null/错误 window 发送；事件 `source` 不匹配时丢弃。
- iframe bridge 拒绝错误 parent、origin、sessionId、token 和乱序 ready/init。
- 握手超时、重复 ready、错误消息和关闭流程有明确状态，不产生未处理 Promise。

### 6.3 Playwright WebKit 测试

- 加载带生产 CSP 的 XHTML 夹具，可信脚本执行并完成 `reader:ready`。
- `<script>`、内联事件、`javascript:` 链接、远程脚本、远程图片和远程 CSS 均不能执行
  或产生网络请求。
- iframe 外部链接点击不导航；表单提交被阻止。
- 来自错误 window/伪造 token 的消息被忽略。
- 测试页面不访问真实书籍或个人数据；网络请求通过 Playwright route 记录并断言为零。

## 7. 必跑命令

```bash
npm install zod
npm run check
npm run test:e2e
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p qurner --test m3_security
cargo test --workspace
npm run build
npm run tauri build
```

开发报告必须记录实际安装的 Zod 版本和 lockfile 变化。

## 8. 禁止事项

- 不提交、不推送、不创建 PR。
- 不安装 `zod` 之外的依赖，不访问非安装所需的外部网络。
- 不使用 `unsafe-inline` 执行脚本，不使用 `allow-same-origin` 放宽 sandbox。
- 不实现 M3C/M3D 的分页、Locator、六类夹具总验收或正式阅读 UI。
- 不跳过、删除或弱化安全测试，不把 CSP 源码存在当成运行时已阻断。

## 9. DeepSeek 开发报告

> 仅在开始实现后填写。开始实现时将状态改为 `in_progress`。

**完成摘要：**

在 M3A 的 `epub://` 资源协议之上实现阅读 iframe 安全边界与双向消息桥；本轮按第 10 节
第三轮复验修复控制字符夹具、协议相对 action/formaction 与 WebKit 混淆导航测试。

基础实现：

- `zod` 4.6.7 严格 `ReaderMessage` 信封（`reader:init/ready/turn-page/error`），所有
  对象 `z.strictObject`；会话 ID 统一由 UUID v4 schema 校验（消息与 iframe 配置共用
  `isValidSessionId`）。
- 父桥（`parent.ts`）只向当前 `iframe.contentWindow` 发送，校验 source/schema/
  session/token；ready 后继续处理 `reader:error` 并转发 `onError`；`turnPage(direction)`
  仅在 ready 发送。iframe 桥（`iframe.ts`）校验 source/parentOrigin/session/token，
  只接受一次 init、发送一次 ready，`turn-page` 返回固定 `E_NOT_IMPLEMENTED`。
- 阅读引擎独立 IIFE 构建为 `src-tauri/resources/reader-engine.js`，由
  `epub://<token>/__qurner/reader-engine.js` 提供。
- `protocol/html.rs` 使用 `lol_html` 流式移除主动内容、清洗危险 URL 并注入唯一
  nonce 脚本；协议层生成独立 128 位 nonce 与 SDS 6.2 CSP，保留路径优先于 manifest。
- 主应用 CSP 允许 `'self'`、ipc、`frame-src epub:`，无远程 HTTP/HTTPS、无通配。

第 10 节第三轮复验后修复（本轮）：

1. **真实控制字符夹具**：`m3_security.rs` 的恶意章节改用 `concat!` 内嵌真实
   `\t`、`\n`、`\r`：`href="h\tttp://…"`、`href="java\nscript:…"`、
   `href="h\rttp://…"`、`xlink:href="h\tttp://…"`、
   `action="h\tttp://…"`、`formaction="java\nscript:…"`；同时保留
   `action="//evil.test/form"`、`formaction="//evil.test/submit"`。断言这些属性均
   被移除，覆盖 href、xlink:href、action、formaction 四类。
2. **协议相对 action/formaction**：`html.rs` 的 `action`/`formaction` 分支在
   `strip_control_whitespace` 后同时拒绝任何 scheme 与 `//` 开头；新增回归断言。
3. **WebKit 混淆导航测试**：Playwright 新增用例，iframe 文档含
   `href="h\tttp://evil.test/tab" target="_top"` 与
   `href="java\nscript:window.pwned = 1"`；点击后父页面 URL 不变、无
   `evil.test` 请求、iframe 的 `pwned` 未定义（sandbox/CSP 兜底）。

**修改文件：**

- `package.json`、`package-lock.json`（zod 4.6.7；`build:reader` 与 `build` 链）
- `src/bridge/messages.ts`、`parent.ts`、`iframe.ts` 及 `*.test.tsx`
- `src/reader-engine/index.ts`
- `vite.reader.config.ts`
- `src-tauri/resources/reader-engine.js`（构建产物）
- `src-tauri/src/protocol/mod.rs`（HTML 分支、nonce 拆分与单元测试）
- `src-tauri/src/protocol/html.rs`（链接防护、空白规范化、action/formaction、head/body）
- `src-tauri/Cargo.toml`（`lol_html.workspace`）
- `src-tauri/tauri.conf.json`（主应用 CSP）
- `src-tauri/tests/m3_security.rs`（真实控制字符夹具）
- `src-tauri/tests/m3_protocol.rs`（header helper 适配动态头）
- `tests/e2e/reader-security.spec.ts`、`tests/e2e/globals.d.ts`
- `docs/tasks/TASK-M3B.md`（顶部状态与本第 9 节）

未修改 `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、
`crates/storage-core/`、`crates/library-core/`、`crates/reader-model/`、数据库
schema、M3C/M3D 任务文件、书库/阅读器 UI 或 Tauri capability；未创建 commit。

**关键设计选择：**

- 会话 ID 格式只有一处定义（`messages.ts` 的 UUID v4 schema），父/iframe 配置与
  消息校验共用。
- URL scheme 检测先剔除浏览器 URL 解析会忽略的 `\t`/`\n`/`\r`，再识别 scheme 或
  协议相对 `//`；`href`、`xlink:href`、`action`、`formaction`、`src`、`poster` 使用
  同一规范化。
- nonce 生成拆成“读取随机字节”和“回退生成”（UUID v4 / getrandom）两个可注入步骤，
  主路径与回退分别有单元测试。
- 父桥状态机：只有 ready 改变状态，`reader:error` 只回调；`turnPage` 在关闭/未就绪
  时不发送。
- head/body 注入由解析器事件驱动（`on_end_tag`/`prepend` + `Rc<Cell<bool>>`），
  不依赖原始字符串。
- WebKit 混淆测试验证浏览器侧兜底（sandbox 阻止 `target="_top"` 导航、CSP 阻止
  `javascript:`），生产清洗由 Rust 测试覆盖。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm install zod` | 0 | zod 4.6.7；`package.json`/`package-lock.json` 已更新 |
| `npm run check` | 0 | `tsc -b`、Biome 23 文件、Vitest 5 文件 23 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 7 passed（含混淆导航） |
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p qurner --lib` | 0 | 17 passed（含 nonce primary/fallback） |
| `cargo test -p qurner --test m3_security` | 0 | 7 passed |
| `cargo test --workspace` | 0 | 237 passed，0 failed，1 ignored |
| `npm run build` | 0 | 阅读引擎 + 主应用 Vite 构建成功 |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.13 MiB |

Playwright 通过 route 拦截 `http://qurner.test/**` 与 `http://<token>.test/**`；
`evil.test` 请求数为 0；内联/远程脚本、图片、CSS 被 CSP 阻断；伪造 token 消息被忽略；
`turn-page` 返回固定 `E_NOT_IMPLEMENTED`；混淆链接不导航/不执行。

**AC 验证：**

- FR-005 / FR-006 / FR-007：原书脚本、事件属性、危险/控制字符混淆 URL、嵌入框架、
  `base`、`meta refresh` 移除；外部链接/area/xlink/ping/action/formaction 不可导航或
  产生外部副作用；远程资源由 CSP 阻断。
- FR-120–FR-125（标识前提）：`ParsedBook` 的 spine/manifest 保留章节 `href`；
  `turn-page` 契约已实现但不执行分页。
- NFR-011：iframe 无 HTTP/HTTPS 请求；无新增网络插件/shell/fs/dialog 权限。
- NFR-012：握手超时、非法消息、错误 source/origin/token 被忽略；协议层非法
  HTML/构造失败固定 500。
- NFR-013：消息 schema、父子桥、HTML 变换、nonce/CSP 均可独立测试。
- AC-002 / AC-120：sandbox 隔离、唯一可信脚本、双向握手、伪造消息拒绝通过。

**未验证项：**

- 未在真实 Tauri WKWebView 中加载 `epub://` iframe 与脚本；Playwright 用 `http://`
  夹具模拟（相同 CSP/HTML/脚本结构），Rust 覆盖真实 `epub://` 响应。
- 主应用 CSP 仅通过 `tauri build` 静态生效，未人工运行 app 验证渲染。
- sandbox opaque origin 下父→iframe `postMessage` 使用 `"*"`；真实 WKWebView 行为
  未端到端复测。
- WebKit 混淆测试覆盖 sandbox/CSP 兜底；生产 HTML 清洗由 Rust 测试覆盖，二者未在
  同一真实 `epub://` 响应上串联验证。
- 未测试样式/图片/字体在 iframe 中的实际渲染。
- 未测试 iframe 被导航到外部文档后的消息泄漏场景。
- 未测试 `reader:turn-page` 的真实分页行为（M3C）。
- 未逐一覆盖超长 payload、二进制消息或非 JSON 消息。
- 未测试多 iframe/多会话同时存在的父桥隔离（单 iframe 假设）。
- nonce primary/fallback 由单元测试覆盖；真实 `/dev/urandom` 缺失平台未触发回退。

**风险与待决问题：**

- 父桥 `targetOrigin "*"` 定向 contentWindow；若 iframe 被导航到恶意文档，init 消息
  可能被其接收。当前 sandbox 无 top-navigation，内容受 CSP/变换保护；M3C 可考虑
  load 后校验或调整 sandbox/origin 策略。
- `lol_html` 改写会规范化部分属性引号/空白；依赖精确字节的样式未验证。
- 每 HTML 响应重新生成 nonce 与改写，未缓存；真实书籍性能未测。
- `__qurner/` 为保留前缀：书籍中真实使用该前缀的资源将无法读取（安全优先）。
- 阅读引擎含 zod（约 86.6 kB），无缓存（`no-store`）。
- 主应用 CSP `connect-src` 仅允许 ipc；M4 增加资源获取时需显式扩展。
- `reader-engine.js` 是构建产物；未先运行 `npm run build:reader`（或 `npm run build`）
  时 Rust `include_str!` 编译会失败，已由脚本链保证。

**依赖变更：**

- 运行时依赖新增 `zod@4.6.7`（用户明确批准），`package-lock.json` 已锁定。
- `package.json` scripts 新增 `build:reader`，`build` 链式先构建引擎（不新增依赖）。
- `src-tauri/Cargo.toml` 新增 `lol_html.workspace`（workspace 既有依赖），
  `Cargo.lock` 相应更新。
- 未新增 Tauri plugin、shell/http/fs/dialog 权限或网络依赖。

## 10. Codex 审查

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `npm run check`：通过；Biome 23 个文件无修复，Vitest 23 passed。
- `npm run test:e2e`：通过；Playwright WebKit 7 passed。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p qurner --test m3_security -- --nocapture`：7 passed。
- `cargo test -p qurner --lib`：17 passed。
- `cargo test --workspace`：237 passed，0 failed，1 ignored。
- `npm run build`：通过；阅读引擎 86.77 kB，主应用构建成功。
- `npm run tauri build`：通过；生成 7.13 MiB `Qurner.app`。
- `npm ls zod --depth=0`：`zod@4.6.7`。

**本轮确认：**

- `IframeBridge` 已复用 UUID v4 validator，非法普通字符串和错误版本测试通过。
- Rust 夹具已包含真实 TAB/LF/CR，覆盖 href、xlink:href、action、formaction。
- `action`/`formaction` 同时拒绝绝对 scheme 和协议相对 `//host`。
- nonce primary/fallback 已拆出并分别测试 32/64 hex 与连续性。
- 父桥 ready 后 error 保持 ready、closed 后 turnPage 不发送消息均有测试。
- Playwright WebKit 混淆导航测试通过，`evil.test` 外部请求为零。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现 `BLOCKER`、`MAJOR` 或影响当前 AC 的 `MINOR` 问题。 | — |

**AC 结果：**

- FR-005、FR-006、FR-007：通过；真实控制字符混淆和协议相对 form action 均被阻断。
- FR-120–FR-125：父桥 turnPage/error 时序通过。
- NFR-011：WebKit route 断言和混淆导航测试通过，无 `evil.test` 请求。
- NFR-012、NFR-013：UUID 配置、nonce primary/fallback、父桥状态边界均通过。
- AC-002 / AC-120：通过。

**后续任务：**

TASK-M3B 通过。真实 Tauri WKWebView 的 `epub://` 端到端加载、sandbox opaque-origin
行为和实际书籍渲染仍由 M3C/M3D 验证，不阻塞当前任务。
