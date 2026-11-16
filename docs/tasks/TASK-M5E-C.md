# TASK-M5E-C：M5 阅读器闭环总验收

**状态：** accepted

**里程碑：** M5

**负责人：** Codex + DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-15

**依赖：** TASK-M5E-A、TASK-M5E-B、TASK-M5R1、TASK-M5R2、TASK-M5R3、TASK-M5R4 accepted

## 1. 目标

以真实 `Qurner.app`、真实/合成 EPUB 和可重复测试证据，完成 M5 阅读器闭环的总验收：
打开书籍、目录、分页/滚动、设置、沉浸式、进度恢复、书签、文本复制和离线行为全部满足
SRS P0 范围，能够作为 MVP 开发的首个完整阅读器闭环。

## 2. 范围

### 必须完成

- 汇总 M5A–M5E-B 的需求、实现、测试和已知限制，形成需求—任务—测试追踪。
- 在真实 Qurner.app 中执行完整用户旅程，记录实际窗口、目录、模式、字号、主题、沉浸式、
  进度、书签和复制证据。
- 使用合成 EPUB 2/3、内嵌 CSS、中文/英文、图片、嵌套目录、缺失字体和异常文件夹具。
- 验证正常关闭与异常终止后的进度恢复；最多丢失 2 秒。
- 验证书签在字号、主题、模式、窗口尺寸变化后仍定位原文。
- 验证禁用网络后核心流程可用；无遥测、远程字体或外发请求。
- 独立运行全部 M5 门禁并记录命令、退出码、环境和结果。
- 形成 M5 缺陷清单；P0/BLOCKER/MAJOR 未关闭时不得通过。

### 不属于本任务

- M5 功能的新增大范围实现；发现缺陷应先创建 rework/后续任务。
- TXT/PDF、高亮批注、收藏夹、文件重定位、备份恢复、自动更新和云同步。
- M6 最终性能/加固报告；本任务只做 M5 闭环验收和必要证据。

## 3. 需求追踪

必须覆盖：

- AC-040：失效文件。
- AC-070：模式切换和持久化。
- AC-080：字号与原文样式。
- AC-090：全局/单书设置。
- AC-100：昼夜主题。
- AC-110：沉浸式。
- AC-120：目录。
- AC-130：正文图片。
- AC-160：进度保存/恢复与异常退出。
- AC-170：书签。
- AC-190：文本选择/复制。
- AC-200：键盘。
- AC-210：打开书籍。
- AC-220：本地离线。

以及 NFR-001–NFR-015 中适用于阅读器闭环的 NFR。

## 4. 验收场景

### 场景 A：完整主流程

1. 通过 Qurner.app 导入合成 EPUB。
2. 打开书籍，确认标题和首章。
3. 打开嵌套目录并跳转。
4. 分页/滚动切换，调整字号和主题。
5. 开启沉浸式，通过热区打开书签/设置/目录。
6. 创建书签，改变设置后跳回书签。
7. 选择并复制一段中文/英文正文。
8. 翻页/滚动后返回书库，再打开同一书籍恢复文本锚点。
9. 关闭应用，再次启动并恢复。

每一步必须记录自动化或人工证据，不能只写“通过”。

### 场景 B：异常终止

- 在阅读位置变化后强制终止 Qurner.app，重启后恢复最近一次成功保存的位置。
- 记录操作时间、最后保存时间、恢复 Locator 和丢失窗口，不得超过 2 秒。
- 测试损坏/缺文件和已被软移除书籍，不得崩溃或覆盖个人数据。

### 场景 C：规模与性能

- 1,000 本合成书库搜索和打开仍满足 NFR。
- 100 MB EPUB 按需加载，不复制整本正文到前端内存。
- 分页翻页、滚动、模式切换和恢复保持交互响应。

### 场景 D：安全与离线

- 禁网后完成核心流程。
- 监控应用网络请求，验证无 HTTP/HTTPS、远程字体/主题/遥测。
- 验证 iframe sandbox、CSP、链接阻止和 token 不泄漏。

## 5. 文件权限

### 允许修改

- `docs/acceptance/` 或任务规定的验收报告文件
- `docs/tasks/TASK-M5E-C.md` 状态和报告
- `tests/e2e/` 中必要的 M5 总验收夹具/测试
- 不直接修改产品实现；功能缺陷必须另开 rework。

### 禁止修改

- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、ADR
- 其他任务文件
- 原始 EPUB、真实用户书库和个人阅读数据

## 6. 测试与证据

必须运行：

```bash
npm run check
npm run test:e2e
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri build
```

同时必须提供：

- Qurner.app 真实 UI 截图/操作记录（不含私人书籍正文或路径）。
- 正常/异常恢复的时间与 Locator 证据。
- 100 MB/1,000 本性能测量。
- 网络请求监测结果。
- 需求—AC—测试追踪表。

## 7. 禁止事项

