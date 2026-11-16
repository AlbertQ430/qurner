# TASK-M5E-C：M5 阅读器闭环总验收

**状态：** blocked

**里程碑：** M5

**负责人：** Codex + DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-15

**依赖：** TASK-M5E-A、TASK-M5E-B、TASK-M5R1、TASK-M5R2 accepted

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
  用户数据）；合成 EPUB 源文件 `/Users/quzhuoyan/qurner-acceptance-fixtures/cn-nested.epub`。
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
