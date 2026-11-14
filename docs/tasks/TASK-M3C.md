# TASK-M3C：分页、连续滚动与 Locator 原型

**状态：** accepted  
**里程碑：** M3  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** TASK-M3A accepted、TASK-M3B accepted

## 1. 目标

在现有受控阅读 iframe 中实现可验证的 CSS 多栏分页、连续滚动、模式切换和文本 Locator
捕获/恢复，证明同一章正文可以在布局、窗口和模式变化后稳定回到相近文本位置。

## 2. 范围

### 必须完成

- 在 `reader-model` 定义版本化 `ReaderLocator`、quote 和阅读模式模型。
- 扩展 Zod/Tauri iframe 消息，支持 init、go-to、turn-page、模式切换、capture、
  location/page 状态和章节边界请求。
- 在 `src/reader-engine` 实现分页、连续滚动、首尾页和稳定页步长。
- 实现 UTF-16 文本偏移、quote 捕获、精确定位、文本回退和歧义选择。
- 改变模式或窗口尺寸时捕获并恢复 Locator。
- 使用 Vitest 与 Playwright WebKit 验证分页、滚动、跨模式和 Locator 边界。
- 保留 M3B 的 CSP、脚本唯一注入、消息鉴权和外部导航阻断。

### 不属于本任务

- 正式 `openBook`/阅读会话 command、数据库进度、书签或目录 UI。
- 跨 spine 自动加载完整新章节；M3C 只发送 `spine-request`，M5/M3D 再串联。
- 字号、主题、字体、沉浸式、插图放大、脚注、外部浏览器链接或 Esc 控件栈。
- 正式书库/阅读器 UI、Tauri 协议 WebView 端到端总验收或 M3 ADR。
- 第三方分页/阅读引擎替换。若 CSS 多栏方案不达标，停止并报告，不得自行更换方案。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收要求 |
|---|---|---|
| FR | FR-070–FR-073 | 分页、滚动、切换和单次运行内模式状态可验证 |
| FR | FR-087 | 布局变化后回到原文本附近，不跳回章节开头 |
| FR | FR-120–FR-122 | Locator 保留 spineIndex/spineHref，为目录映射提供稳定标识 |
| FR | FR-160–FR-161 | Locator 可序列化、捕获和恢复，为持久化提供领域模型 |
| AC | AC-070, AC-120, AC-160 | spike 覆盖模式、章节标识和位置恢复 |
| NFR | NFR-005 | 已在布局内的 turn-page 响应可测量，不引入同步网络操作 |
| NFR | NFR-013 | Locator 和布局引擎保持不依赖 Tauri 的可测试边界 |

## 4. 设计约束

### 4.1 领域模型

- `reader-model` 新增 `ReaderMode`（`paged`/`scrolling`）、`ReaderQuoteContext` 和
  `ReaderLocator`，serde 使用 camelCase，版本固定为 `1`。
- `spineIndex` 为 32 位无符号整数；`spineHref` 非空且只允许 EPUB 根内相对路径，
  拒绝绝对路径、反斜杠、NUL、空段越界和 `..`。
- `textOffset` 为非负整数，M3C 上限为 JavaScript 安全整数
  `9_007_199_254_740_991`；Rust 使用 `u64` 并在构造时校验。
- `intraSpineProgression` 必须有限且在 `0..=1`。
- quote 的 `exact` 最多 128 个 UTF-16 code unit，prefix/suffix 各最多 32；正常锚点
  目标 exact 长度为 32，短章节可用实际可用文本，全空章节 exact 可为空。
- 所有模型必须可 JSON round-trip；非法 progression、超长字段、不安全 href 和未知
  schema version 必须拒绝。

### 4.2 文本偏移

- 按 DOM 顺序遍历正文文本节点，排除 `script/style/noscript/template` 和明确的非渲染
  内容。
- `textOffset` 使用 UTF-16 code unit，与浏览器 `Range` 和 JS string 长度一致；CR/LF/
  CRLF 统一为 LF，连续空白不折叠。
- 建立规范化偏移到 `(Text, sourceOffset)` 的映射，捕获和恢复必须使用同一映射算法。
- 捕获锚点优先取位于阅读视口顶部/中部附近的可见正文文本；不得用固定屏幕坐标作为
  Locator。
