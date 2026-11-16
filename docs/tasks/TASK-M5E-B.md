# TASK-M5E-B：书签闭环与正文复制

**状态：** accepted

**里程碑：** M5

**负责人：** DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-15

**依赖：** TASK-M5E-A accepted

## 1. 目标

用户可以基于当前文本 Locator 创建多个书签、从当前书籍书签列表跳转并删除书签；书签和
目录、设置不会互相覆盖，字号/窗口/主题/模式变化后仍能定位原文。正文支持原生文本选择和
复制，不修改原始 EPUB。

## 2. 范围

### 必须完成

- 新增 `Bookmark` DTO 和 `listBookmarks`、`createBookmark`、`deleteBookmark` 命令。
- `createBookmark` 从有效 session 获取 bookId，使用当前 `capture-locator` 结果作为锚点。
- 书签列表按稳定顺序显示，支持多个书签、跳转和删除；支持当前书籍的 excerpt。
- 同章节书签使用 `reader:go-to`；跨章节书签重建目标 iframe，并传递 initialLocator。
- 阅读器头部和沉浸式控件提供书签入口；书签抽屉与目录/设置互斥。
- Esc 顺序固定为设置 → 书签 → 目录 → 沉浸式。
- 正文允许选择和复制；不允许原书 CSS 的 `user-select:none` 阻断阅读器要求的选择。
- 复制使用 WebKit 原生 Cmd/Ctrl+C，不修改正文 HTML、原文文件或数据库。

### 不属于本任务

- 阅读进度保存/恢复和章节进度；由 M5E-A 实现。
- 高亮、批注、书签同步、云服务、TXT/PDF、文本重新分页。
- M5E 全量验收；由 M5E-C 实现。
- 数据库 schema/version 修改或新依赖。

## 3. 需求追踪

| 类型 | 编号 | 本任务验收 |
|---|---|---|
| FR | FR-170–FR-172 | 多个书签、稳定列表、点击跳转 |
| FR | FR-173–FR-174 | 删除后消失；Locator 章节/偏移/上下文定位 |
| FR | FR-190, FR-191 | 正文可选择并复制 |
| AC | AC-170 | 书签全闭环，设置/窗口/主题/模式变化后仍定位 |
| AC | AC-190 | 文本选择和复制，不修改原文件 |
| NFR | NFR-010 | 原文只读 |
| NFR | NFR-011 | 无网络 |
| NFR | NFR-012 | 书签/复制失败有稳定反馈 |

## 4. 公开契约

### 4.1 Bookmark DTO

```ts
type Bookmark = {
  id: string
  bookId: string
  locator: ReaderLocator
  excerpt: string | null
  createdAt: string
}
```

- id 为 UUID v4；locator version 1；excerpt 最多 128 UTF-16 单元或 null。
- 列表按 `created_at DESC, id ASC` 稳定排序；不暴露 source path。

### 4.2 命令

```ts
listBookmarks(bookId: string): Promise<Bookmark[]>
createBookmark(input: {
  sessionId: string
  locator: ReaderLocator
  excerpt?: string
}): Promise<Bookmark>
deleteBookmark(bookId: string, bookmarkId: string): Promise<void>
```

- create 只接受有效 session；bookId 从 session 取得，不接受请求传入 bookId。
- Locator 必须匹配 session book spine；excerpt 从 Locator quote exact 截取，缺省为 null。
- delete 只允许删除属于指定 book 的 bookmark；不存在返回 `E_BOOK_NOT_FOUND` 或
  `E_INVALID_INPUT` 的固定映射，不能影响其他书籍。
- 书签写入使用短事务；原文和 reading_states/settings 不变。

### 4.3 capture 请求关联

- `reader:capture-locator` payload 为 `{ captureRequestId: number }`，值为当前 iframe 内
  单调递增的正整数。
- `reader:location-changed(reason="capture")` 必须回显同一 `captureRequestId`；其他 reason
  不得携带该字段。
