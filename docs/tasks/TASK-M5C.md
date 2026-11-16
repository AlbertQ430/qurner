# TASK-M5C：嵌套目录、阅读模式、章节导航与键盘

**状态：** accepted

**里程碑：** M5

**负责人：** DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-15

**依赖：** TASK-M5A、TASK-M5B accepted

## 1. 目标

用户进入阅读器后可以打开保留层级的 EPUB 目录、跳转到章节或片段，在分页与连续滚动之间
临时切换，并使用方向键和 Esc 完成对应阅读操作；跨章节导航必须使用会话声明的 spine，
不得猜测章节路径或静默回到错误位置。

## 2. 范围

### 必须完成

- `ReaderSession` 增加只读、有序 spine 映射，并由 Rust、前端 Zod 和测试使用同一契约。
- ReaderShell 使用会话令牌与 spine href 构造安全章节 URL，不再固定只加载 `frameUrl`。
- 存在非首章 `initialLocator` 时直接加载对应章节，避免 M5B 固定首章导致的 spine 不匹配。
- 新增可访问的嵌套目录抽屉；目录可打开、关闭、用 Esc 关闭并保留层级。
- 点击目录项跳转到目标 spine 和可选 fragment；关闭未选择的目录不改变位置，选择后关闭
  保留跳转位置。
- `reader:init` 增加严格校验的 `initialNavigation`，支持 `start`、`end` 和 `fragment`。
- 跨章节 `spine-request`：下一页/章使用目标章节起点，上一页/章使用目标章节末尾。
- 分页模式下左右键翻页；滚动模式下左右键滚动约一屏，上下键保留系统行滚动。
- 提供分页/连续滚动的临时切换；切换后定位到切换前文本附近。
- 目录或模式控件不会吞掉按钮、输入框、链接和可编辑区域的正常键盘行为。

### 不属于本任务

- 字号、前景色、背景色、昼夜主题、沉浸式和控件显示/隐藏；由 M5D 实现。
- 阅读模式、目录状态或沉浸式状态的持久化；由 M5D 实现。
- Locator 防抖保存、异常恢复、书签和文本复制；由 M5E 实现。
- EPUB 内部正文链接、脚注弹窗和图片放大；分别属于 V1.x 或后续任务。
- 新 Tauri command、数据库 schema/version、依赖、Router、全局状态库或 capability 修改。
- 真实用户书库、原始 EPUB 或其他个人数据修改。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收 |
|---|---|---|
| FR | FR-070, FR-071, FR-072 | 分页和滚动可用、可临时切换，切换后定位到原文本附近 |
| FR | FR-120–FR-125 | 显示 EPUB TOC、保留嵌套、随时打开、点击跳转及关闭位置规则 |
| FR | FR-200, FR-201 | 两种模式的左右键规则、上下键不拦截；Esc 优先关闭目录 |
| AC | AC-120 | 本任务完整覆盖目录层级、跳转和关闭位置 |
| AC | AC-070 | 本任务覆盖模式可用、临时切换和定位；单书持久化由 M5D 补齐 |
| AC | AC-200 | 本任务覆盖方向键和当前唯一顶层界面目录的 Esc；沉浸式顺序由 M5D 补齐 |
| NFR | NFR-010 | 阅读、导航和模式切换不修改原始文件或个人数据 |
| NFR | NFR-011 | 不发起网络请求，不加载会话外资源 |
| NFR | NFR-012 | 无效目录目标、章节读取失败和边界操作有稳定行为且不泄漏路径/令牌 |

## 4. 公开契约

### 4.1 ReaderSession spine

```ts
type ReaderSpineItem = {
  index: number
  href: string
}

type ReaderSession = {
  sessionId: string
  bookId: string
  title: string
  frameUrl: string
  spine: ReaderSpineItem[]
  toc: TocNode[]
  initialLocator: ReaderLocator | null
  initialProgression: number
  settings: ReaderSettings
}
```