- 不把未运行的人工步骤写成已验证。
- 不用 mock 结果替代真实 Qurner.app 冒烟证据。
- 不修改真实书籍或用户数据来制造通过。
- 不提交、推送或发布。

## 8. 交接要求

依赖 A/B accepted 后开始，将状态改为 `in_progress`；完成后填写开发/验收报告并改为
`ready_for_review`。Codex 独立审查后决定 `accepted` 或返工。

## 9. DeepSeek 开发报告

**完成摘要：**

以真实 `Qurner.app`（隔离 HOME，未修改真实用户书库）与合成 EPUB 夹具执行 M5 阅读器闭环总验收，并独立运行全部 M5 门禁。

**结论：不通过。** 真实应用在打开有效 EPUB 后阅读器显示“阅读内容加载超时，请重试。”，无法进入阅读（BLOCKER）；书库默认网格视图卡片尺寸异常（MAJOR）；其余需阅读器的场景因 BLOCKER 无法验证，未用 mock 替代真实 app 证据。详细证据见 `docs/acceptance/M5E-C-REPORT.md` 与 `/tmp/qurner-acceptance/shots/`。

**真实应用验收（摘要）：**

- 启动 ✓、导入合成 EPUB3（中文/嵌套目录/图片/内嵌 CSS/缺失字体）✓“已导入 1”。
- 书库列表视图正常；默认网格视图卡片异常（条纹封面占位几乎占满视口，书名/作者不可见）。
- 打开书籍 → 阅读器 10 秒超时，重试仍超时（BLOCKER，`shots/18`、`shots/19`）。
- 失效文件（移动源文件）打开显示“原始文件已被移动、删除或无法读取。”✓（AC-040）。
- `kill -9` 后重启书库记录保留；阅读位置恢复未验证（阅读器不可用）。
- 网络：`lsof` 三轮采样 qurner 进程无 TCP/UDP 连接；真实断网未执行（避免影响宿主）。
- 100 MB：首次失败因夹具 nav 缺少 `xmlns:epub`（已修复），修复后真实导入未完成 → 未验证。
- 1,000 本：真实 app 批量导入未执行 → 未验证（自动化 E2E 1,000 本性能用例通过，不能替代真实 app 证据）。

**修改文件：**

- `docs/acceptance/M5E-C-REPORT.md`（新增，验收报告）
- `docs/tasks/TASK-M5E-C.md`（状态与本节报告）
- 未修改产品实现、SRS/SDS/PLAN/ADR/其他任务、数据库或用户数据；未提交。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 16 文件 165 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 43 passed |
| `npm run build` | 0 | 生产前端构建成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 300 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 7.92 MiB |
| `git diff --check` | 0 | 通过 |

**AC 结果：**

- AC-040：真实 app 失效文件错误通过；软移除未验证。
- AC-210：真实 app 打开书籍失败（阅读器加载超时，BLOCKER）。
- AC-070/080/090/100/110/120/130/160/170/190/200：自动化层通过，真实 app 因 BLOCKER 未验证（阻塞，不计通过）。
- AC-220：lsof 无外联证据；真实断网未验证。

**未验证项：**

- 真实 app 中的目录、分页/滚动、字号、主题、沉浸式、书签、文本复制、进度保存/恢复、异常终止恢复、键盘、软移除书籍。
- 真实 app 100 MB 与 1,000 本规模、断网运行。
- 剪贴板内容（M5E-B 人工验证过，本轮阅读器不可用）。

**风险与待决问题：**

- BLOCKER：真实 app `epub://` iframe/reader-engine 未就绪；需另开 rework 调查 WKWebView 自定义 scheme、iframe sandbox/CSP、协议时序。
- MAJOR：书库网格视图卡片尺寸/虚拟行高度异常，列表视图正常。
- 本轮验收使用 CGEvent 自动化（System Events 无辅助权限）；真实 app 自动化坐标依赖窗口尺寸，后续复测需保持窗口位置/尺寸一致。
- 100 MB 导入夹具已修复，但真实 app 导入未完成；需在修复 BLOCKER 后重测。

**依赖与禁止路径：**

- 未修改 SRS/SDS/DEVELOPMENT_PLAN/AI_COLLABORATION/ADR/其他任务、产品实现、数据库 schema、原文或真实用户数据。
- 未安装依赖、未联网搜索、未提交/推送。

### 第二轮复验报告（2026-11-16，M5R1/M5R2 修复后）

**结论：** 真实 Qurner.app 主阅读流程已可用（M5R1 握手与 M5R2 网格阻塞解除）。本轮发现 1 项
沉浸式控件未自动隐藏的缺陷，另有导入自动化、失效/软移除与规模场景未能在本轮真实 app
中完成（如实列为未验证）。

**环境与方法：**