- exact/prefix/suffix 以纯文本保存，不包含 HTML、路径、书名或敏感诊断信息。

### 4.3 分页

- CSS 多栏使用独立 `qurner-*` 类名和最终布局样式，只覆盖布局/尺寸，不覆盖原书字体、
  颜色和段落到无法阅读的程度。
- 页步长严格为 `viewportWidth + columnGap`；当前页按最近步长取整并 clamp。
- pageCount 从实际最大横向滚动量计算，不使用可能产生空白尾页的简单总宽 `ceil`。
- 首页 `pageIndex=0`，末页索引为 `pageCount-1`；每个可达页必须有正文或图片。
- `turn-page` 在章节内部翻页；首/末页再次向该方向翻页时发送一次对应的
  `spine-request`，不得伪造跨章成功。
- 翻页后发送 `page-changed` 和 `location-changed(reason="turn")`。

### 4.4 连续滚动

- 使用原生纵向文档滚动；不建立第二套虚拟滚动文本。
- `turn-page` 的 prev/next 在滚动模式滚动约一个视口高度；上下方向键保持系统默认。
- 滚动停止后发送 location-changed，并按短防抖避免每个像素产生消息。
- 顶部/底部边界发送一次 `spine-request`；重复输入不得在位置离开边界前重复发送。
- 滚动模式页码固定 `pageIndex=0, pageCount=0`。

### 4.5 模式与布局恢复

- `reader:init` 携带 mode 与 initialLocator；初始布局和恢复完成后才发送 ready。
- `reader:update-settings` 当前只接受 mode；切换前捕获 Locator，切换后恢复到同一
  文本锚点。
- `ResizeObserver`/window resize 和字体加载完成先捕获再恢复，布局期间暂缓翻页输入。
- 精确 textOffset 的 quote 不匹配时，在本章文本中搜索 exact 与上下文；多处匹配选择
  intraSpineProgression 最近者。
- 无匹配回到本章开头，发送一次可恢复 `E_LOCATOR_NOT_FOUND`，不得跳到其他章节。
- 布局恢复完成发送 `location-changed(reason="layout")`。

### 4.6 消息与父桥

严格新增 payload：

```ts
reader:init -> { mode: ReaderMode; initialLocator: ReaderLocator | null }
reader:go-to -> { locator: ReaderLocator }
reader:update-settings -> { mode: ReaderMode }
reader:capture-locator -> {}
reader:location-changed -> {
  reason: 'init' | 'turn' | 'scroll' | 'layout' | 'go-to' | 'capture'
  locator: ReaderLocator
  mode: ReaderMode
  pageIndex: number
  pageCount: number
  atStart: boolean
  atEnd: boolean
}
reader:page-changed -> {
  pageIndex: number
  pageCount: number
  atStart: boolean
  atEnd: boolean
}
reader:spine-request -> { direction: 'prev' | 'next'; locator: ReaderLocator }
```

- ParentBridge 增加 initialMode/initialLocator、`goTo`、`updateMode`、
  `captureLocator` 以及 location/page/spine-request 回调。
- 所有子消息继续执行 source、parentOrigin、sessionId、token 和 strict Zod 校验。
- M3B 原有的 `turn-page` 错误占位删除；合法 turn-page 执行真实翻页/滚动。

### 4.7 注入的章节信息

- 协议为 spine 内 XHTML 的 trusted script 增加 `data-spine-index` 和
  `data-spine-href`；值来自当前 `ParsedBook.spine`，不从 URL 猜测。
- `data-spine-href` 使用浏览器/HTML 属性安全转义。非 spine HTML 不启用阅读引擎或使用
  明确不可初始化状态，避免生成错误 Locator。
- 引擎只处理当前文档章节，不读取任意文件或网络。

## 5. 文件权限

### 允许修改

- `crates/reader-model/`
- `src/bridge/`（消息、父桥及测试）
- `src/reader-engine/`
- `src-tauri/src/protocol/html.rs`、`src-tauri/src/protocol/mod.rs`
- `src-tauri/resources/reader-engine.js`（构建产物）
- `vite.reader.config.ts`、`package.json`（仅脚本；不得新增依赖）
- `tests/e2e/` 及分页/Locator 专用夹具
- 本任务状态和第 9 节开发报告