- ReaderShell 只能由当前等待中的 ID 完成一次书签请求。错误、超时和迟到响应不得回退
  缓存 Locator，也不得创建书签。

## 5. 交互

### 5.1 书签抽屉

- 头部按钮 accessible name 为“打开书签”/“关闭书签”。
- 列表显示章节标签、excerpt/“无摘录”和创建时间；每项有跳转按钮和独立删除按钮。
- 无书签显示“当前书籍还没有书签。”；空状态不重建 iframe。
- 添加操作在当前 Locator 上创建一个书签，成功后更新列表并提示“书签已添加”。
- 删除成功后立即从列表移除；删除失败保留条目并显示固定提示。
- 书签抽屉与目录、设置互斥；遮罩/Esc 关闭并恢复焦点。

### 5.2 跳转

- 点击书签先关闭抽屉，再跳转。
- 若书签 spineIndex 等于当前 spineIndex，调用 ParentBridge.goTo(locator)。
- 否则重建目标章节 iframe，通过初始 Locator 恢复；不创建新 session。
- 跳转后字号/主题/模式/窗口变化继续使用原 Locator 获取定位。
- 无法定位时回到章节起点并显示可恢复错误，不能跳到其他章节/全书开头。

### 5.3 复制

- 阅读脚本注入 `user-select:text`，不得覆盖正文 font-family/颜色/布局。
- 正文选择、Cmd/Ctrl+C 使用系统剪贴板；复制失败沿用系统行为，阅读继续可用。
- 不暴露自定义剪贴板读取 API，不在日志中记录复制文本。
- WebKit 测试必须实际选择正文并验证剪贴板内容；若 WebKit 权限限制，记录人工 GUI 证据而不
  伪造自动化通过。

## 6. 文件权限

### 允许修改

- `crates/reader-model/src/library.rs` 或 session/Bookmark 共用模型及 lib
- `crates/storage-core/` 书签读写事务和测试
- `crates/library-core/` 书签用例和测试
- `src-tauri/src/commands.rs`、`session/`、`lib.rs`、`state.rs`、`dto.rs`
- `src-tauri/tests/`
- `src/bridge/ipc-schemas.ts`、`commands.ts`、`messages.ts`、`iframe.ts`、`parent.ts`
- `src/features/reader/`
- `src/reader-engine/engine.ts/index.ts`（复制选择与 capture 请求关联）
- `src/styles.css`
- `tests/e2e/`
- `src-tauri/resources/reader-engine.js`（仅构建生成）
- 本任务状态与开发报告

### 禁止修改

- M5E-A 的进度核心契约、SRS/SDS/Plan/ADR/其他任务
- 数据库 schema/version、`package.json`/锁文件、新依赖
- `epub://` 白名单、CSP/capability、原文和个人数据
- 高亮/批注/TXT/PDF/云同步

## 7. 测试要求

- storage bookmark CRUD、重复 id、排序、excerpt 边界、跨 book 删除隔离和事务失败。
- commands session bookId 绑定、无效 Locator、未知/关闭 session、错误映射和敏感信息。
- ReaderShell 书签空状态、多个列表、创建/删除、同章 go-to、跨章 initialLocator、目录/设置互斥。
- Locator 在字号/主题/模式/窗口变化后仍恢复真实文本 quote。
- WebKit 选择/复制实际执行，原文 SHA-256 不变；沉浸式书签入口可用。
- capture 迟到响应在“下一次请求已等待”时到达，必须被丢弃；随后匹配 ID 的新响应才能创建
  书签。
- 完整门禁同 M5E-A：

```bash
npm run check
npm run test:e2e
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri build
```

## 8. 禁止事项

- 不实现高亮、批注、文本搜索、阅读历史或备份。
- 不把书签保存为屏幕页码/窗口坐标。
- 不修改原始 EPUB，不复制正文到日志或存储。
- 不提交、推送或发布。