- macOS 26.6 arm64；Qurner.app（M5R2 后 `npm run tauri build` 构建，7.92 MiB）。
- 隔离 HOME：`/tmp/qurner-acceptance/m5ec2/home`（数据目录仅合成书“中文嵌套图片”，无真实
  用户数据）；合成 EPUB 源文件 `$HOME/qurner-acceptance-fixtures/cn-nested.epub`。
- 截图目录：`/tmp/qurner-acceptance/shots2/`。
- 导入步骤说明：本轮尝试在新隔离 HOME 中重新导入合成 EPUB 3 次，文件对话框的 CGEvent
  键盘输入/焦点在当前桌面环境下不稳定（事件被 Finder/Harness 抢占或未送达），导入未完成；
  为继续验证阅读闭环，使用隔离 HOME 中复制自上一轮隔离验收的合成书数据库（仅合成书、
  无真实数据）初始化。**“重新导入”本轮未验证，不计通过。**

**真实应用证据（截图编号对应 `/tmp/qurner-acceptance/shots2/`）：**

| 场景 | 证据 | 结果 |
|---|---|---|
| 启动/空书库 | `01-empty.png` | 通过 |
| 书库网格（M5R2 修复后） | `09-library-seeded.png` | 书名/作者/移除完整，封面可见 |
| 打开书籍/首章 | `10-reader.png` | 首章“第一章 中文”可见，位置“第一章 · 12.5%” |
| 目录（嵌套显示） | `11-toc.png` | “第一章 / 第二章（嵌套）”显示 |
| 目录跳转 | `14-toc-key.png` | 键盘跳转“第二章 嵌套”，100% |
| 分页→滚动模式 | `15-scroll-mode.png` | 连续滚动选中 |
| 阅读设置/字号 | `16-settings.png`、`17-font.png` | 字号 100%→110% |
| 主题 | `18-night.png` | 黑夜主题生效 |
| 设置持久化 | 正常重启后 `28-normal-restart-reader.png` | 黑夜 + 110% 保持（此前点击“保存为全局默认”） |
| 沉浸式 | `19-immersive.png`、`20/21` | 入口可用；**控件 6.5s 未自动隐藏（缺陷）** |
| 书签 | `22-bookmarks.png`、`23-bookmark-added.png` | 空态/添加成功，DB `bookmarks` 1 条 |
| 文本复制 | `25-copy2.png`；`pbpaste` 输出 | 系统剪贴板：“第二章 嵌套 / 第二章正文，用于进度与书签跳转。” |
| 返回并重开恢复 | `26-reopen.png` | 恢复第二章 100%、黑夜、110% |
| 正常退出/重启恢复 | `27`、`28`；DB `reading_states` | 退出前保存 `global_progression=1.0, updated_at=2026-11-16T14:10:21Z`；重启恢复第二章 100% |
| 异常终止（kill -9） | `29-abnormal-restart.png`；DB | 重启后恢复最近成功保存位置（第二章 100%，spineIndex=1）；未产生未保存位移，≤2s 丢失窗口未实测 |
| 网络 | `lsof -a -p <qurner> -i` 无输出 | 进程无 TCP/UDP 连接（无外联证据） |

**缺陷清单（本轮）：**

| 级别 | 现象 | 证据 | 影响 |
|---|---|---|---|
| MAJOR | 进入沉浸式后控件不自动隐藏：鼠标移出热区并等待 6.5s，header 控件仍显示；只有再次点击/退出沉浸式才隐藏（代码审查：`controlsVisible` 仅由热区 hover/focus/click 置 true，无空闲隐藏计时器） | `19-immersive.png`、`21-immersive-hidden2.png` | AC-110“隐藏控件”在真实 app 鼠标路径下不满足；阅读沉浸体验受损 |

**AC 结果（本轮真实 app）：**

- 通过：AC-070（模式切换；字号/主题持久化）、AC-080（字号）、AC-090（保存为全局默认）、
  AC-100（昼夜主题）、AC-120（目录显示与跳转）、AC-160（正常退出/重启与 kill -9 后恢复）、
  AC-170（书签添加/列表/DB）、AC-190（真实系统剪贴板复制内容）、AC-210（打开书籍/首章）、
  AC-220 证据（无网络连接；未断网实测）。
- 部分/缺陷：AC-110（入口与退出通过，控件自动隐藏失败）。
- 未验证（本轮未重新执行）：AC-040（失效文件/软移除；上轮真实 app 通过，本轮自动化点击
  未完成，不沿用）、AC-130（正文图片显示）、书签跨设置跳转后定位（本轮仅添加/同页展示）、
  目录跳转后的字号/主题恢复（部分）、100 MB 与 1,000 本真实 app 规模（自动化 E2E 覆盖，
  非真实 app）、真实断网运行、剪贴板内容跨重启等。

**门禁（本轮独立执行）：**

| 命令 | 退出码 | 结果 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 16 文件 167 tests |
| `npm run test:e2e` | 0 | Playwright WebKit 45 passed |
| `npm run build` | 0 | 生产前端构建成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 300 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 7.92 MiB |
| `git diff --check` | 0 | 通过 |