- Rust `ReaderSession` 增加 `spine: Vec<ReaderSpineItem>`；DTO 使用 camelCase。
- spine 必须非空，按阅读顺序排列，`index` 从 0 开始连续递增，不得重复；`href` 必须通过
  现有 EPUB 根内相对路径校验。
- 多个 spine 项可以引用同一 `href`；它们是不同的逻辑阅读位置，不得按 href 去重或拒绝。
- `frameUrl` 仍是首章 URL，token 格式和资源路径限制不变。
- 前端 `readerSessionSchema` 必须校验 spine 顺序、TOC 所有 `spineIndex` 均在范围内，且
  `initialLocator.spineIndex + spineHref` 必须精确匹配同一个 spine 项。任何不匹配均视为
  非法会话，关闭 session 并显示稳定错误。
- 不新增独立 token 字段。token 仍只从已验证 `frameUrl` 第一段取得。

### 4.2 章节 URL

- 前端新增纯函数，以已验证 32–64 位小写十六进制 token 和 spine href 构造
  `epub://<token>/<encoded-href>`。
- 编码规则与 Rust `build_frame_url` 一致：保留 RFC 3986 unreserved 字符与 `/`，其他 UTF-8
  字节使用大写两位十六进制百分号编码。
- 构造结果仍通过完整 frame URL 校验；不得产生 query、fragment、反斜杠、NUL、绝对路径、
  空路径段或 `.`/`..` 段。
- 无效 spine href、token 或不匹配的 TOC 目标不得进入 iframe；返回稳定错误，不显示 href。

### 4.3 reader:init

协议版本保持 1，payload 扩展为：

```ts
{
  spineIndex: number
  mode: 'paged' | 'scrolling'
  initialLocator: ReaderLocator | null
  initialNavigation?:
    | { kind: 'start' }
    | { kind: 'end' }
    | { kind: 'fragment'; fragment: string }
}
```

- `initialLocator !== null` 时不得同时提供 `initialNavigation`；Zod 必须拒绝该组合。
- `spineIndex` 必须是当前章节的非负整数；ParentBridge 从 ReaderShell 当前逻辑位置传入，
  阅读脚本必须在开始恢复 Locator 或 navigation 前使用该值覆盖 HTML 注入的
  `data-spine-index`。因此重复 href 的两个 spine 项仍生成各自正确的 Locator。
- `initialLocator=null` 且无 `initialNavigation` 时从章节起点开始，保持现有行为。
- fragment 必须非空、最多 2,048 个 UTF-16 单元且不含 NUL、反斜杠或首字符 `#`。
- fragment 仅作为 DOM id 使用，不得拼接 CSS selector、写入 URL query 或日志。
- 阅读脚本在布局和导航完成后才发送 `reader:ready` 和首个 `location-changed(init)`。
- fragment 原始值找不到时允许对合法百分号编码单次解码后重试；仍找不到则回到章节起点，
  完成正常 ready，不使整章进入致命错误。

## 5. 阅读器行为

### 5.1 初始章节

- 有 `initialLocator`：按 `spineIndex` 找同一 `spineHref`，从该章节构造 iframe URL，并把
  Locator 传给 `reader:init`。
- 无 `initialLocator`：加载 `spine[0]`，发送 `{kind:'start'}` 或不提供 navigation。
- `frameUrl` 只用于令牌和契约校验，不得覆盖上述章节选择。
- `openBook` 已成功但会话 schema 无效时，必须关闭该 session，且不加载任何章节 URL。

### 5.2 目录抽屉

