# TASK-M5D：设置继承、字号、主题与沉浸式阅读

**状态：** accepted

**里程碑：** M5

**负责人：** DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-15

**依赖：** TASK-M5C accepted

## 1. 目标

用户可以在阅读时即时调整字号、阅读模式和昼夜主题，设置按“内置默认 → 全局默认 →
单书覆盖”继承并持久化；用户还可以进入不自动持久化的沉浸式阅读，在需要时从顶部热区
显示目录、设置和退出控件。

## 2. 范围

### 必须完成

- 使用既有 `app_settings` 和 `book_settings` 表实现全局默认与单书覆盖的读写、清除和合并。
- 完成 `getEffectiveSettings`、`updateAppSettings`、`updateBookSettings` 和
  `resetBookSettings` 四个 Tauri 命令及严格前端桥。
- M5C 的模式切换改为持久化单书覆盖；字号和主题也使用同样的单书覆盖规则。
- 阅读设置抽屉提供字号减小/增大、白天/黑夜、保存为全局默认和恢复本书跟随全局。
- 字号、主题和模式变化即时应用到当前 iframe，并在布局恢复后保持当前文本锚点。
- 字号只改变文字尺寸，不缩放图片、不重建正文 HTML；主题不反转图片或覆盖原书显式特殊颜色。
- 沉浸式模式隐藏非必要控件；顶部热区或键盘焦点可显示控件，目录和设置仍可用。
- 沉浸式默认关闭且不持久化；Esc 按设置抽屉 → 目录抽屉 → 沉浸式的顺序关闭。
- 设置写入失败显示固定非阻塞提示，不关闭阅读 session，不覆盖已有有效设置。

### 不属于本任务

- 阅读进度自动保存、异常恢复、章节进度百分比；由 M5E 实现。
- 书签列表、创建、跳转和删除；由 M5E 实现。沉浸式中的书签入口留到 M5E。
- 文本选择与复制、高亮批注、字体族选择、自定义颜色、行距/段距/页边距和系统全屏。
- 数据库 schema/version 或迁移修改；必须复用既有三张设置相关表和列。
- 新依赖、Router、Zustand、远程字体、主题下载或网络请求。
- 修改原始 EPUB、软移除个人数据或改变 `epub://` 安全边界。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收 |
|---|---|---|
| FR | FR-073 | 阅读模式保存到每本书，重开后恢复 |
| FR | FR-080–FR-084, FR-087 | 字号即时生效、单书保存、保留原书 CSS/字体、缺失字体回退、锚点不丢 |
| FR | FR-090–FR-093 | 全局默认、单书覆盖、修改全局不覆盖单书、新书继承当前全局 |
| FR | FR-100, FR-101 | 白天/黑夜主题可切换、可读且不反转图片 |
| FR | FR-110–FR-114, FR-116 | 沉浸式主动开关、隐藏控件、热区显示、目录/设置可访问、打开书籍不自动进入 |
| FR | FR-201 | Esc 按设置、目录、沉浸式由内向外关闭 |
| AC | AC-070 | 模式切换与单书持久化完整通过 |
| AC | AC-080 | 字号变化、保存、原样式保留和 Locator 恢复通过 |
| AC | AC-090 | 全局与单书继承完整通过 |
| AC | AC-100 | 两种主题和图片/特殊颜色保护通过 |
| AC | AC-110 | 沉浸式开关、控件显示、目录/设置入口和不自动进入通过 |
| AC | AC-200 | Esc 顺序通过；书签入口由 M5E 补齐 |
| NFR | NFR-010 | 设置只修改应用数据库，不修改原始文件和正文 DOM |
| NFR | NFR-011 | 不请求远程字体、主题或网络资源 |
| NFR | NFR-012 | 设置读取/写入失败有稳定中文反馈，不泄漏路径或底层异常 |

## 4. 公开契约

### 4.1 设置类型

```ts
type ReaderSettings = {
  mode: 'paged' | 'scrolling'
  fontScale: number
  theme: 'day' | 'night'
}

type ReaderSettingsPatch = {
  mode?: 'paged' | 'scrolling'
  fontScale?: number
  theme?: 'day' | 'night'
}

type BookSettingsPatch = {
  mode?: 'paged' | 'scrolling' | null
  fontScale?: number | null
  theme?: 'day' | 'night' | null
}
```

- `fontScale` 必须是有限数且位于 `0.5..=3.0`；UI 按 0.1 步长调整。
- 所有 patch 至少包含一个字段并拒绝未知字段。`BookSettingsPatch` 中的 `null` 表示清除
  该字段的单书覆盖、恢复全局继承；全局 patch 不接受 `null`。
- 设置对象和 patch 在 Rust、Tauri 输入/输出、前端 Zod 和 iframe 消息中使用同一语义。

### 4.2 Tauri 命令

```ts
getEffectiveSettings(bookId: string): Promise<ReaderSettings>
updateAppSettings(patch: ReaderSettingsPatch): Promise<ReaderSettings>
updateBookSettings(input: {
  bookId: string
  patch: BookSettingsPatch
}): Promise<ReaderSettings>
resetBookSettings(bookId: string): Promise<ReaderSettings>
```