**未验证项与风险：**

- 真实 app 重新导入：CGEvent 文件对话框键盘输入不稳定（本轮 3 次未成功）；用隔离合成书库
  初始化继续验证（数据库仅合成书），导入步骤不计通过。
- 失效/软移除本轮未重测（上轮通过）、正文图片（AC-130）未在真实 app 验证。
- 100 MB / 1,000 本真实 app 导入与性能未完成（自动化 E2E 45 项覆盖 1,000 本虚拟化；
  100 MB 夹具此前导入失败为夹具问题，修复后未完成真实导入）。
- 真实断网未执行（避免影响宿主）；仅 lsof 无外联证据。
- 沉浸式隐藏缺陷需修复（建议新任务），本轮不修改产品实现。
- 自动化坐标依赖窗口位置/尺寸与前台焦点；本轮多次出现 Finder/Harness 抢占焦点。

**修改文件：** `docs/tasks/TASK-M5E-C.md`（追加本节与状态）；未修改产品实现、SRS/SDS/
PLAN/ADR/其他任务、数据库 schema、真实用户书库；未提交 commit。

## 10. Codex 审查

**结论：** 阻塞

**审查日期：** 2026-11-16

**独立执行的检查：**

- `npm run check`：通过，14 个文件、165 项测试。
- `npm run test:e2e`：通过，WebKit 43 项。
- `cargo fmt --all --check`、Clippy、Rust 300 项测试、生产构建和 Tauri release 构建均通过。
- 独立查看 `13-library-loaded.png`、`17-listview.png`、`18-reader.png`、`29-missing-file.png`，
  与报告描述一致。
- 隔离数据库位于 `/tmp/qurner-acceptance/home/...`，仅有合成书“中文嵌套图片”；夹具
  SHA-256 和 101.13 MB 文件存在。
- 代码审查发现高置信度阻塞链：`ReaderShell` 的 iframe 使用
  `referrerPolicy="no-referrer"`，而 `src/reader-engine/index.ts` 在
  `document.referrer` 为空时直接返回，不创建 `IframeBridge`；现有 WebKit 测试未覆盖
  该组合，因此自动化通过但真实应用握手超时。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| BLOCKER | `src/features/reader/ReaderShell.tsx` / iframe、`src/reader-engine/index.ts` | 真实 Qurner.app 打开有效 EPUB 后 10 秒握手超时。iframe 禁止 referrer，而引擎依赖 `document.referrer` 计算父 origin；为空即不启动消息桥。 | 创建 `TASK-M5R1`，修复并增加真实无 referrer/打包环境回归测试；修复后真实 app 必须显示正文。 |
| MAJOR | `src/styles.css` / `.book-grid-row`、`src/features/library/LibraryPage.tsx` / grid columns | 真实 app 默认网格单卡占满视口，封面占位被行高裁成横向大块，书名不可见；列表视图正常。 | 创建 `TASK-M5R2`，修复真实应用网格尺寸和列测量，增加对应视觉回归。 |

**AC 结果：**

- AC-040：真实 app 失效文件错误通过；软移除未验证。
- AC-070/080/090/100/110/120/130/160/170/190/200：自动化层通过，真实 app 被 BLOCKER 阻塞。
- AC-210：真实 app 失败。
- AC-220：lsof 无连接；真实断网未验证。

**返工或后续任务：**

- M5E-C 转 `blocked`，不得视为通过。
- `TASK-M5R1`、`TASK-M5R2` 均为 `draft`，等待用户按顺序授权启动。
- 两项修复并重新执行真实 app M5E-C 旅程后，才可重新评估 M5 完成。

### 第二轮审查（第二次全量复验）

**结论：** 阻塞

**审查日期：** 2026-11-16

**独立执行的检查：**

- 复核 `shots2/19`、`20`、`21`：进入沉浸式并移出控制区后，header 持续可见。
- 复核隔离数据库：只有合成书“中文嵌套图片”；`reading_states` 恢复为第二章 100%，
  书签记录存在。
- `npm run check`：通过，14 个文件、167 项测试。
- `npm run test:e2e`：通过，WebKit 45 项。
- `cargo fmt --all --check`、Clippy、Rust 300 项测试、生产构建和 Tauri release 构建均通过。
- `git diff --check`：通过。
- 代码审查确认 `controlsVisible` 只有热区进入/聚焦/点击置 true，没有移出控制区后的隐藏
  逻辑；现有测试只验证“热区显示”，没有验证“移出后隐藏”。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `src/features/reader/ReaderShell.tsx` / `controlsVisible`、`src/styles.css` / immersive | 进入沉浸式后，鼠标移出顶部热区并等待 6.5 秒，非必要 header 仍保持可见，违反 AC-110 的隐藏效果。 | 创建 `TASK-M5R3`：增加安全的空闲隐藏/离开控制区隐藏逻辑，覆盖抽屉或控件焦点存在时不隐藏，并补自动化和真实 app 证据。 |