- 阅读器头部提供 accessible name 为“打开目录”的按钮；目录打开后改为“关闭目录”。
- 抽屉使用 `nav`/列表语义呈现 TOC，递归保留 `children` 层级；每个目录项是独立 button。
- 目录为空时显示“本书没有可用目录。”，阅读正文仍可用。
- 打开目录不发送导航消息、不重新加载 iframe、不改变当前位置。
- 未选择目录项就关闭抽屉时，恢复打开前阅读位置和目录按钮焦点。
- 选择目录项后关闭抽屉，重建目标 iframe，并在 ready 后把焦点移到阅读内容。
- 点击遮罩关闭和 Esc 关闭都遵守“未选择不移动”的规则。
- Esc 在 M5C 只关闭最上层目录；目录未打开且没有其他 M5D 界面时不执行破坏性操作。

### 5.3 目录导航

- 以目录项 `spineIndex` 从 session spine 取得唯一 href，不得根据 `frameUrl` 或书库路径猜测。
- 目标章节与当前章节相同也允许重建 iframe；M5C 统一使用重建路径，避免 fragment 被初始化
  的“回到章节起点”覆盖。
- 有 fragment 时传 `{kind:'fragment', fragment}`；无 fragment 时传 `{kind:'start'}`。
- 选择目录后不请求新 session，不重新导入书籍，不修改设置。
- 目标章节资源 404、iframe 错误或握手失败进入现有可恢复 frame-error，仍可返回书库；
  不得显示资源路径、token 或底层 message。

### 5.4 章节边界

- 阅读脚本继续在章首页/章末页或滚动边界发送 `spine-request`。
- `next` 导航到 `currentSpineIndex + 1` 并使用 `start`；`prev` 导航到
  `currentSpineIndex - 1` 并使用 `end`。
- 首章 `prev` 与末章 `next` 为无操作，不换章、不回到书首/书尾、不显示致命错误。
- 同一位置重复边界输入不得触发重复导航；旧 iframe 迟到的 message 不得覆盖当前章节。
- 导航前关闭旧 ParentBridge；新章节只能创建一个新 ParentBridge。

### 5.5 模式切换

- ReaderShell 用本地状态维护当前临时 mode，初始值来自 `session.settings.mode`。
- 提供 accessible 的“分页”和“连续滚动”切换；当前项使用 `aria-pressed` 或等价语义。
- bridge ready 后切换调用现有 `ParentBridge.updateMode(mode)`，阅读脚本按 Locator 恢复。
- 新章节加载时把当前临时 mode 作为 `initialMode`，不回到 session 初始值。
- 本任务不调用 `getEffectiveSettings`、`updateBookSettings` 或 `updateAppSettings`，不写
  localStorage/sessionStorage；关闭重开后仍使用已持久化值，直到 M5D 实现保存。

### 5.6 键盘

- 阅读脚本在 iframe 内监听 `keydown`：
  - 分页模式 ArrowLeft/ArrowRight 调用本地 turn-page。
  - 滚动模式 ArrowLeft/ArrowRight 滚动约 0.9 个视口高度。
  - ArrowUp/ArrowDown 不 `preventDefault`，保留系统行滚动。
  - 带 Ctrl/Meta/Alt 的组合键不拦截。
- 父窗口在目录关闭且焦点不在 iframe 内时，对非交互目标应用相同左右键规则。
- 任一键盘入口遇到 input、textarea、select、button、链接或 contenteditable 目标时不得拦截。
- 目录打开时左右键不翻页，Esc 关闭目录；不同时触发两条翻页路径。
- 使用空格/Enter 操作目录项和模式按钮时不得误触方向键逻辑。

## 6. 组件与生命周期

```text
session(spine) -> current chapter state
  -> buildSpineFrameUrl(token, href)
  -> iframe(key=frameGeneration, src)
  -> one ParentBridge
  -> reader:init(mode, locator | navigation)
  -> ready

TOC item / spine-request
  -> close old bridge
  -> choose target spine
  -> replace iframe
  -> build new bridge

Back / session retry
  -> existing close-once session lifecycle
```

- 章节切换只改变阅读 frame 状态，不关闭 M5A session。
- 每次 frame replacement 增加本地 generation/key；旧 iframe load、ready、location 或
  spine-request 均不得改变新 frame 状态。