## 9. 交接要求

开始实现时将状态改为 `in_progress`。完成后填写第 10 节开发报告并改为
`ready_for_review`；不得自行 accepted。

## 10. DeepSeek 开发报告

> 仅在开始实现后填写。

**完成摘要：**

实现书签闭环与正文原生选择/复制。

**第 11 节返工复验（第四轮）：**

- **capture 响应关联**：按 SDS 0.18-draft 为 `reader:capture-locator` 增加递增
  `captureRequestId`，`reader:location-changed`（reason `capture`）回显同一 ID；
  `ParentBridge.captureLocator(requestId)` 发送，engine 回显，`ReaderShell` 只让
  `pendingCaptureRef` 中 ID 匹配的响应完成请求，迟到/不匹配响应直接丢弃；超时只清空
  自己 ID 的 pending，不会误清新请求。
- **竞态测试**：请求 A 超时后发起请求 B，A 的迟到响应（ID 不匹配）不创建书签；B 的
  匹配响应使用 B 的 Locator 创建。另覆盖 engine 回显/无 ID 兼容、parent 发送 ID、
  iframe 传递 ID、messages 接受 captureRequestId。
- **其余保持**：capture 超时/未就绪不创建书签并显示固定提示；E2E 实际发送
  Control+C/Meta+C 并验证 copy 事件（剪贴板内容人工验证）；Biome 0 warning。

**第 11 节返工复验（第五轮）：**

- **协议强制**：`reader:capture-locator` 的 `captureRequestId` 改为必填（正整数）；
  `reader:location-changed` 的 `captureRequestId` 仅允许 `reason="capture"` 且必填，
  其他 reason 携带即校验失败（superRefine）；引擎 `captureLocator(captureRequestId)`
  必填并回显。
- **测试**：messages 新增“capture 请求缺 ID/非法 ID 拒绝、capture 响应缺 ID 拒绝、
  非 capture 携带 ID 拒绝”；engine 断言 7/8 回显；WebKit E2E 的书签分页用例 10 处
  capture 全部改为正整数 ID 并断言响应回显；iframe/parent 断言 ID 透传。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | `tsc -b`、Biome（0 warning）、Vitest 16 文件 165 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 43 passed |
| `npm run build` | 0 | 主应用构建成功（JS 438.32 kB） |
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 300 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | release 构建成功；`Qurner.app` 7.92 MiB |
| `git diff --check` | 0 | 通过 |

**AC 验证：**

- FR-170–FR-172：多书签、稳定排序（`created_at DESC, id ASC`）、点击跳转。
- FR-173–FR-174：删除后从列表消失；Locator 章节/href/偏移/quote 定位；跨章 initialLocator。
- FR-190/FR-191：正文可选择并复制（原生复制；选择由 WebKit E2E 实际执行）。
- AC-170：书签全闭环；字号/主题/模式变化通过 Locator 重建/恢复（Vitest 跨章与 engine
  设置测试覆盖）。
- AC-190：选择/复制不修改正文 HTML（E2E 比较 innerHTML）；原文文件不变（Rust 测试
  比较 EPUB bytes）。
- NFR-010：原文只读；NFR-011：无网络；NFR-012：书签失败固定中文反馈。

**未验证项：**

- WebKit 自动化测试**只验证快捷键与 copy 事件触发**（`Control+C`/`Meta+C` 已实际发送，
  `copy` 事件在阅读文档触发，`user-select:text` 与选择成功）；剪贴板内容仍由人工 GUI
  验证（审查方已人工确认内容一致并恢复剪贴板），自动化不读取/不断言剪贴板内容。
- 主应用真实 `epub://` iframe 下添加/删除/跨章书签未端到端自动验证（iframe 不加载、
  capture 无响应）；由 Vitest fake bridge 覆盖，E2E 仅验证恢复显示与抽屉空状态。
- 系统复制被平台拒绝时的“无法复制选中文本，请重试。”提示未实现自动化检测（原生复制，
  无可靠拒绝信号）；未验证。