### 禁止修改

- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`
- `crates/storage-core/`、`crates/library-core/`、数据库 schema
- 正式书库/阅读器 UI、路由和 M3D 任务文件
- Tauri capability、远程网络权限或 CSP 降级
- 第三方阅读引擎依赖

不得新增外部依赖；继续使用已有 Zod、HTML/CSS/Web APIs、Vitest 和 Playwright WebKit。
若 CSS 多栏无法满足契约，停止并提交数据与最小复现，不得自行改选方案。

## 6. 测试要求

### 6.1 Rust 模型

- ReaderMode/ReaderLocator/quote camelCase JSON round-trip。
- 拒绝不安全 spineHref、错误 version、负数/超大 textOffset、NaN/Infinity/越界
  progression、超长 exact/prefix/suffix。
- 中文、emoji（代理对）和短/空章节 quote 的 UTF-16 长度边界正确。

### 6.2 Vitest

- 新消息 schema 的正/负向表，包括未知字段、错误 Locator、错误 page 状态。
- ParentBridge 仅 ready 后发送 go-to/update-mode/turn-page/capture；关闭后零发送。
- 子消息 source/origin/session/token 错误时忽略；location/page/spine 回调只触发一次。
- Locator 文本索引、quote 搜索、歧义选择和无匹配回退可在不含布局的测试中验证。

### 6.3 Playwright WebKit

使用固定 viewport 和合成 XHTML/CSS：

- 长章节分页 pageCount>1；next/prev 更新索引，首页/末页 clamp，无横向溢出和空白尾页。
- 连续滚动 turn-page 约一屏；顶部/底部只发送一次对应 spine-request。
- 在章节中段捕获 Locator 后，窗口 resize 可恢复同一 quote；不得回到章节开头。
- 分页→滚动→分页使用同一 Locator，恢复文本偏移误差不超过一个文本节点，quote 仍可命中。
- go-to 可跳到指定文本；文件内容变化导致偏移失效时通过 quote/上下文找回；无匹配时回
  章节开头并只发送一次 E_LOCATOR_NOT_FOUND。
- 中英文混排、emoji 和长段落后的偏移/恢复正确。
- 原有 CSP、远程请求为零、可信脚本握手和伪造消息测试继续通过。

## 7. 必跑命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p reader-model
cargo test -p qurner --lib
cargo test -p qurner --test m3_security
cargo test --workspace
npm run check
npm run test:e2e
npm run build
npm run tauri build
```

## 8. 禁止事项

- 不提交、不推送、不创建 PR。
- 不安装依赖、不访问外部网络、不修改用户书籍或个人数据。
- 不修改 CSP、sandbox、消息鉴权或 manifest 白名单来让测试通过。
- 不实现 M4/M5 UI，不伪造跨章节加载、进度持久化或正式分页完成。
- 不跳过、删除或弱化失败测试，不把未验证内容写成通过。

## 9. DeepSeek 开发报告

> 仅在开始实现后填写。开始实现时将状态改为 `in_progress`。

**完成摘要：**

在 M3A/M3B 的受控阅读 iframe 中实现分页、连续滚动、模式切换和文本 Locator 原型；本轮
按第 10 节第三轮复验修复滚动 anchor、init 位置事件、跨 spine 错误码并补齐 WebKit 边界
测试。

基础实现：

- `reader-model` 新增 `ReaderMode`、`ReaderQuoteContext`、`ReaderLocator`（version 1、
  camelCase）及统一校验。
- `src/bridge/messages.ts` 严格 Zod 协议（init/go-to/update-settings/capture-locator/
  location-changed/page-changed/spine-request）。
- `src/bridge/parent.ts` 支持 initialMode/initialLocator、goTo/updateMode/
  captureLocator 与 location/page/spine 回调，只在 ready 后发送。
- `src/bridge/iframe.ts` init 启动引擎，ready 由引擎初始化完成触发。
- `src/reader-engine/locator.ts`：文本索引、offset↔节点映射、quote 捕获/恢复。
- `src/reader-engine/engine.ts`：CSS 多栏分页、连续滚动、边界 spine-request 去重、
  resize/字体/模式切换恢复、go-to、capture-locator、无匹配回退。