- 显式返回、打开错误、reader schema 错误和卸载继续遵守 M5B close-once。
- 重建 iframe 后，在 ready 前保持“正在准备阅读内容…”，不得闪现旧章节或空 TOC 覆盖层。
- 重试整个书籍仍调用 `openBook` 创建新 session；章节边界导航不得创建新 session。

## 7. 文件权限

### 允许修改

- `crates/reader-model/src/session.rs`、`crates/reader-model/src/lib.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/tests/m5_commands.rs`
- `src/bridge/ipc-schemas.ts`
- `src/bridge/messages.ts`、`src/bridge/parent.ts`、`src/bridge/iframe.ts`
- 上述 bridge 的现有测试文件
- `src/features/reader/`
- `src/reader-engine/engine.ts`、`src/reader-engine/index.ts` 及任务专用测试
- `src-tauri/resources/reader-engine.js`（只允许由 `npm run build` 生成，禁止手工编辑）
- `src/styles.css`
- `tests/e2e/` 中与阅读器目录、模式、导航和键盘相关的现有测试/夹具
- ReaderSession 测试 fixture 所在的现有前端测试文件（仅补 `spine` 或导航字段）
- 本任务状态和第 12 节开发报告

### 禁止修改

- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、ADR
- 其他 `docs/tasks/TASK-*.md`
- `crates/storage-core/`、`crates/library-core/`、数据库 schema/version
- Tauri command 名称/参数、`epub://` 路径和资源白名单、CSP、capability、文件/网络权限
- `package.json`、锁文件、任何依赖
- 原始 EPUB、真实用户书库或个人阅读数据

若必须修改禁止路径才能实现，标记 `blocked` 并报告最小原因，不得自行扩大授权。

## 8. 测试要求

### Rust / DTO

- openBook 返回完整 spine，顺序、index、href 与 manifest/ParsedBook 一致。
- 空 spine 或非法路径在打开阶段保持失败，不创建 session。
- ReaderSession JSON 仍不含 source_path、token 独立字段或完整路径。
- frameUrl 仍指向首章且可被协议读取。

### 前端 schema

- 合法 spine、嵌套 TOC、相同 href 的不同 spine 项和首章/非首章 initialLocator 通过。
- 空 spine、非连续/重复 index、非法 href、TOC 越界 spineIndex、Locator 与 spine 不匹配
  均失败。
- `initialNavigation` 三种 kind 通过；空/超长 fragment、首字符 `#`、NUL 或反斜杠失败。
- `spineIndex` 缺失、负数、非整数或超出 session spine 范围时消息 schema 失败。
- Locator 与 navigation 同时出现失败；所有对象拒绝未知字段。
- 章节 URL 对 ASCII、中文、空格、`%` 和嵌套目录正确编码，且非法路径不会被构造。

### 目录与 ReaderShell

- 嵌套 TOC 保持父子层级和顺序，按钮名来自 label。
- 打开/遮罩关闭/Esc 关闭不重建 iframe、不改变当前章节或 mode，并恢复焦点。
- 覆盖空目录文案、目录按钮打开后的动态 accessible name、遮罩点击和关闭按钮。
- 点击同章 fragment、跨章无 fragment、跨章 fragment 都重建一次 iframe，并发送正确
  navigation；目标 URL 来自 spine。
- 非首章 initialLocator 初始加载正确章节，不加载 frameUrl 首章。
- 目录选择后关闭；未选择关闭不移动，选择后关闭不恢复旧位置。
- 章节 prev/next 使用 end/start；首末边界无操作；旧 frame 迟到消息被忽略。
- 模式按钮 ready 后调用 updateMode；切到连续滚动后导航新章节，新 bridge 的
  `initialMode` 必须仍为 scrolling；不调用设置写命令/存储。
- 返回书库仍只关闭 session 一次；章节切换不关闭 session。
- 相同 href 的相邻 spine 项导航时，新 bridge 收到新的逻辑 `spineIndex`，不会沿用首个
  href 的 index。