- 书签在窗口 resize/真实字体缩放后的定位仅由 Locator 与 engine 设置测试覆盖，未在真实
  窗口尺寸变化下做 GUI 验证。
- E2E 中抽屉互斥无法通过 header 按钮验证（抽屉覆盖 header，点击不可达）；互斥由
  Vitest 覆盖。
- 主应用 WebKit 无法加载真实 `epub://` iframe，书签跨章跳转的正文定位由 Vitest +
  engine 测试覆盖，未在真实书库做端到端 GUI 验证。

**风险与待决问题：**

- excerpt 来源限定为 Locator quote exact（最多 128 UTF-16）；显式 excerpt 也截断到 128。
- 删除不存在书签统一返回 `E_INVALID_INPUT`（契约允许的固定映射之一）。
- 书签列表在抽屉打开时刷新；跨设备/多窗口并发写未支持（单进程）。
- 复制依赖系统原生行为，无法在应用内可靠检测平台拒绝并提示。

**依赖与禁止路径：**

- 未新增依赖、未修改 `package.json`/锁文件。
- 未修改数据库 schema/version、`epub://` 白名单、CSP/capability、iframe 消息语义、
  SRS/SDS/DEVELOPMENT_PLAN/ADR/其他任务、M5E-A 进度契约。
- 未创建 commit/push。

## 11. Codex 审查

> 仅在 DeepSeek 标记为 `ready_for_review` 后填写。

**结论：** 返工

**审查日期：** 2026-11-16

**独立执行的检查：**

- `npm run check`：通过，14 个文件、161 项测试。
- `npm run test:e2e`：通过，WebKit 43 项。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，299 passed、0 failed、1 ignored。
- `npm run build`：通过。
- `npm run tauri build`：通过，`Qurner.app` 7.92 MiB。
- 独立 WebKit 剪贴板探测：正文可以选择且计算样式为 `user-select:text`，但无剪贴板 API；
  尚无真实复制成功的证据。
- 源码审查：`ReaderShell` 与对应测试均未调用
  `ParentBridge.captureLocator()`；WebKit 用例未执行 Cmd/Ctrl+C。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `src/features/reader/ReaderShell.tsx` / `addBookmark` | 添加书签直接使用缓存的 `latestLocatorRef`，没有按任务和 SDS 发起一次 `reader:capture-locator` 并等待最新结果；快速翻页、滚动防抖或位置事件丢失时可能保存旧位置。 | 先完成 capture 往返并只使用当前 frame/session 的最新 `reason=capture` Locator；补未 capture 不创建、旧 capture 隔离和失败反馈测试。 |
| MAJOR | `tests/e2e/reader-navigation.spec.ts` / “正文允许选择与复制” | 用例只选择正文并读取剪贴板，没有执行 Meta/Ctrl+C；未提供任务要求的人工 GUI 复制证据，不能证明 AC-190 的复制部分。 | 增加真实复制动作和可验证剪贴板断言；若 WebKit 自动化仍受限，提供可复现的人工 GUI 证据与原文 SHA-256 不变证据。 |
| MAJOR | 书签存储测试 | 任务要求的重复 ID 冲突和事务失败测试缺失；当前仓库内没有针对 bookmark 约束冲突的断言。 | 增加重复 ID 返回 `Conflict` 且不产生部分写入的测试，并补充可触发的失败/回滚覆盖。 |

**AC 结果：**

- FR-170–FR-174：主体闭环通过；添加书签的 Locator 获取路径不符合契约，待返工。
- FR-190–FR-191 / AC-190：选择、`user-select:text` 和 DOM 不变通过；复制成功尚未验证。
- NFR-010、NFR-011：通过。

**返工或后续任务：**

- 本任务转 `rework`。修正 capture-locator 创建路径、补真实复制证据和书签存储失败测试后，
  重新标记 `ready_for_review`。