- 协议仅对 spine XHTML 注入引擎并附带 `data-spine-index`/`data-spine-href`。

第 10 节第四轮复验后修复（本轮）：

1. **失败后清除 Locator 缓存**：`scrollToStart()` 同时清除 `anchor`；跨 spine 与文本
   无匹配导致回到章节开头后，随后的 `location-changed(reason="go-to")` 返回章节开头
   的位置（pageIndex 0、开头 textOffset），而不是失败前的旧页。
2. **滚动 turn 位置前进断言**：滚动测试在 turn 前 capture、turn 后读取
   `location-changed(reason="turn")` 的 locator.textOffset，断言严格大于翻屏前值。
3. **真正延迟的字体 ready**：字体测试在 iframe 内用 `Object.defineProperty` 覆盖
   `document.fonts.ready` 为受控 Promise，init 后先断言没有 layout 事件，再手动
   resolve；等待 layout 后断言 locator 与 init 位置差 ≤2。不再依赖已立即完成的字体
   状态。
4. **末页可见内容**：末页测试除剩余宽度外，遍历正文文本节点的 client rect，确认当前
   视口内存在可见正文（rect 与视口相交），排除空白尾页。
5. **错误后位置事件断言**：跨 spine 与缺失 Locator 测试均等待对应的 `go-to` 位置
   事件，断言 pageIndex 0 且 textOffset 回到章节开头附近（≤ init 开头 +2），同时验证
   `E_LOCATOR_SPINE_MISMATCH` 与 `E_LOCATOR_NOT_FOUND` 各只发送一次。

**修改文件：**

- `crates/reader-model/src/locator.rs`（新增）、`crates/reader-model/src/lib.rs`
- `src/bridge/messages.ts`、`parent.ts`、`iframe.ts` 及 `*.test.tsx`
- `src/reader-engine/locator.ts`、`engine.ts`、`index.ts`、`locator.test.tsx`
- `src-tauri/src/protocol/html.rs`、`src-tauri/src/protocol/mod.rs`
- `src-tauri/resources/reader-engine.js`（构建产物）
- `src-tauri/tests/m3_security.rs`
- `tests/e2e/reader-security.spec.ts`、`tests/e2e/reader-pagination.spec.ts`
- `docs/tasks/TASK-M3C.md`（顶部状态与本第 9 节）

未修改 `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、
`crates/storage-core/`、`crates/library-core/`、数据库 schema、正式 UI、M3D 任务文件、
Tauri capability/CSP；未新增依赖；未创建 commit。

**关键设计选择：**

- Locator 的 Rust 与 TypeScript 校验规则一一对应；page 状态 schema 分页要求
  `pageIndex < pageCount`，滚动固定 0/0。
- 引擎维护 `anchor`（当前/恢复目标）与 `lastLocator`：capture 优先 anchor，恢复后报告
  恢复目标；滚动 turn、用户滚动与翻页会清除 anchor，避免报告旧位置。
- resize/字体/模式切换走 capture（或 lastLocator）→ applyLayout → restore →
  `location-changed(layout)`。
- 分页 pageCount 由最大横向滚动量计算，翻页 clamp，边界 spine-request 由
  boundarySent 去重；跨章节与文本缺失使用不同错误码，便于 M3D/M5 区分处理。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p reader-model` | 0 | 14 passed |
| `cargo test -p qurner --lib` | 0 | 17 passed |
| `cargo test -p qurner --test m3_security` | 0 | 8 passed |
| `cargo test --workspace` | 0 | 243 passed，0 failed，1 ignored |
| `npm run check` | 0 | `tsc -b`、Biome、Vitest 6 文件 27 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 13 passed |
| `npm run build` | 0 | 阅读引擎 + 主应用 Vite 构建成功 |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.14 MiB |

**AC 验证：**

- FR-070–FR-073：分页/滚动/模式切换与页码状态通过消息验证；init 事件提供初始状态。
- FR-087：resize/模式切换恢复同一 quote，textOffset 误差 ≤2。
- FR-120–FR-122：Locator 保留 spineIndex/spineHref；协议注入 data-spine-*；跨章节
  Locator 返回 `E_LOCATOR_SPINE_MISMATCH`。