- DOM、console、storage 和错误提示不含 token、完整 frame URL 或底层路径。

### WebKit / 键盘 / 导航

- 分页模式左右键翻页，滚动模式左右键约一屏。
- 上下键保持默认滚动；参数化覆盖 input、textarea、select、button、链接和
  contenteditable 正文目标上均不拦截。
- Ctrl/Meta/Alt 分别与左右键组合时均不翻页。
- 目录打开时左右键不翻页，Esc 关闭目录。
- `initialNavigation=start/end/fragment` 在真实 WebKit 中分别定位章首、章末和 id 元素；
  fragment 优先使用 getElementById，非法 selector 字符或不存在 id 不会导致整章失败，
  且必须回到章首并正常 ready。
- 分页模式下，fragment 位于一页内部非列首位置时仍定位到该页；目标接近章节末页时定位到
  包含目标的页，而不是其后的空白页。
- 嵌套 TOC 测试必须断言子项位于父项对应的 `<li>` 内，而不只是同时出现在抽屉中。

### 必跑命令

```bash
npm run check
npm run test:e2e
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri build
```

## 9. 禁止事项

- 不实现 M5D/M5E 功能，不持久化 mode、主题、字号、进度、书签或沉浸式状态。
- 不新增依赖、Router、Zustand 或其他状态库。
- 不修改 `epub://` 安全边界或让前端自行读取 ZIP。
- 不把 TOC fragment 拼进 CSS selector 或传入 `querySelector`。
- 不以 reload 整个应用、创建新 openBook session 代替章节导航。
- 不在日志、DOM 文本、存储或错误页暴露 token/frame URL/完整本地路径。
- 不提交、推送或发布。

## 10. 已知风险与实现约束

- WebKit iframe 内的 keydown 不会冒泡到父窗口；阅读脚本和 ReaderShell 各自处理自身焦点域，
  测试必须覆盖二者且避免重复翻页。
- 同章目录跳转在 M5C 通过重建 iframe 保持单一可靠路径；若后续性能不可接受，只允许在
  SDS 修订后增加章节内 go-to 协议，不得在本任务自行扩展消息类型。
- 页码、滚动位置和 Locator 只在内存中使用；真正的持久化/异常恢复归 M5E。
- 真实 `epub://` 跨章节加载仍属于 WKWebView 集成风险，E2E 与最终 M6 应用验收都需保留证据。

## 11. 交接要求

开始实现时将状态改为 `in_progress`。完成后：

1. 将状态改为 `ready_for_review`。
2. 填写第 12 节完成摘要、修改文件、真实命令与退出码。
3. 按 FR/AC 列出证据；未验证项和已知风险不得省略。
4. 明确是否修改禁止路径、是否新增依赖、是否创建 commit。

## 12. DeepSeek 开发报告

> 仅在开始实现后填写。

**完成摘要：**

实现嵌套目录、章节导航、初始 navigation、模式切换与键盘，并把 spine 纳入公开会话契约。

**第 13 节返工复验：**

- **重复 spine href**：`readerSessionSchema` 不再拒绝重复 href（同一 manifest 资源允许
  多个 itemref；仍要求 index 从 0 连续、href 合法）；`reader:init` 新增逻辑
  `spineIndex`（必填；缺失/负数/非整数均拒绝），ReaderShell 传当前逻辑位置，
  ParentBridge 透传、IframeBridge 传给引擎；engine 在初始化导航/Locator 前用该值覆盖
  协议 HTML 注入的 `data-spine-index`，避免重复 href 时报告错误章节位置。Rust `m5_commands` 使用重复 itemref（两个 spine 项同
  href）验证 openBook 保留两个逻辑项；Vitest 覆盖重复 href 通过、spineIndex 消息校验和
  bridge 传递；`npm run build` 重新生成 `reader-engine.js`（构建产物，未手工编辑）。