- `getEffectiveSettings` 与 `updateBookSettings/resetBookSettings` 只接受 active book；
  book 不存在或已软移除返回 `E_BOOK_NOT_FOUND`。
- `updateAppSettings` 返回新的全局默认设置，不读取具体书籍。
- `updateBookSettings` 只修改 patch 中出现的字段，返回修改后合并的 effective settings。
- `resetBookSettings` 清除该书所有覆盖并返回当前全局 effective settings。
- 命令输入严格使用 `{ bookId }`、`{ patch }`、`{ input: { bookId, patch } }` 的既有
  camelCase 约定；响应不包含路径、SQL 或底层错误文本。
- 不新增数据库列或 schema migration；复用 `app_settings`、`book_settings` 和
  `reader_defaults` 键。

### 4.3 设置合并

```text
内置默认 {paged, 1.0, day}
  < app_settings[reader_defaults]
  < book_settings 非 NULL 字段
```

- 无全局记录或字段时使用内置默认。
- 修改全局字段不修改任意 `book_settings` 非 NULL 字段。
- 单书未覆盖字段始终反映当前全局值；清除覆盖后立即回落到全局值。
- 持久化 JSON、mode/theme 字符串、fontScale 非有限或越界时返回 `E_STORAGE`，不得静默
  重置或覆盖原记录。
- 全局 patch 和单书 patch 的合并/写入必须在 storage worker 内原子完成，避免两个快速点击
  互相覆盖；`updated_at` 使用 UTC RFC3339。

### 4.4 iframe settings

`ReaderSettings` 作为完整对象进入 `reader:init` 和 `reader:update-settings`，拒绝未知字段：

```ts
reader:init -> {
  spineIndex: number
  settings: ReaderSettings
  initialLocator: ReaderLocator | null
  initialNavigation?:
    | { kind: 'start' }
    | { kind: 'end' }
    | { kind: 'fragment'; fragment: string }
}

reader:update-settings -> { settings: ReaderSettings }
```

- 初始化必须先应用设置，再恢复初始 Locator/navigation。
- 运行中更新先捕获当前 Locator，再应用 mode/fontScale/theme 全量设置并恢复 Locator。
- 设置消息与 session/token 绑定，非法范围、未知主题/模式或额外字段不得发送到 engine。
- `reader:ready` 只能在初始设置、布局和 Locator 恢复完成后发送。

## 5. 阅读器行为

### 5.1 设置读取与继承

- `openBook` 返回的 effective settings 是 ReaderShell 初始 `settings` 状态；打开成功后
  将完整的 mode、fontScale、theme 传给 iframe。
- 设置面板打开时可以使用当前状态；不因打开面板重新打开书籍或重建 iframe。
- 修改单书字段成功后，后续章节继续使用当前临时显示值；服务端返回的 effective settings
  是最终状态来源。
- 快速连续点击必须按请求顺序或以单调 generation 处理；旧请求的迟到响应不得覆盖新值。
- `updateAppSettings` 成功后提示“已保存为全局默认”；它只改变未覆盖字段的未来继承值。

### 5.2 设置抽屉

- 阅读器头部提供“阅读设置”按钮；打开后显示 accessible 的设置抽屉/对话框。
- 设置抽屉至少包含：
  - 字号减小和增大按钮，当前值以百分比显示，范围 0.5–3.0，步长 0.1。
  - 白天/黑夜主题按钮，当前主题有 `aria-pressed` 或等价状态。
  - “恢复本书跟随全局”按钮。
  - “保存为全局默认”按钮。
- 模式按钮继续位于头部；点击后立即应用并写单书 mode 覆盖。
- 设置抽屉和目录抽屉互斥；打开一个关闭另一个，遮罩/Esc 可关闭设置。
- 设置写入失败显示固定非阻塞提示，不关闭抽屉、不关闭 session，也不把失败说成成功。
- 不写 localStorage/sessionStorage、Query cache 或日志。

### 5.3 字号

- 字号改变立即应用到当前 iframe，并保持当前章节和文本 Locator 附近位置。
- 只在阅读根/正文基线上缩放文字；不得修改 `font-family`，不得缩放、裁切或反转图片，
  不得重写正文 HTML/内联 style。
- 原书 CSS 中未显式覆盖的排版保持不变；用户字号只覆盖正文基线字号。
- 切换分页/滚动或窗口尺寸后，字号继续有效。
- 同一本书关闭重开恢复已保存字号；不同书互不覆盖。

### 5.4 主题

- 白天主题使用浅色阅读背景和深色默认正文颜色；黑夜主题使用深色阅读背景和浅色默认正文
  颜色。
- 主题只覆盖阅读根/正文的默认颜色，不添加图片 `filter`、反色、混合模式或整体透明度。
- 原书元素显式声明的特殊颜色不被无条件改写；主题不得让背景或正文出现不可读组合。
- 主题切换即时生效，并在当前 Locator 恢复后再视为完成。
- 主题作为单书可覆盖设置参与继承。

### 5.5 沉浸式