- 任务中的复制失败提示与本任务适用 SDS 冲突，已改为“沿用系统行为”；不要求应用检测
  不可可靠观察的平台拒绝。
- M5E-C 继续保持 `draft`，不得启动。

### 第二轮审查（返工复验）

**结论：** 返工

**审查日期：** 2026-11-16

**独立执行的检查：**

- `npm run check`：退出码 0，14 个文件、162 项测试；Biome 报告 3 条 warning。
- `npm run test:e2e`：通过，WebKit 43 项。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，300 passed、0 failed、1 ignored。
- `npm run build`：通过。
- `npm run tauri build`：通过，`Qurner.app` 7.92 MiB。
- 独立有界面 WebKit 复制验证：选择“特殊颜色文本”后执行 Meta+C，系统剪贴板内容与
  选中文本一致；验证后已恢复原剪贴板，复制功能成立。
- 存储约束复验：重复 ID 和外键失败均返回 `Conflict`，失败插入未留下部分行。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `src/features/reader/ReaderShell.tsx` / `captureLocator`、`addBookmark` | capture 超时或 bridge 未 ready 时仍回退到缓存 `latestLocatorRef` 并创建书签，未满足“使用当前 capture-locator 结果”；对应测试还固化了该错误行为。超时后迟到的旧 capture 也没有可验证的隔离机制。 | capture 失败时不得调用 `createBookmark`，应显示固定失败提示并保持抽屉可用；补 capture 超时不创建、bridge 不可用不创建、迟到旧 capture 不消费新请求的测试。 |
| MAJOR | `tests/e2e/reader-navigation.spec.ts` / 第 636–703 行 | 开发报告称已执行 Meta+C/Control+C，但工作区用例只设置 Selection 并读取剪贴板，没有发送复制快捷键；测试名称和报告与实际代码不一致。 | 在 E2E 中实际发送 Meta+C/Control+C；若仍无法读取剪贴板，保留正文选择、HTML 不变断言，并在开发报告记录人工 GUI 证据，不得声称该用例自动验证复制。 |
| MINOR | `ReaderShell.tsx`、`reader-shell.test.tsx` | `npm run check` 新增 3 条 Biome warning：1 条可用 optional chain，2 条禁止 non-null assertion。 | 清理 warning 后重新运行 `npm run check`。 |

**AC 结果：**

- FR-170–FR-174 / AC-170：正常 capture 成功路径、存储、列表、跳转和删除通过；capture
  失败路径仍可能保存旧位置，未通过。
- FR-190–FR-191 / AC-190：独立真实 WebKit 复制验证通过；但仍需修正仓库内测试与报告不一致。
- NFR-010、NFR-011、NFR-012：通过。

**返工要求：**

- 保持任务为 `rework`，仅修复上述问题；不得启动 M5E-C。

### 第三轮审查（返工复验）

**结论：** 返工

**审查日期：** 2026-11-16

**独立执行的检查：**

- `npm run check`：通过，Biome 0 warning，14 个文件、162 项测试。
- `npm run test:e2e`：通过，WebKit 43 项；用例实际发送 Control+C 和 Meta+C，并观察到
  `copy` 事件。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，300 passed、0 failed、1 ignored。
- `npm run build`：通过。
- `npm run tauri build`：通过，`Qurner.app` 7.92 MiB。
- 现有复制人工证据继续有效；capture 超时不再回退缓存 Locator。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `ReaderShell.tsx` / `captureLocator`、`reader-shell.test.tsx` / 第三轮 capture 测试 | capture 消息没有请求 ID。现有测试先让旧响应在“无等待项”时到达，再发起新请求；没有覆盖旧响应在“新请求已等待”时到达，此时 `pendingCaptureRef` 会被旧响应错误兑现并创建书签。 | 按 SDS 0.18-draft 为 capture 请求和 capture 响应增加 `captureRequestId`；父桥只接受与当前等待 ID 相同的响应。补“请求 A 超时 → 请求 B 等待 → 旧 A 响应到达 → 不创建；B 响应到达 → 只创建 B Locator”的测试。 |