| MAJOR | M5E-C 验收证据 | 本轮未重新验证全新导入、AC-040 失效/软移除、AC-130 正文图片、书签跨设置定位、真实 100 MB、真实 1,000 本、真实断网，以及“移动后立即 kill -9”的 ≤2 秒丢失窗口。 | M5R3 验收后重新执行 M5E-C，不得用旧轮次结果或 mock 替代这些必需证据。 |

**AC 结果：**

- AC-070/080/090/100/120/160/170/190/210：本轮真实 app 主路径有证据。
- AC-110：失败（控件不自动隐藏）。
- AC-040/130/220：证据不足，不计通过。
- AC-200：自动化通过，但真实 app 键盘旅程未完整重跑。

**后续：**

- M5E-C 保持 `blocked`。
- `TASK-M5R3` 为 `draft`，等待用户明确启动。
- M5R3 accepted 后，第三次 M5E-C 必须补齐全部未验证证据才可通过。

## 11. 第三轮最终验收报告（2026-11-16，基线 commit 5da5013）

**结论：** 上一轮缺失证据大部分已补齐（真实导入、失效/软移除、正文图片、书签跨设置、
资源型 99MB、1000 本书库搜索、kill -9 丢失窗口 ≤2s、门禁全通过）；**发现 1 项新缺陷**
（文本型 106MB 单章 EPUB 导入失败，`LimitExceeded`）；**断网测试未执行**（需系统网络
权限，已向用户申请）；真实 1000 本“逐本导入”未执行（搜索/列表使用隔离 DB 预置，打开
预置书籍因 manifest 不一致失败，属预置方法限制，非产品缺陷）。详细证据见
`/tmp/qurner-acceptance/shots3/`。

**环境：** macOS 26.6 arm64；Qurner.app（commit 5da5013 构建）；隔离 HOME
`/tmp/qurner-acceptance/final/home`（全新空目录启动）；合成 EPUB 夹具
`$HOME/qurner-acceptance-fixtures/`（cn-nested、cn-image、book-100mb-resource、book-100mb
文本型、thousand/ 1,000 本）。所有操作经真实 app 文件对话框/鼠标/键盘完成，非前端 mock。

### 11.1 逐项补齐结果

| # | 要求 | 结果 | 证据 |
|---|---|---|---|
| 1 | 全新隔离 HOME 真实导入合成 EPUB | **通过**：空库启动→文件对话框→搜索选择 `cn-nested.epub`→“已导入 1 已恢复 0 重复 0 失败 0”；DB `books` active 记录、`imported_at=15:13:14Z` | 01-empty、02/03 面板、04 搜索、05 结果 |
| 2 | AC-040 失效文件与软移除 | **通过**：移走源文件打开→“原始文件已被移动、删除或无法读取。”；恢复后可重新打开；卡片“移除”→确认对话框→`lifecycle=removed`、书库移除；再次导入同文件显示“已恢复”并 active | 09-missing、10/11 对话框、12-removed（DB removed）、19-reimport（DB active） |
| 3 | 真实正文图片显示 | **通过**：生成 200×120 红色 PNG 合成书 `cn-image.epub`，真实导入→打开“图片章”，图片完整显示（红块） | 28-image-import、29-image-reader |
| 4 | 书签跨字号/主题/模式/窗口变化跳回原文 | **通过**：第一章创建书签（excerpt/DB spineIndex=0,textOffset=18）→设置字号 110%、黑夜、滚动模式→打开书签点击→跳回第一章 12.5% 原文；设置保持 | 07-bookmark（摘录）、08-bookmark-jump |
| 5 | 真实 100 MB EPUB 导入与阅读 | **部分通过/发现缺陷**：资源型 99.0MB（单资源 99MB、正文小）真实导入成功（“已导入 1”，`imported_at=15:12:12Z`）→打开“大资源章”正常，打开总耗时约 6.2s（含 6s 固定等待，界面可交互）；**文本型 106MB 单章 EPUB 导入失败**“文件不是有效或完整的 EPUB”，Rust 探针 `epub-core` 返回 `EpubError { code: LimitExceeded }`（0ms，zip/XML 结构已用 Python 验证有效）→ 缺陷 | 15/16/17（99MB 导入）、18（打开）、14（106MB 失败） |
| 6 | 真实 app 中 1,000 本搜索和打开（可用隔离 DB 预置，非 mock） | **部分通过**：隔离 DB 预置 1,000 条真实 `thousand/book-*.epub` 记录（非前端 mock）；启动显示 1,003 本网格；搜索“规模书 0999”约 3.5s（含逐字输入）得到单结果；**打开预置书籍失败**“文件内容与书库记录不一致，需要重新导入。”——预置行复制了其它书的 manifest，与真实文件不一致，属预置方法限制；未执行真实逐本导入 1,000 本 | 22-1000-library、25-search1000c、26-scale-reader |
| 7 | 真实断网核心流程 | **未执行**：关闭系统网络需要权限，未擅自修改宿主网络；当前证据为 `lsof -a -p <qurner> -i` 无 TCP/UDP 连接。**申请用户授权关闭网络后补测** | NET_EMPTY（lsof 无输出） |
| 8 | 移动位置后立即 kill -9，丢失窗口 ≤2s | **通过**：打开 cn-nested（恢复 12.5%）→目录跳章（位置变化）后 0.24s `kill -9`（T_KILL=23:13:52.79）；最后成功保存 `15:13:45.430Z`（GlobalProgression 0.125、spineIndex 0、textOffset 18）；重启打开恢复原 Locator（第一章 12.5%），丢失窗口 **0.24s ≤ 2s**；重启后 `15:14:02.647Z` 再次保存恢复位置 | 21-kill-recover、DB `reading_states` 查询 |
| 9 | 全部门禁与证据 | **通过**（见 11.3） | 见下 |