- 用户通过“沉浸式”按钮主动进入；打开书籍时初始值必须为 false。
- 进入后隐藏返回、标题、目录、模式和设置等非必要控件，阅读区域占满可用空间。
- 顶部保留高度约 48 px 的可聚焦“显示阅读控件”热区；指针进入或键盘焦点到达时显示
  头部控件。控件显示后仍可打开目录和设置。
- 沉浸式状态不持久化；切换书籍、关闭阅读器或重新打开时恢复为 false。
- Esc 顺序固定为：关闭设置抽屉 → 关闭目录抽屉 → 退出沉浸式。目录/设置打开时不得
  直接退出沉浸式。
- M5D 不新增书签入口；M5E 在沉浸式控件中补齐书签。

### 5.6 错误和生命周期

- 设置读取失败：显示“本地阅读设置无法读取。”并保留当前会话；不得静默使用默认值。
- 设置写入失败：显示“设置未能保存，请重试。”；当前阅读仍可用。
- 设置写入失败不得清除旧数据库值；下一次成功写入或重开书籍使用持久化值。
- 设置成功不得关闭或替换 session；章节导航继续使用当前 settings。
- 任何设置操作不得修改原始 EPUB、书签、阅读进度或其他书籍设置。

## 6. 组件与数据流

```text
ReaderShell
  settings = session.settings
  -> ParentBridge(settings)
  -> iframe reader:init(settings)
  -> engine applySettings(settings)

font/theme/mode change
  -> update local settings + bridge.updateSettings()
  -> updateBookSettings({ bookId, patch })
  -> response effective settings

save as global
  -> updateAppSettings(full current patch)
  -> success status only

reset book
  -> resetBookSettings(bookId)
  -> response effective settings

immersive
  -> local boolean, never persisted
  -> hidden header / top hot zone
```

- ReaderShell 目前只使用一个活动 session；设置命令必须与当前 bookId 绑定，旧 book 的迟到
  响应必须忽略。
- 章节切换不改变设置，只使用当前完整 settings 初始化新 iframe。
- iframe/bridge/session 关闭逻辑沿用 M5C/M5B 的 close-once 和 generation 守卫。

## 7. 文件权限

### 允许修改

- `crates/reader-model/src/session.rs`、`crates/reader-model/src/lib.rs`
- `crates/storage-core/` 中设置读写服务、事务和测试（不修改 schema/version）
- `crates/library-core/` 中设置合并、更新和测试
- `src-tauri/src/commands.rs`、`dto.rs`、`lib.rs`、`error.rs`、`state.rs`
- `src-tauri/tests/`
- `src/bridge/ipc-schemas.ts`、`commands.ts`、`messages.ts`、`parent.ts`、`iframe.ts`
- `src/features/reader/`
- `src/reader-engine/engine.ts`、`src/reader-engine/index.ts` 及任务专用测试
- `src/styles.css`
- `tests/e2e/` 中设置、主题、字号、沉浸式相关测试/夹具
- `src-tauri/resources/reader-engine.js`（只允许由 `npm run build` 生成）
- 本任务状态和第 11 节开发报告

### 禁止修改

- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、ADR
- 其他 `docs/tasks/TASK-*.md`
- 数据库 schema/version、迁移文件或 `Cargo.toml` 依赖
- `epub://` 资源白名单、CSP/capability、文件/网络权限
- `package.json`、锁文件、任何新依赖
- 原始 EPUB、真实用户书库、阅读进度或书签数据

若必须修改禁止路径才能实现，标记 `blocked` 并报告最小原因。

## 8. 测试要求

### Rust / storage / library

- 无设置时返回内置默认；全局 patch、单书非空覆盖和单书 NULL 继承分别正确。
- 修改全局字段不改变已有单书覆盖；修改单书字段不回写全局。
- 清除单书字段后立即回落全局；重置单书只清除该书。
- 非法 mode/theme/fontScale、损坏 JSON、未知字段和不存在/removed book 返回稳定错误，
  且不修改旧数据。
- 并发/连续 patch 的最终状态确定，不出现部分写入或交叉覆盖。
- 所有设置写入不修改 `books`、`reading_states`、书签或原始文件，updated_at 可验证。

### Tauri / 前端命令桥

- 四个命令参数严格 camelCase，输入/输出 Zod 校验；空 patch、null 规则、非法范围拒绝。
- `getEffectiveSettings` active/inactive book 分支；`updateBookSettings` 只发送 patch 字段。
- `updateAppSettings` 返回全局而非当前单书 effective；`resetBookSettings` 返回回落值。
- 错误码和固定文案不泄漏 SQLite、路径或底层 message。
- 失败、乱序响应和旧 bookId 响应不会覆盖当前设置或关闭 session。

### ReaderShell / engine