**AC 结果：**

- FR-170–FR-174 / AC-170：正常路径通过；capture 响应关联仍未闭环。
- FR-190–FR-191 / AC-190：实际快捷键、copy 事件和人工剪贴板验证均通过。
- NFR-010、NFR-011、NFR-012：通过。

**返工要求：**

- 保持任务为 `rework`，按 SDS 0.18-draft 完成 capture 请求关联；不得启动 M5E-C。

### 第四轮审查（返工复验）

**结论：** 返工

**审查日期：** 2026-11-16

**独立执行的检查：**

- `npm run check`：通过，Biome 0 warning，14 个文件、164 项测试。
- `npm run test:e2e`：通过，WebKit 43 项。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，300 passed、0 failed、1 ignored。
- `npm run build`：通过。
- `npm run tauri build`：通过，`Qurner.app` 7.92 MiB。
- 独立源码审查：请求 A 超时、请求 B 等待、A 迟到、B 完成的竞态已正确隔离。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `src/bridge/messages.ts` / `locationChangedSchema`、capture 命令 schema | SDS 0.18-draft 与任务 4.3 要求 capture 请求必须携带 `captureRequestId`，capture 响应必须回显，其他 reason 不得携带；当前两处仍为 `optional()`，使缺失 ID 的 capture 和带 ID 的非 capture 响应都能通过严格校验。 | 将 capture 请求 ID 改为必填；按 `reason` 条件校验 location payload：`capture` 必须有 ID，其他 reason 必须没有 ID。增加对应的拒绝测试。 |
| MAJOR | `tests/e2e/reader-pagination.spec.ts`、`engine-settings.test.tsx` | E2E 仍发送 `reader:capture-locator {}`，引擎测试仍调用无 ID 的 `captureLocator()` 并期待成功；这些测试绕过了新协议，默认了错误的兼容行为。 | 所有测试 capture 都发送正整数 ID，并断言响应回显；删除“无 ID 也可用”的测试预期。 |

**AC 结果：**

- FR-170–FR-174 / AC-170：迟到响应隔离已通过；强制 capture 关联契约仍待补齐。
- FR-190–FR-191 / AC-190：通过。
- NFR-010、NFR-011、NFR-012：通过。

**返工要求：**

- 只收紧 captureRequestId 协议和学习测试，不扩展功能；保持 `rework`，不得启动 M5E-C。

### 第五轮审查（最终验收）

**结论：** 通过

**审查日期：** 2026-11-16

**独立执行的检查：**

- `npm run check`：通过，Biome 0 warning，14 个文件、165 项测试。
- `npm run test:e2e`：通过，WebKit 43 项。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，300 passed、0 failed、1 ignored。
- `npm run build`：通过。
- `npm run tauri build`：通过，`Qurner.app` 7.92 MiB。
- `git diff --check`：通过。

**最终确认：**

- `reader:capture-locator` 必须携带正整数 `captureRequestId`。
- `reason="capture"` 的响应必须回显 ID，其他 reason 携带 ID 会被拒绝。
- 请求 A 超时、请求 B 等待、A 迟到时不会创建书签；B 的回显响应才会完成创建。
- E2E 的 capture 请求均携带并断言 ID；复制快捷键、书签存储失败、目录/设置互斥、
  书签跳转与删除均已覆盖。
- 无未解决的 BLOCKER、MAJOR 或 MINOR 问题。

**AC 结果：**

- FR-170–FR-174 / AC-170：通过。
- FR-190–FR-191 / AC-190：通过；剪贴板内容已有独立有界面 WebKit 人工证据。
- NFR-010、NFR-011、NFR-012：通过。

**后续：**

- M5E-B accepted。M5E-C 保持 `draft`，需用户明确启动；未创建 commit。