- **补充关键测试**：
  - WebKit E2E 9 项（原 4 + 新增 5）：分页 fragment 定位（含非列首页与末页附近）、
    缺失 fragment 回章首且正常 ready、滚动模式交互目标（input/textarea/select/链接/
    contenteditable）左右键不翻页、Meta/Alt 组合键不翻页；
  - Vitest：空目录文案、动态 accessible name（打开目录/关闭目录/关闭目录遮罩）、
    遮罩点击与关闭按钮关闭（iframe/bridge/session 不变）、嵌套父子 DOM 层级、临时
    scrolling 模式继承到下一章节、章节切换不关闭 session；
- **MINOR**：`src-tauri/resources/reader-engine.js` 为 `npm run build` 生成的阅读脚本
  产物（`build:reader`），本任务未手工编辑；报告与提交范围需一并说明。

- **Rust spine**：`ReaderSpineItem { index, href }`；`ReaderSession.spine` 按
  `ParsedBook.spine` 顺序构造（index 从 0、href 为 OPF 根内相对路径），DTO 为
  camelCase。openBook 响应仍不含 token/source_path。
- **前端 schema**：`readerSessionSchema` 新增 `spine`（非空、index 从 0 连续、href
  走严格相对路径校验；允许同一 href 的多个逻辑项），并要求 frameUrl 指向 spine 首项、
  TOC 所有 `spineIndex` 在范围内、`initialLocator.spineHref` 精确匹配同一 spine 项；
  任何不匹配视为非法会话并 closeBook。
- **章节 URL**：`buildSpineFrameUrl(token, href)` 按 Rust `build_frame_url` 规则编码
  （RFC3986 unreserved + `/`，其他 UTF-8 字节大写百分号），构造结果再经严格 frame URL
  校验；非法 href/token 返回 null，不进入 iframe。
- **ReaderShell**：
  - 以 `initialLocator.spineIndex` 选择初始章节（非首章直接加载对应 spine href），
    `frameUrl` 只用于 token 和首项校验；无 locator 时加载 spine[0]；
  - 目录抽屉：accessible name “打开目录/关闭目录”，`role="dialog"` + `nav` + 递归
    `ul/li` 保留层级；空目录显示“本书没有可用目录。”；打开不重建 iframe、不改变位置；
    遮罩/关闭按钮/Esc 关闭未选择时保持位置并恢复目录按钮焦点；选择后关闭并在 ready
    后把焦点移到阅读内容；
  - 目录项按 spineIndex 选章节：同章/跨章有 fragment 传 `{kind:'fragment'}`，无
    fragment 传 `{kind:'start'}`，统一重建 iframe；旧 bridge 先关闭，旧 iframe 迟到
    ready/spine-request 由 `iframeRef.current` 守卫忽略；
  - spine-request：next → 下一章 + start，prev → 上一章 + end；首末边界无操作；
    章节切换不关闭 session、不创建新 session；
  - 模式：本地临时 mode，初始来自 session.settings.mode；分页/连续滚动按钮
    `aria-pressed`，bridge ready 后调用 `updateMode`，新章节以当前临时 mode 作为
    initialMode；不调用设置写命令或存储；
  - 键盘：父窗口在目录关闭且目标非交互元素时左右键调用 `turnPage`；目录打开时左右键
    不翻页；Esc 关闭目录；Ctrl/Meta/Alt 不拦截；输入框/按钮/链接/contenteditable 不
    拦截；
  - 重试仍按 M5B 契约重新 openBook（新 session），章节导航不重新 openBook。
- **reader:init/navigation**：messages 增加 `initialNavigation`（start/end/fragment，
  fragment 非空≤2048、无 NUL/反斜杠、不以 # 开头），Zod 拒绝 locator+navigation
  并存；ParentBridge 透传；IframeBridge 传给 engine。