- FR-160–FR-161：Locator JSON round-trip 与捕获/恢复。
- AC-070 / AC-120 / AC-160：模式、章节标识与位置恢复覆盖。
- NFR-005：翻页/滚动在布局内同步完成，无网络操作。
- NFR-013：Locator/文本索引/schema/父桥可独立测试。
- M3B 安全边界（CSP、唯一脚本、消息鉴权、外部导航阻断）继续通过。

**未验证项：**

- 未在真实 Tauri WKWebView 中加载 `epub://` iframe；Playwright 用 `http://` 夹具模拟。
- 分页仅用合成 XHTML/CSS；复杂 EPUB（图片/表格/绝对定位/字体）可能产生空白页或
  溢出。
- pageCount 由 scrollWidth 四舍五入，亚像素布局可能有 ±1 误差。
- resize 测试以 capture 验证 Locator，未逐帧验证视觉位置；字体 layout 消息可能与
  resize 时序交错。
- 字体测试验证 `document.fonts.ready` 触发的 layout 路径，未注入真实延迟字体文件。
- `caretRangeFromPoint` 依赖 WebKit；其他引擎可能需回退路径。
- 未测试滚动模式方向键（系统默认保留）。
- 跨章节仅发送 `spine-request`，未串联新章节（M5/M3D）。
- 恢复精度允许一个文本节点；大文本节点内字符级精度未单独量化。

**风险与待决问题：**

- CSS 多栏在复杂原书样式下可能不满足"每页有正文/无空白尾页"，需 M6 前真实书籍复核。
- 恢复依赖 `caretRangeFromPoint` 取可见文本；分页视口边缘可能取到相邻文本。
- 每次 capture 重建文本索引（O(n)），长章节有开销；NFR-005 未量化。
- resize/字体 layout 可能产生多次 layout 事件；前端需以最后一次为准。
- 跨 spine 只发请求不加载新章节，父桥需在 M5/M3D 串联时处理
  `E_LOCATOR_SPINE_MISMATCH` 与新章节初始化。
- 非 spine HTML 不注入引擎，父桥加载非 spine 文档会握手超时。
- 引擎体积约 104 kB，无缓存。

**依赖变更：**

- 无新增外部依赖；沿用既有 Zod、Web APIs、Vitest、Playwright WebKit。
- `package.json`/`Cargo.toml` 未新增依赖。

## 10. Codex 审查

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p reader-model`：14 passed。
- `cargo test -p qurner --lib`：17 passed。
- `cargo test -p qurner --test m3_security`：8 passed。
- `cargo test --workspace`：243 passed，0 failed，1 ignored。
- `npm run check`：通过；Biome 27 个文件、Vitest 6 文件 27 tests。
- `npm run test:e2e`：通过；Playwright WebKit 13 passed。
- `npm run build`：通过；阅读引擎 104.25 kB，主应用构建成功。
- `npm run tauri build`：通过；生成 7.14 MiB `Qurner.app`。

**本轮确认：**

- init location/page 事件已产生，滚动 turn 前后清空 anchor。
- 末页 clamp、尾页剩余宽度、顶部/底部 spine-request、initialLocator、跨 spine 和
  字体 ready layout 均有 WebKit 测试。
- 横栏可见文本捕获、resize/模式恢复和 page schema 通过。
- M3B 安全回归继续通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现 `BLOCKER`、`MAJOR` 或影响当前 AC 的 `MINOR` 问题。 | — |

**AC 结果：**

- FR-070–FR-073：通过；分页、滚动、模式、init 和边界状态均有测试。
- FR-087：通过；initial/resize/模式/字体 layout 均恢复到同一文本。
- FR-120–FR-122：通过；spine 标识、跨 spine 错误和边界请求已验证。
- FR-160–FR-161：通过；Locator Rust/TS round-trip 与捕获/恢复已验证。
- NFR-013：通过；最高风险边界有独立回归。
- AC-070、AC-120、AC-160：通过。

**后续任务：**

TASK-M3C 通过。真实 Tauri WKWebView、复杂 EPUB 布局和六类夹具总验收仍由 M3D 完成。