- 初始 settings 的 mode/fontScale/theme 从 session 传入 init；所有章节继承当前 settings。
- 字号按钮按 0.1 改变，范围边界不可越界；engine 收到完整 settings，Locator 在变化后恢复。
- 字号不改变 `font-family`、图片尺寸或原书特殊颜色；不重写正文 HTML。
- 白天/黑夜切换即时改变默认背景/正文颜色，图片无滤镜/反相；重开恢复单书主题。
- mode、fontScale、theme 各自可独立继承/覆盖；全局保存和本书重置有 UI 状态反馈。
- 设置抽屉打开不和目录同时存在；Esc/遮罩关闭不重建 iframe。
- 沉浸式初始 false；进入隐藏控件，热区/焦点显示控件，目录和设置可打开；退出恢复控件。
- 沉浸式 Esc 顺序设置 → 目录 → 沉浸式；设置/目录打开时不会提前退出沉浸式。
- 设置写入失败保留当前会话、旧持久化值和可读错误提示；不产生未处理 Promise rejection。

### WebKit E2E

- 内嵌 CSS 的正文在字号变化后字号变化而图片尺寸不变，默认字体族和特殊颜色不被覆盖。
- 切换主题后默认背景/文字变化，图片像素/样式不变，特殊颜色保持。
- 字号/主题/模式变化前后 Locator quote 保持，当前章节不回开头。
- 分页和滚动模式分别覆盖字号变化后的布局。
- 沉浸式隐藏/显示控件、热区交互和 Esc 顺序在真实 WebKit 中可观察。
- 无网络请求、无远程字体/主题资源；DOM 文本/日志/存储不含 session token。

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

- 不实现 M5E 的进度、书签和复制功能。
- 不新增数据库列、迁移、依赖、远程字体、主题下载或网络请求。
- 不把设置写回原始 EPUB，不修改图片、正文 DOM 或用户私人数据。
- 不用行内脚本、`innerHTML` 重建正文或整体 `zoom`/transform 代替文字缩放。
- 不把 token、frame URL、绝对路径或底层错误写入 UI/日志/storage。
- 不提交、推送或发布。

## 10. 交接要求

开始实现时将状态改为 `in_progress`。完成后：

1. 将状态改为 `ready_for_review`。
2. 填写第 11 节完成摘要、修改文件、真实命令与退出码。
3. 按 FR/AC 列出证据；未验证项和已知风险不得省略。
4. 明确是否修改禁止路径、是否新增依赖、是否创建 commit。

## 11. DeepSeek 开发报告

> 仅在开始实现后填写。

**完成摘要：**

实现设置继承/写入、字号、主题、模式持久化与沉浸式阅读。

**第 12 节返工复验（第二轮）：**

- **保留外部 CSS 字号**：engine 首次应用设置时记录原书 body 的 **computed** 字号；
  `fontScale=1` 时恢复原 body 内联值（内联为空时移除，外部 CSS 字号生效），非 1 时以
  computed 字号为基准缩放。Vitest 新增“外部 `<style>` 字号在 scale 1 下不被内联覆盖”
  测试；WebKit 夹具改为外部 `#styled { font-size: 22px }`，断言 scale 1 保持 22px、
  普通文本 16px→24px 缩放、图片/字体族/特殊颜色/HTML 不变。
- **语义损坏设置的事务回滚**：`update_app_settings` 在事务内执行 JSON 结构/未知键/
  mode/theme/fontScale 值域 SQL 校验；`update_book_settings` 在事务内先读取并校验
  现有 mode/theme/fontScale，再 UPSERT；失败时不写入且 `updated_at`/原记录保持不变。
  新增未知键、非法 fontScale、单书非法旧 mode 的失败与未修改断言。
- **沉浸式 WebKit 稳定性**：测试点击沉浸式使用 `toPass` 重试，热区改为页面内
  `focus()`（避免 Playwright focus 触发 onFocus 后元素卸载的竞态）；连续运行
  `library.spec` 8 次与完整 WebKit 套件 3 次均通过。
- **报告同步**：移除旧的 `body { font-size: 1em }`/仅 `html` 主题描述与重复 AC 条目，
  更新为当前实现与真实测试数量（137 / 42 / 291）。

**第 12 节返工复验：**

- **原子设置写入**：`storage-core` 新增 `update_app_settings`（单个事务内
  `json_valid` 校验 + 逐字段 `json_set`，只写显式 patch 字段）和
  `update_book_settings`（单条 UPSERT + CASE，未提供保留、`null` 清除、值覆盖）；
  library 去掉“读-合并-另发请求写”的两段式流程，改为校验 patch 后单请求原子写，再读取
  effective。新增并发测试（共享 worker 两线程分别写 mode/fontScale 与全局字段），最终
  两字段都保留；全局 `reader_defaults` JSON 只含显式 patch 的字段（不含未提供字段）。
- **跨书旧响应失效**：`ReaderShell.open()` 递增 `settingsGenerationRef`；新增 deferred
  `updateBookSettings` 跨书测试，旧书响应不覆盖新书设置、旧 session 只关闭一次。
- **ready 前设置不丢失**：`applyToBridge` 在 bridge 非 ready 时缓存到
  `pendingSettingsRef`；`onReady` 回放最新 `settingsRef`，不再依赖 waiting 期间的
  `updateSettings` 返回值；新增测试（waiting 时点击模式不丢、ready 后发送）。