- **engine**：start 支持 navigation：
  - start 回章首、end 到章末（分页末页/滚动底部）、fragment 用
    `getElementById`（原值一次，失败单次 `decodeURIComponent` 重试，仍失败回章首）；
  - fragment 场景在 `fonts.ready` 后先 relayout、再精确定位，然后发送 ready 和首个
    location-changed(init)，避免字体加载导致定位偏移；
  - 键盘：iframe 内左右键（分页翻页、滚动约 0.9 屏），上下键不 preventDefault，
    修饰键与交互目标不拦截。
- **测试**：Vitest 122 tests（schema 3、章节 URL 3、ReaderShell 27、messages/iframe
  navigating/spineIndex 断言）；WebKit E2E 42 passed（fragment/end/缺失 fragment、分页
  fragment 与末页附近、inline 非列首、滚动键盘+上下键+Ctrl、交互目标、Meta/Alt、
  分页翻页、重复 href 逻辑 spineIndex）；Rust workspace 283 passed/1 ignored
  （m5_commands 断言重复 href 的两个 spine 项）。

**修改文件：**

- `crates/reader-model/src/session.rs`、`src/lib.rs`
- `src-tauri/src/commands.rs`、`src-tauri/tests/m5_commands.rs`
- `src/bridge/ipc-schemas.ts`、`ipc-schemas.test.tsx`（新增）
- `src/bridge/messages.ts`、`parent.ts`、`iframe.ts`、`messages.test.tsx`、
  `iframe.test.tsx`
- `src/features/reader/ReaderShell.tsx`、`session-navigation.ts`（新增）、
  `session-navigation.test.tsx`（新增）、`reader-shell.test.tsx`
- `src/reader-engine/engine.ts`
- `src/styles.css`
- `tests/e2e/reader-navigation.spec.ts`（新增）
- `docs/tasks/TASK-M5C.md`（状态与本第 12 节）

构建产物 `src-tauri/resources/reader-engine.js` 由 `npm run build` 重新生成（未手改）。
未修改 SRS/SDS/DEVELOPMENT_PLAN/ADR/其他任务、`crates/storage-core`、
`crates/library-core`、命令名称/参数、`epub://` 白名单、CSP、capability、依赖；
未创建 commit。

**关键设计选择：**

- 章节切换只重建阅读 frame（bridge 关闭 → 新 iframe → 新 bridge），不关闭 session；
  旧回调通过当前 iframe 引用守卫，避免迟到消息覆盖新章节。
- 目录选择后统一重建 iframe（同章也重建），fragment 通过 `initialNavigation` 传给
  引擎，避免被初始化“回章首”覆盖。
- fragment 定位等待 `fonts.ready` 后 relayout 再精确定位，再 ready；仅 fragment 场景
  延迟 ready，其他场景保持原时机。
- 父窗口与 iframe 各自处理焦点域内键盘，避免同一次按键触发两条翻页路径；目录打开时
  父窗口左右键不处理。
- 临时 mode 只存 React state，不调用设置命令、不写存储。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | `tsc -b`、Biome、Vitest 13 文件 122 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 42 passed |
| `npm run build` | 0 | `build:reader` + Vite 成功（JS 422.13 kB） |
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 283 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.79 MiB |

**AC 验证：**

- FR-070/FR-071/FR-072：分页/滚动可用；左右键与临时模式切换；新章节继承 mode；
  持久化留 M5D。
- FR-120–FR-125：嵌套目录展示、随时打开、跳转并关闭；未选择关闭不改变位置。
- FR-200/FR-201：两种模式左右键规则、上下键不拦截、Esc 关闭目录。
- AC-120：目录层级、跳转与关闭位置（Vitest + WebKit 导航）。
- AC-070：模式可用与临时切换定位（单书持久化 M5D）。
- AC-200：方向键与目录 Esc；沉浸式顺序留 M5D。
- NFR-010：只读会话与原文件；导航不写个人数据。
- NFR-011：E2E 仅本地 route，无网络；不加载会话外资源。
- NFR-012：无效 TOC 目标/schema 失败关闭 session 并显示固定错误；不泄漏路径/token。