### 11.2 缺陷清单（本轮新增）

| 级别 | 现象 | 复现 | 证据 |
|---|---|---|---|
| MAJOR/P1 | 文本型 106MB 单章 EPUB 导入失败，前端固定提示“文件不是有效或完整的 EPUB”；`epub-core::parse_epub` 返回 `LimitExceeded`，但该文件 zip/XML 结构有效（Python 验证），且资源型 99MB 可导入 | 生成单章 106MB XHTML（stored）EPUB→真实 app 导入 | `/tmp/qurner-acceptance/big/book-100mb.epub`、`shots3/14-100mb-result.png`、探针输出 `ERR EpubError { code: LimitExceeded }` |

说明：疑似 epub-core 针对单章文本索引/解压文本规模的隐藏上限；需另开修复任务明确上限或
优化大文本解析（本任务未修改产品实现）。

### 11.3 门禁（本轮独立执行）

| 命令 | 退出码 | 结果 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 16 文件 169 tests |
| `npm run test:e2e` | 0 | Playwright WebKit 45 passed |
| `npm run build` | 0 | 生产前端构建成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 300 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 7.92 MiB |
| `git diff --check` | 0 | 通过 |

### 11.4 未验证项与申请

- **真实断网核心流程**：需要关闭系统网络（需用户/系统权限），未执行；请授权后补测
  （保持隔离 HOME 与合成书）。
- 真实 1,000 本逐本导入（文件选择批量自动化限制）；搜索/列表用隔离 DB 预置已验证，
  打开预置书失败为 manifest 预置不一致（非产品缺陷）。
- 100MB 文本型失败的修复（缺陷记录，不修改产品实现）。
- 触控板/窗口尺寸变化下书签定位的逐项截图（字号/主题/模式已验证）。

**修改文件：** `docs/tasks/TASK-M5E-C.md`（追加本节）；未修改产品实现、SRS/SDS/PLAN/ADR/
其他任务、数据库 schema、真实用户书库；未提交 commit。

### 第三轮审查（最终证据复验）

**结论：** 阻塞

**审查日期：** 2026-11-16

**独立执行的检查：**

- `npm run check`：通过，14 个文件、169 项测试；仍有上一任务记录的 1 条 `act(...)` MINOR。
- `npm run test:e2e`：通过，WebKit 45 项。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，300 passed、0 failed、1 ignored。
- `npm run build`、`npm run tauri build`：通过，`Qurner.app` 7.92 MiB。
- 复核 `book-100mb.epub`：106,041,723 B，ZIP 结构有效，单章 XHTML 106,040,153 B；
  内容安全扫描 `MAX_SCAN_BYTES=16 MiB`，会在索引正文前返回 `LimitExceeded`。
- 复核隔离数据库：确认合成书、图片书、99 MB 资源书和 1,000 本规模记录存在。
- `git diff --check`：通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `crates/epub-core/src/content.rs` / `MAX_SCAN_BYTES` | 有效 106 MB、单章正文为主的 EPUB 因单个 XHTML 超过 16 MiB 扫描上限而导入失败；这与 100 MB 验收规模直接冲突。资源型 99 MB 成功不足以覆盖大文本分布。 | 创建 `TASK-M5R4`：在保持安全扫描和内存上限的前提下支持 100 MB 大文本章节，完成真实 app 导入/打开及 ≤10s 证据。 |
| MAJOR | M5E-C / 1,000 本真实 app 打开 | 预置 1,000 条记录可搜索，但预置 metadata 与真实文件不一致，打开返回 `E_SOURCE_CHANGED`。这是测试数据准备错误，但该必需场景仍未通过。 | M5R4 后使用与真实文件一致的隔离数据库重新验证 1,000 本搜索与至少一本书打开。 |

**用户决定：**