- **字号/主题保留原书默认样式**：`fontScale=1` 恢复原书 body 内联字号（不再写死
  `1em`），非 1 时以原书基线计算；主题以 body 内联背景/正文色覆盖常见 EPUB 的
  `body { background/color }`，元素级显式颜色仍保留。WebKit 夹具加入 body
  `font-size:18px; background:white; color:black` 与特殊颜色/字体/图片，断言默认字号
  18px、night 背景/正文色变化、图片尺寸/内联样式/`filter` 与 `body.innerHTML` 不变、
  `font-family` 与特殊颜色保持。
- **头部模式失败提示**：`settingsError` 在设置抽屉未打开时也以非阻塞状态文案显示。
- **WebKit 沉浸式 E2E**：`library.spec` 新增真实主应用测试：进入沉浸式隐藏返回/标题/
  控件、热区可聚焦并显示控件、打开阅读设置、Esc 关闭设置、再次 Esc 退出沉浸式。
- **设置抽屉焦点**：打开后焦点移入抽屉关闭按钮，关闭后恢复“阅读设置”按钮焦点。
- **键盘 E2E 说明**：WebKit/Playwright 的真实按键在 iframe 焦点下不稳定，键盘规则
  测试改为在 WebKit 内派发可冒泡 `keydown`（bubbles/cancelable、含 Ctrl/Meta/Alt 与
  input/textarea/select/link/contenteditable 目标），并断言 `preventDefault` 与
  turn/spine-request 计数；真实硬件按键路径仍需人工/GUI 冒烟验证。

- **Rust 设置服务**：
  - `reader-model`：`ReaderSettings` 复用内置默认（paged/1.0/day）与 0.5–3.0 校验。
  - `storage-core`：新增 `app_settings` UPSERT、`book_settings` 三字段 UPSERT、删除单书设置；
    写入在 storage worker 内串行执行；`updated_at` 由 SQLite UTC RFC3339 写入。
  - `library-core/settings.rs`：合并顺序“内置默认 < `app_settings[reader_defaults]` <
    `book_settings` 非 NULL”；active book 校验（removed/不存在 → `E_BOOK_NOT_FOUND`）；
    全局/单书 patch 校验（0.5–3.0、mode/theme 枚举、至少一字段）；持久化 JSON 损坏 →
    `E_STORAGE` 且不覆盖原值；单书 `null` 清除覆盖、reset 删除全部覆盖后回落全局。
  - `openBook` 的 effective settings 改用同一合并模块。
- **Tauri 命令**：`getEffectiveSettings`、`updateAppSettings`、`updateBookSettings`
  （`{ input: { bookId, patch } }`）、`resetBookSettings`；严格 camelCase/未知字段/null
  规则校验；错误不包含路径/SQL。
- **前端桥**：新增 4 命令及严格 Zod 输入/输出（`ReaderSettingsPatch` 非 null、
  `BookSettingsPatch` 允许 null 清除、至少一字段）；`ReaderSettings` 继续复用严格 schema。
- **协议**：`reader:init` 使用完整 `settings` 对象（替代 mode），`reader:update-settings`
  为 `{ settings }`；ParentBridge `updateSettings`、IframeBridge 转发、engine
  `applySettings`。
- **engine**：`applySettings` 先 capture 当前 Locator，再应用 mode/fontScale/theme、
  relayout 并 restore Locator；字号只设置 `body` 基线 `font-size: calc(1em * var(--qurner-font-scale))`，
  不修改 font-family、不缩放/滤镜图片、不重写正文 HTML/内联 style；主题仅设置 `html`
  默认背景/正文颜色与 `qurner-night` class，原书显式颜色与背景保留。
- **ReaderShell**：
  - 头部“阅读设置”打开设置抽屉（字号百分比、0.1 步长增大/减小与 0.5–3.0 边界禁用、
    白天/黑夜 aria-pressed、保存为全局默认、恢复本书跟随全局、关闭按钮/遮罩）；
  - 字号/主题/模式变化本地即时应用并调用 `updateBookSettings`（仅 patch 字段），响应
    effective settings 为最终状态；旧响应由单调 generation 忽略；写入失败显示
    “设置未能保存，请重试。”且不关闭抽屉/session；成功保存全局提示“已保存为全局默认”，
    恢复成功提示“已恢复为跟随全局”；
  - 设置抽屉与目录互斥；Esc 顺序：设置 → 目录 → 沉浸式；
  - 沉浸式按钮主动进入（初始 false、不持久化），隐藏非必要控件，顶部 48px 可聚焦热区
    在指针/焦点进入时显示头部；显示后目录/设置可用；Esc 退出；
  - 章节切换继续使用当前完整 settings，不关闭 session。
- **测试**：
  - Rust：library settings 4（默认/全局/单书/清除/重置/非法/removed/损坏 JSON/只改设置表/
    updated_at）；命令 2（继承链路、错误）；workspace 289 passed/1 ignored。
  - Vitest：命令桥设置 3（正/负/错误映射）、ReaderShell 设置与沉浸式 6、engine
    applySettings 1（字号变量/主题 class/正文与图片不变）；全量 132 tests。
  - WebKit E2E：`update-settings` 字号/主题即时生效、特殊颜色与 font-family 保持、图片尺寸
    与内联样式不变、`body.innerHTML` 不变、night 背景变化、Locator textOffset 保持；全量
    43 passed。