**未验证项：**

- 真实 Tauri WKWebView 中 `epub://` 跨章节 iframe 加载与 ReaderShell 目录/模式 UI 的
  端到端（本次 WebKit E2E 使用独立 iframe harness 与 mock IPC；ReaderShell 交互由
  Vitest fake bridge 覆盖）。重复 href 的跨章节真实 iframe 也在此限制内（已由 Rust
  spine 与前端 schema/engine 参数覆盖）。
- 真实用户书库、真实 EPUB 的目录层级与 fragment 定位容差。
- Meta/Alt + 左右键已在 WebKit E2E 覆盖（Meta+ArrowRight、Alt+ArrowLeft 不滚动）；
  目录打开时遮罩/Esc 的 E2E 仍未做（Vitest 覆盖 window keydown 与遮罩按钮）。
- 章节资源 404/握手失败的真实 `epub://` 路径（由 frame-error 状态与 E2E 非法 frame
  URL 近似覆盖）。

**风险与待决问题：**

- 同章目录跳转通过重建 iframe 实现，可能带来短时加载与焦点跳动；若性能不可接受，需
  SDS 修订后增加章节内 go-to，本任务未扩展消息类型。
- fragment 定位依赖 `fonts.ready` 与布局稳定；复杂字体/图片书籍可能有像素级偏差；
  仅 fragment 场景延迟 ready。
- 父窗口与 iframe 键盘规则分离，焦点在 iframe 时父窗口不处理；若未来出现跨域焦点
  事件差异，需要补充集成测试。
- `src-tauri/resources/reader-engine.js` 是构建产物，测试前需执行 `npm run build`
  （本任务门禁已包含）。
- E2E harness 依赖自定义 route + 固定视口，和真实 Tauri 协议栈仍有差异。

**依赖与禁止路径：**

- 未新增依赖，未修改 `package.json`/锁文件。
- 未修改禁止路径；仅按 `npm run build` 重新生成阅读脚本产物。
- 未创建 commit/push。

## 13. Codex 审查

> 仅在 DeepSeek 标记为 `ready_for_review` 后填写。

**结论：** 通过

**审查日期：** 2026-11-15

**独立执行的检查：**

- `npm run check`：通过；13 files、122 tests，TypeScript、Biome、Vitest 均通过。
- `npm run test:e2e`：通过；WebKit 42 passed。
- `npm run build`：通过；主应用和阅读脚本构建成功。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：283 passed、0 failed、1 ignored。
- `npm run tauri build`：通过；生成 `Qurner.app`，7.79 MiB。
- `git diff --check`：通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| 无 | — | 未发现 `BLOCKER` 或 `MAJOR` 问题。 | — |

**AC 结果：**

- FR-070/FR-071/FR-072：分页/滚动、临时切换和定位恢复通过；模式持久化按任务范围留 M5D。
- FR-120–FR-125：嵌套目录、空目录、遮罩/关闭按钮、fragment 跳转和关闭不导航通过；
  重复 href 的两个逻辑 spine 项通过 ReaderShell 与 WebKit 覆盖。
- FR-200/FR-201：分页/滚动方向键、交互目标、Meta/Alt 和目录 Esc 通过；沉浸式外层
  Esc 顺序留 M5D。
- AC-120：通过。
- AC-070：本任务范围内的临时模式切换与定位通过；单书持久化属于 M5D。
- AC-200：本任务范围内的键盘行为通过；沉浸式顺序属于 M5D。
- NFR-010/011/012：未发现原始文件写入、网络或敏感信息泄漏，基础检查通过。

**返工或后续任务：**

- `TASK-M5C` 通过验收，状态改为 `accepted`；未创建 commit。
- 真实 Tauri `epub://` 跨章节加载、真实用户 EPUB 目录容差和最终应用冒烟仍由 M6
  以及后续真实应用验收覆盖，不阻塞本任务。