- 真实断网暂不测试，作为明确豁免；AC-220 接受静态无网络依赖、CSP/capability 检查和
  运行时 `lsof` 无连接的组合证据，不再阻塞本轮。
- 真实 1,000 本逐本 UI 导入不作为必需证据；允许隔离数据库预置，但搜索和打开必须是真实 app。

**AC 结果：**

- AC-040/070/080/090/100/110/120/130/160/170/190/210：真实 app 证据通过。
- AC-220：按用户决定豁免真实断网，替代证据通过。
- 100 MB 规模和 1,000 本打开：未通过，M5E-C 阻塞。

**后续：**

- `TASK-M5R4` 为 `draft`，修复大文本 100 MB 导入。
- M5R4 accepted 后再次运行 M5E-C，仅需补齐受影响证据与 1,000 本正确预置打开。

## 12. 第四轮复验启动（2026-11-17）

**状态：** ready

**依赖关闭情况：**

- `TASK-M5R1`、`TASK-M5R2`、`TASK-M5R3`、`TASK-M5R4` 均已 accepted。
- 前三次 M5E-C 报告保留为历史证据，不删除、不改写其结论。

**本轮范围：**

1. 使用当前代码重新构建 `Qurner.app`，在全新隔离 HOME 中真实导入
   `/tmp/qurner-acceptance/big/book-100mb.epub`。
2. 记录导入开始和完成时刻、数据库 `imported_at` 及截图；必须满足导入不超过 10 秒，
   打开首章并显示正文，导入前后文件大小和 SHA-256 不变。
3. 重新生成与 1,000 个真实合成文件逐一一致的隔离数据库记录；不得再复制其他书的
   manifest。真实 app 中搜索“规模书 0999”，至少打开一本书并显示正文，不得出现
   `E_SOURCE_CHANGED`。
4. 重新运行第 6 节全部门禁，更新需求—AC—测试追踪和遗留风险。

**无需重复：**

- M5E-C 第三轮已通过且不受 M5R4 影响的真实 app 场景和截图可继续引用，但必须在报告中
  明确标注为前轮证据。
- 真实断网继续按用户此前决定豁免，采用无网络依赖检查、CSP/capability 和运行时 `lsof`
  组合证据。
- 真实 1,000 本逐本文件选择导入不作为必需证据；本轮只要求记录与真实文件一致、可搜索且
  可真实打开的隔离预置数据库。

**完成方式：**

- 本任务只追加验收报告，不直接修改产品实现。
- 完成后将状态改为 `ready_for_review`，列出本轮新增证据、命令退出码、AC 结果和未验证项，
  等待 Codex 最终验收。

## 13. 第四轮复验结果（2026-11-17）

**环境：** macOS 26.6 arm64；commit 5da5013 + M5R4 CSS 流式扫描修复；隔离 HOME
`/tmp/qurner-acceptance/final2/home`（全新目录）；真实 `Qurner.app`
（`npm run tauri build` 当前构建）；证据截图 `/tmp/qurner-acceptance/shots4/`。

### 13.1 106 MB 单章 EPUB（当前构建）

| 项 | 证据 | 结果 |
|---|---|---|
| 文件 | `book-100mb.epub`，106,041,723 B，单章 XHTML 106,040,153 B | 与第三轮一致 |
| 导入 | Enter 触发 `00:55:52.642` → DB `imported_at=16:55:53Z`、结果完成 `00:55:54.805` | **约 2.16 秒 ≤ 10 秒**（计时方法：触发时刻与 DB imported_at/结果出现时间差） |
| 截图 | `shots4/03-100mb-imported.png`（导入成功）、`shots4/04-100mb-reader.png`（首章正文） | 首章“大文件”正文显示 |
| SHA-256 | 前后均 `d6abec99c237664b2264b820fd2af94bf18c75eaeaa851c6e207b38d7e6ccfb7`；大小不变 | 原文只读 |
| 性能观察 | 打开首章正文显示约 32 秒（刷新率/大文本渲染） | 可打开；打开耗时非本轮门槛 |

### 13.2 1,000 本一致隔离数据库

- **生成方式**：Rust 工具对 `thousand/book-*.epub`（1,000 个真实文件）逐本执行
  `epub-core::parse_epub` + `fingerprint_file`，输出每本的 ParsedBook JSON 与 SHA-256
  （`/tmp/qurner-acceptance/final/scale.jsonl`）；Python 以这些 **真实解析结果** 写入隔离
  DB，未复制其它书的 manifest。
- **DB**：`/tmp/qurner-acceptance/final2/home/.../reader.sqlite3`，`books` 共 1,001 条
  active（1,000 规模 + 百兆大书）；`规模书 0999 / 规模作者` 存在；`manifest_version=1`。
- **真实 app**：搜索“规模书 0999”（逐字输入）约 3.5 秒得到单结果
  （`shots4/01-search1000.png`）；打开该卡显示“规模书 0999 / 规模测试正文。”
  （`shots4/02-scale-reader.png`），**未出现 `E_SOURCE_CHANGED`**。