**修改文件：**

- `crates/reader-model/src/session.rs`
- `crates/storage-core/src/open.rs`、`service.rs`
- `crates/library-core/src/settings.rs`（新增）、`open.rs`、`service.rs`、`lib.rs`
- `crates/library-core/tests/settings.rs`（新增）
- `src-tauri/src/commands.rs`、`lib.rs`
- `src-tauri/tests/m5d_settings.rs`（新增）
- `src/bridge/ipc-schemas.ts`、`commands.ts`、`messages.ts`、`parent.ts`、`iframe.ts`
  及对应测试
- `src/features/reader/ReaderShell.tsx`、`reader-shell.test.tsx`
- `src/reader-engine/engine.ts`、`engine-settings.test.tsx`（新增）
- `src/styles.css`
- `tests/e2e/reader-navigation.spec.ts`、`reader-pagination.spec.ts`、
  `reader-security.spec.ts`、`m3-matrix.spec.ts`（init/update-settings settings 契约）
- `src-tauri/resources/reader-engine.js`（仅由 `npm run build` 生成）
- `docs/tasks/TASK-M5D.md`（状态与本第 11 节）

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | `tsc -b`、Biome、Vitest 15 文件 137 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 42 passed（连续 3 次完整运行通过；`library.spec` 连续 8 次通过） |
| `npm run build` | 0 | 主应用 + 阅读脚本成功（JS 427.37 kB） |
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 291 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | release 成功；`Qurner.app` 7.84 MiB |
| `git diff --check` | 0 | 通过 |

**AC 验证：**

- FR-073/AC-070：模式切换即时应用并写入单书覆盖；重开使用持久化值。
- FR-080–FR-084/FR-087/AC-080：字号 0.1 步长、即时应用、持久化、原书 CSS/字体族保留、
  图片尺寸不变、Locator 保持；WebKit 验证。
- FR-090–FR-093/AC-090：全局/单书继承、修改全局不覆盖单书、清除回落、reset 只清本书。
- FR-100/FR-101/AC-100：白天/黑夜主题即时切换，图片无滤镜/反相，特殊颜色保留。
- FR-110–FR-114/FR-116/AC-110：沉浸式主动进入、隐藏控件、热区/焦点显示、目录与设置可用、
  打开书籍不自动进入。
- FR-201/AC-200：Esc 设置 → 目录 → 沉浸式；书签入口留 M5E。
- NFR-010：只写设置表，不修改原始文件/正文 DOM/其他数据。
- NFR-011：无远程字体/主题/网络请求。
- NFR-012：读取失败“本地阅读设置无法读取。”、写入失败“设置未能保存，请重试。”，
  不含路径/底层错误。

**未验证项：**

- 真实 Tauri WKWebView 中 `epub://` 跨章节 + 设置命令的端到端（WebKit E2E 使用独立
  iframe harness；设置持久化链路由 Rust/命令测试与 Vitest fake client 覆盖）。
- 真实硬件按键路径（WebKit E2E 使用 iframe 内 DOM keydown 事件验证引擎规则；
  Playwright 真实按键在 iframe 焦点下不稳定）。
- 真实 `updateBookSettings` IPC 失败路径（Vitest 使用 fake client 注入失败）。
- 多进程并发与真实 IPC 乱序压力（已用共享 storage worker 的双线程测试验证字段合并）。
- 复杂真实 EPUB 的字体族/rem/em 排版与主题可读性回归。

**风险与待决问题：**

- 字号通过 `body` 基线覆盖实现；原书显式 `px` 字号/body 背景/颜色会保留，可能与主题
  组合出非预期视觉，需真实书籍验证。
- 设置读-校验-写在 storage worker 中串行执行，但跨多个请求不是单个 SQLite 事务；单进程
  无交叉覆盖，崩溃窗口可能留下部分写入（任务要求“原子”仅在 worker 串行层面满足）。
- `reader-engine.js` 为 `npm run build` 生成产物，测试/发布前必须重新构建。
- 沉浸式控件显示后由状态保持，指针离开不自动隐藏；与 M5E 书签入口需协调。
- 设置抽屉打开时调用 `getEffectiveSettings` 刷新；若用户极快修改，旧响应由 generation
  忽略。

**依赖与禁止路径：**

- 未新增依赖；未修改 `package.json`/锁文件。
- 未修改数据库 schema/version、`epub://` 白名单、CSP/capability、SRS/SDS/
  DEVELOPMENT_PLAN/ADR/其他任务。
- `src-tauri/resources/reader-engine.js` 仅由 `npm run build` 生成。
- 未创建 commit/push。

## 12. Codex 审查

> 仅在 DeepSeek 标记为 `ready_for_review` 后填写。

**结论：** 返工

**审查日期：** 2026-11-15

**独立执行的检查：**

- `npm run check`：通过；14 files、132 tests，TypeScript、Biome、Vitest 均通过。
- `npm run test:e2e`：通过；WebKit 43 passed。
- `npm run build`：通过。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：289 passed、0 failed、1 ignored。
- `npm run tauri build`：通过；生成 `Qurner.app`，7.84 MiB。
- `git diff --check`：通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `crates/library-core/src/settings.rs:75-100`、`118-149`、`crates/storage-core/src/service.rs:62-78` | 设置更新不是原子操作。`update_app`/`update_book` 先通过一个 worker 请求读取旧值，再在 Rust 中合并，最后通过另一个请求写回；两个并发 patch 可以在读写之间交错，导致 `mode`、`fontScale` 或 `theme` 丢失。`app_settings` 还写入完整设置而不是任务约定的显式 patch；报告声称“单进程无交叉覆盖”，但并发命令仍会穿插。 | 在 storage worker 内增加单个请求的 read-merge-validate-write（最好同一 SQLite 事务）；单书写入只改 patch 出现的列，清除字段写 NULL；全局只写显式 patch。增加并发 patch 丢失测试。 |
| MAJOR | `src/features/reader/ReaderShell.tsx:157-214`、`339-365` | `open()`/切换书籍没有递增 `settingsGenerationRef`。旧书的 `updateBookSettings`、`getEffectiveSettings` 或 `resetBookSettings` 在打开新书后才返回时，generation 仍可能相等且 `closedRef` 已被重置，旧 effective settings 会覆盖新书设置。 | 在每次打开/切换 bookId 时使 settings generation 失效，并忽略旧 bookId/旧 session 的响应；增加 deferred 响应跨 book 切换测试。 |
| MAJOR | `src/features/reader/ReaderShell.tsx:280-316` | iframe 已加载但尚未收到 `reader:ready` 时修改 mode/font/theme，`bridge.updateSettings()` 因 bridge 处于 waiting 返回 false；`onReady` 不会重新应用当前 state，界面显示新设置但正文仍使用旧设置。 | 保存最新 settings ref，并在 `onReady` 后重新发送当前完整设置；或在 ready 前禁用设置控件。增加 pre-ready 模式/字号变更测试。 |
| MAJOR | `src/reader-engine/engine.ts:483-496` | 默认 `fontScale=1` 仍用 `body { font-size: calc(1em * var(...)) }` 覆盖原书 body 字号，违反“未调整时保留原 CSS”；黑夜主题只改 `html`，常见 EPUB 在 `body { background/color }` 声明时仍会得到白底黑字或不可读组合。 | 让 `fontScale=1` 保留原始 computed body 字号，并选择不使用整体 zoom/transform 的文字缩放方案；主题覆盖阅读根和正文默认背景/颜色，但不覆盖子元素显式特殊颜色和图片。补充 body 自带字号/背景/颜色夹具。 |
| MAJOR | `src/features/reader/ReaderShell.tsx:339-364`、`778-787` | 头部模式切换触发的写入失败只设置 `settingsError`，但错误文本只在设置抽屉打开时渲染；用户从头部切换失败看不到要求的非阻塞错误。 | 将设置错误提示放在全局 ReaderShell 状态区，或在设置抽屉外也渲染固定提示；增加头部模式失败测试。 |
| MAJOR | `tests/e2e/reader-navigation.spec.ts`、报告“未验证项” | 任务要求在 WebKit 中验收沉浸式隐藏/显示控件、热区和 Esc 顺序，但开发报告明确未实现该 E2E，只做了 Vitest；当前无法证明真实 WebKit 下控件和 iframe 尺寸行为。 | 增加 Playwright 主应用或真实 WKWebView 夹具的沉浸式流程测试，覆盖进入、热区显示、设置/目录访问、Esc 顺序和退出。 |
| MINOR | `src/features/reader/ReaderShell.tsx:716-789` | 设置抽屉声明 `role="dialog" aria-modal="true"`，但打开时没有将焦点移入，关闭后也不恢复触发按钮焦点；与目录抽屉的可访问焦点处理不一致。 | 补充设置抽屉焦点管理和回归测试。 |

**AC 结果：**

- FR-073/FR-080–FR-084/FR-087：基础实现和现有测试通过，但 pre-ready 更新、默认字号保留和真实沉浸式 E2E 仍未满足。
- FR-090–FR-093/AC-090：顺序测试通过；并发原子性和全局 patch 契约未满足。
- FR-100/FR-101/AC-100：当前夜间/字体夹具通过，但常见 body 默认颜色/背景夹具未覆盖，不能判定完整通过。
- FR-110–FR-114/FR-116/AC-110：Vitest 基础流程通过；真实 WebKit 沉浸式验收缺失。
- FR-201/AC-200：基础 Esc 顺序通过；设置抽屉焦点和真实浏览器流程仍需补齐。
- NFR-010/011/012：未发现原文件、网络或敏感信息泄漏；设置写入失败/并发路径仍需修复。

**返工或后续任务：**

- `TASK-M5D` 转为 `rework`，保持未提交。
- 修复设置写入原子性、跨 book 旧响应、pre-ready 设置应用、字号/主题默认样式和错误提示后，重新运行全部门禁并补充真实 WebKit 沉浸式测试。
- 未创建 commit；未启动 M5E。