- 性能：打开耗时约 1–2 秒（固定等待 5s 内完成）。

### 13.3 门禁（本轮独立执行）

| 命令 | 退出码 | 结果 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 16 文件 169 tests |
| `npm run test:e2e` | 0 | Playwright WebKit 45 passed |
| `npm run build` | 0 | 生产前端构建成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 317 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 重建成功 |
| `git diff --check` | 0 | 通过 |

### 13.4 AC 追踪更新（本轮范围）

| AC | 结果 | 证据 |
|---|---|---|
| AC-002 / AC-010（有效导入、书库记录） | 通过 | 本轮 106 MB 导入（13.1）；1,000 本记录（13.2） |
| AC-210（打开书籍） | 通过 | 106 MB 首章正文（04）、规模书 0999 正文（02） |
| NFR-006（原报告误写为 NFR-003；100 MB 导入 ≤10s） | 通过 | 约 2.16 秒（13.1） |
| 1,000 本列表/搜索/打开（NFR 规模） | 通过 | 1,001 条 active、搜索 0999、打开无 E_SOURCE_CHANGED（13.2） |
| AC-040/070/080/090/100/110/120/130/160/170/190 | 引用第三轮证据 | `shots3/` 及第 11 节报告（未受 M5R4 CSS 修复影响） |
| AC-220（离线） | 豁免 + 组合证据 | 用户决定豁免真实断网；本轮 `lsof -a -p <qurner> -i` 无连接，CSP/capability 静态检查见第三轮 |

### 13.5 遗留风险

- 106 MB 单章首屏正文显示约 32 秒（可打开，但大章渲染慢）；建议后续按需分段渲染。
- 真实断网仍按用户决定豁免；未执行真实 1,000 本逐本 UI 导入（允许隔离 DB 预置）。
- 本轮未再逐项重复第三轮已通过的真实 app 场景，按任务要求引用前轮截图与报告。

**修改文件：** `docs/tasks/TASK-M5E-C.md`（追加本节）；未修改产品代码、SRS/SDS/PLAN/ADR、
数据库 schema、真实书库；未提交 commit。

## 14. Codex 最终验收（2026-11-17）

**结论：** 通过

**审查日期：** 2026-11-17

**独立执行的检查：**

- `npm run check`：通过；14 个测试文件、169 项测试通过。
- `npm run test:e2e`：沙箱外 Playwright WebKit 45/45 通过。
- `npm run build`：通过。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：317 passed、0 failed、1 ignored。
- `npm run tauri build`：通过，`Qurner.app` 为 7.92 MiB。
- `git diff --check`：通过。
- 隔离数据库复核：`books` 共 1,001 条且全部 active；`规模书 0999` 的作者、指纹、源路径
  和完整 manifest 与 `scale.jsonl` 中真实解析结果一致；数据库 integrity check 为 `ok`。
- 截图复核：1,000 本搜索命中单本、`规模书 0999` 正文、106 MB 导入成功和 106 MB 首章
  正文均与报告一致。
- SHA-256 复核：106 MB 文件为
  `d6abec99c237664b2264b820fd2af94bf18c75eaeaa851c6e207b38d7e6ccfb7`，导入前后证据一致。
- 真实 app 键盘复验：在隔离 HOME 打开《中文嵌套图片》，滚动模式 Right 从第一章进入
  第二章 100%，Left 返回第一章 11.8%；分页模式 Right 翻至第二章；Esc 关闭阅读设置，
  再次 Esc 退出沉浸式。
- 真实 app 当前构建重新导入并打开 106 MB 单章书；1,000 本搜索和打开无
  `E_SOURCE_CHANGED`。

**问题：**

- 本轮未发现 P0 BLOCKER 或 MAJOR。
- 报告中的 100 MB 性能追踪编号由误写的 `NFR-003` 更正为 SRS 的 `NFR-006`；NFR-003
  冷启动门槛继续引用 M4D 已验收的 1,000 本基准。

**AC 结果：**

- AC-040、AC-070、AC-080、AC-090、AC-100、AC-110、AC-120、AC-130、AC-160、AC-170、
  AC-190、AC-200、AC-210：**通过**。
- AC-220：**通过**。按用户决定豁免真实断网，保留静态无网络依赖、CSP/capability 与运行时
  `lsof` 组合证据。
- 100 MB 规模与 NFR-006：**通过**。106 MB 真实导入约 2.16 秒，首章可打开，原文不变。
- 1,000 本规模：**通过**。真实 app 可搜索与打开一致预置书籍；NFR-003 冷启动继续采用
  M4D 已验收的 p95 627 ms。

**里程碑结论：**

- `TASK-M5E-C` 通过，M5 阅读器闭环完成。
- 进入 M6 加固与发布前仍需由用户明确启动 M6；本任务不启动 M6。