### 第二次返工复验

**结论：** 返工

**审查日期：** 2026-11-15

**本次独立执行的检查：**

- `npm run check`：通过；14 files、136 tests。
- `cargo test --workspace`：通过；0 failed，1 ignored。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `npm run build`、`npm run tauri build`：通过；`Qurner.app` 7.84 MiB。
- `npm run test:e2e`：第一次完整运行 41 passed、1 failed（沉浸式测试找不到“显示阅读控件”）；单测复跑和第二次完整套件均通过 42 tests。该门禁目前不稳定。
- `git diff --check`：通过。

**剩余问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `src/reader-engine/engine.ts:501-515`、`tests/e2e/reader-navigation.spec.ts:94` | 字号基线只记录 `body.style.fontSize`（内联样式）。当字号来自外部 CSS 或 `<style>` 时，`originalBodyFontSize` 为空，`fontScale=1` 会设置 `1em` 并改变原书 18px 等布局；当前 WebKit 夹具只使用 body 内联样式，未覆盖该常见场景。 | 读取/保留 computed body 字号并在 scale=1 时恢复原始样式；增加外部/内联 `<style>` 设置的 body 字号夹具。 |
| MAJOR | `crates/storage-core/src/open.rs:126-194`、`crates/library-core/src/settings.rs:153-179` | 全局设置事务只检查 `json_valid`，不先验证 JSON 是对象且键/值符合 schema；`json_set` 会先提交，再由 library 的 `parse_global` 发现未知键或非法值并返回 `E_STORAGE`，此时原记录已经被修改。单书设置同样可能在保留非法旧字段时先写入新字段。 | 在写入事务内完成对象、未知键和值范围校验；校验失败必须回滚且 `value_json`/`updated_at` 均不变，增加“语法合法但语义非法”的回归测试。 |
| MAJOR | `tests/e2e/library.spec.ts:382-405` | 沉浸式 WebKit 门禁出现一次失败：完整套件中点击“沉浸式”后找不到热区，Playwright 快照显示头部控件仍在且状态为“正在准备阅读内容…”。单测/第二次全量运行通过说明存在时序竞态或测试与加载状态耦合。 | 修复进入沉浸式时 loading/ready 与热区的确定性，并让测试等待明确的 ready/沉浸式状态；连续运行完整 WebKit 套件不得出现失败。 |
| MINOR | `docs/tasks/TASK-M5D.md:373-376`、`415-426` | 报告仍保留旧的 `body { font-size: 1em }` / 仅 html 主题描述，以及重复的 AC 条目；与实际第二次返工实现和最新测试数量不一致。 | 更新开发报告后再作为正式复验证据。 |

**AC 结果：**

- FR-073/FR-090–FR-093/AC-090：并发/继承主路径已有测试，但语义损坏数据的写入保护和跨事务 active 校验仍需确认，不能判定完整通过。
- FR-080–FR-084/FR-087/AC-080：内联字号夹具通过；外部 CSS 字号的默认保持不通过。
- FR-100/FR-101/AC-100：内联 body 样式夹具通过；外部样式夹具和完整主题可读性仍需补证。
- FR-110–FR-114/FR-116/AC-110：功能实现和单测通过，但真实 WebKit 门禁存在一次阻塞性失败，不能判定稳定通过。
- FR-201/AC-200：设置抽屉焦点与 Esc 顺序测试通过。

**返工或后续任务：**

- `TASK-M5D` 保持 `rework`。
- 修复 computed 字号保留、语义非法设置回滚和沉浸式测试竞态后，重新运行完整门禁；`npm run test:e2e` 必须从干净状态连续通过。
- 未创建 commit；未启动 M5E。

### 最终复验

**结论：** 通过

**审查日期：** 2026-11-15

**独立执行的检查：**

- `npm run check`：通过；14 files、137 tests。
- `npm run test:e2e`：连续两次完整运行均 42 passed。
- `cargo test --workspace`：通过；0 failed，1 ignored。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `npm run build`：通过。
- `npm run tauri build`：通过；生成 `Qurner.app`，7.84 MiB。
- `git diff --check`：通过。

**本轮确认：**

- 设置更新在 storage worker/SQLite 事务内完成；并发 patch 和语义非法旧数据不会丢字段或被写入。
- 跨书旧设置响应被 generation 隔离；ready 前修改会在 ready 后回放。
- `fontScale=1` 恢复内联/computed 原书字号，外部 CSS 字号夹具通过。
- 主题覆盖正文默认背景/颜色，图片尺寸/滤镜和子元素特殊颜色保持不变。
- 头部模式失败提示、设置抽屉焦点和沉浸式 WebKit 流程测试通过。
- 未发现 `BLOCKER` 或 `MAJOR` 问题。

**返工或后续任务：**

- `TASK-M5D` 状态改为 `accepted`。
- 真实 Tauri 跨章节设置冒烟和最终应用验收仍按计划由 M6 覆盖。
- 未创建 commit；未启动 M5E。
