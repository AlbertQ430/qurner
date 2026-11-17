# TASK-M6C：SRS 全追踪与发布候选验收

**状态：** accepted

**里程碑：** M6

**负责人：** Codex + DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-17

**依赖：** TASK-M6A、TASK-M6B accepted

## 1. 目标

完成 SRS 1.0 全量需求追踪、发布候选 `Qurner.app` 和已知限制说明，证明 MVP 发布门槛
全部满足；本任务不自动发布或修改远程状态。

## 2. 范围

### 必须完成

- 生成 FR、AC、NFR、任务、自动化测试和真实 app 证据的完整追踪表。
- 独立验证 AC-NFR-001 至 AC-NFR-005 和 SRS 7.3 发布门槛。
- 使用当前正式构建运行最终门禁并生成 `Qurner.app`。
- 验证退出、重新启动、同 HOME 恢复书库、设置、进度和书签。
- 补充含远程资源 EPUB 的真实 app 冒烟：确认无出站请求、资源被阻断、页面有稳定降级，
  并记录截图；承接 `TASK-M6A` 的 MINOR 发布前复核项。
- 对照 README 更新开发运行、构建、支持平台、隐私、数据和已知限制。
- 汇总所有 BLOCKER/MAJOR；未关闭时不得通过。
- 形成 M6 最终验收报告，确认是否达到发布候选状态。

### 不属于本任务

- 签名、公证、GitHub Release、应用商店发布或其他外部发布动作。
- 新功能、未证实的架构重构或推迟到 V1.x/V2.x 的需求。

## 3. 需求追踪

| 类型 | 编号 | 验收要求 |
|---|---|---|
| FR | SRS P0 全部 FR | 全覆盖且无遗漏 |
| AC | AC-001–AC-220 中全部 MVP AC | 每项有自动化或真实 app 证据 |
| NFR | NFR-001–NFR-015 | 适用项全部验证，限制明确记录 |
| AC | AC-NFR-001–AC-NFR-005 | 性能、持久化、隐私、错误和兼容性 |

## 4. 设计约束

- 以 SRS 1.0 和 SDS 1.0 为唯一正式基线。
- 引用历史任务证据时必须标明日期、commit、夹具和适用范围。
- 发布候选产物必须来自当前 commit 的干净工作区；验证后记录 SHA-256。

## 5. 文件权限

### 允许修改

- `docs/acceptance/`
- `docs/tasks/TASK-M6C.md`
- `tests/e2e/` 中必要的追踪/发布冒烟测试
- `README.md` 的交付、运行和已知限制说明
- 用户于 2026-11-18 额外授权的最小主题修复：
  `src/reader-engine/engine.ts`、`src/reader-engine/engine-settings.test.tsx`、
  `src-tauri/resources/reader-engine.js`、`src/styles.css`

### 禁止修改

- 产品实现、SRS/SDS/PLAN/ADR、schema、协议、依赖和权限。
- 原始 EPUB、真实书库和个人数据。

## 6. 行为与边界

- 每个 P0 AC 必须能追溯到实现和有效证据。
- 人工视觉验收必须记录操作系统、屏幕、窗口尺寸和截图。
- 任何已知限制必须在 README 和验收报告中同时出现。

## 7. 测试要求

- 独立运行全部最终门禁。
- 真实 app 发布候选冷启动、重启恢复和完整用户旅程。
- 核对产物架构、大小、SHA-256 和 Bundle Identifier。
- 输出最终 GO/NO-GO 建议；是否发布由用户决定。

## 8. 必跑命令

```bash
npm run check
npm run test:e2e
npm run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run tauri build
git diff --check
```

## 9. 禁止事项

- 不把旧证据无范围说明地直接复制为当前通过。
- 不把发布候选误称为已发布。
- 不签名、公证、推送、创建 Release 或修改远程状态。
- 不隐藏已知限制、失败测试或人工验收缺口。

## 10. DeepSeek 开发报告

**完成摘要（返工后）：**

按审查处理四项 MAJOR 与两项文档问题：

1. **基线**：改为 HEAD `e01bf7d`（`5da5013..e01bf7d` 含 M5R4 的 `crates/epub-core/src/content.rs`
   等产品文件），发布候选来自当前 HEAD 构建。
2. **macOS 14 兼容性**：产物 `LSMinimumSystemVersion=14.0`、`LC_BUILD_VERSION minos 14.0`
   （SDK 26.5）+ macOS 26.6 实机运行；**用户于 2026-11-18 明确豁免 macOS 14 实机实测**
   （风险自担）。NFR-001/AC-NFR-001 按豁免口径通过。
3. **NFR-005 录制器**：修复窗口选择（标题 `Qurner` + 宽高 ≥800×500 + 最大面积；无匹配
   时列出候选并退出 2）；重跑成功：`selected window title=Qurner size=1000x720`，
   20/20 样本，58.09 fps，最大帧间隔 34.16 ms，p50 41.92 ms，**p95 54.19 ms ≤150 ms**
   （保守值），退出码 0；视频 SHA-256 `2329d0…3a99`。
4. **夜主题瞬态（已修复）**：经用户授权，`src/reader-engine/engine.ts` 改为先同步设置
   正文背景/文字色、再切换 `qurner-night` 类；`engine-settings` 测试增加夜→日恢复断言；
   `npm run build:reader` 重建 `reader-engine.js`，`npm run tauri build` 重建产物。
   真实 app 连续切换白天/夜间 20 次，切换后立即截图正常（`m6b/shots/111-theme-day-1.png`、
   `112-theme-night-1.png`）。
5. **系统深色外观下外壳/抽屉浅底浅字（用户复现，已修复）**：dark media 只覆盖了书库/导入，
   未覆盖阅读器外壳；系统深色外观下 `body` 文字变浅而外壳/抽屉仍浅底 → 浅底浅字。已在
   `src/styles.css` 的 dark media 中为 `.reader-shell/.reader-header/.reader-settings/
   .reader-toc/.reader-bookmarks` 固定浅底 + `color:#26221c`；真实 app 截图
   `m6b/shots/120-dark-settings-fixed.png`、`121-dark-toc-fixed.png`。
6. **Vitest 文件数**：更正为 17 文件 220 tests。
7. **README**：改为“M0–M5 完成，M6A/M6B 通过，M6C 验收进行中”，不提前宣称完成；
   已知限制、隐私/数据、发布候选产物信息同步。

**修改文件：**

- `docs/acceptance/TASK-M6C-REPORT.md`
- `docs/tasks/TASK-M6C.md`
- `README.md`
- `scripts/m6b_turnpage_recorder.swift`（窗口选择修复，验收脚本）
- `src/reader-engine/engine.ts`（用户授权：主题切换顺序）
- `src/reader-engine/engine-settings.test.tsx`（夜间→白天恢复断言）
- `src-tauri/resources/reader-engine.js`（由源码重建）
- `src/styles.css`（用户授权：系统深色外观下阅读器壳层与抽屉对比度）

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 17 文件 220 tests |
| `npm run test:e2e` | 0 | WebKit 46 passed |
| `npm run build` | 0 | 成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 320 passed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app（arm64，7.9 MB） |
| `git diff --check` | 0 | 通过 |
| `/tmp/m6b-recorder --samples 20` | 0 | 20/20；p95 54.19 ms；58.09 fps |

**发布门槛与 AC 追踪：**

P0 FR 87/87 被 AC 覆盖；AC/NFR 表见报告；SRS 7.3 全部通过（NFR-001 采用用户豁免的
macOS 14 证据口径）；结论 **GO**。

**发布候选产物：**

`Qurner.app`（arm64、7.9 MB、bundle `com.qurner.reader`、版本 0.1.0、SHA-256
`f9c0909a…15da`、minos 14.0、未签名/未公证）；发布候选就绪。

**未验证项：**

- macOS 14 实机冒烟（已由用户豁免）。
- 真实整机关网（沿用进程级 sandbox 证据）。

**风险与待决问题：**

- **GO（发布候选就绪）**：macOS 14 实机证据由用户豁免；夜主题瞬态已修复并 20 次切换
  验证通过。签名/公证/发布由用户决定；未提交 commit。

## 11. Codex 审查

**结论：** 返工
**审查日期：** 2026-11-17

**独立执行的检查：**

- `npm run check`：通过，TypeScript/Biome 通过，Vitest 17 个文件、220 项通过。
- `npm run test:e2e`：沙箱内 WebKit 启动因 macOS 应用注册限制以 `Abort trap: 6`
  失败；经批准在沙箱外复跑，WebKit 46/46 通过。
- `npm run build`：通过。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，320 passed、0 failed、1 ignored。
- `npm run tauri build`：通过；`Qurner.app/Contents/MacOS/qurner` 为 8,153,200 B、
  Mach-O arm64，SHA-256
  `e4127257a9a72438cadd2a1a1ec40b103fb73beaa0f34c69fc77339d14d08684`，与报告一致。
- `git diff --check`：通过。
- `python3 scripts/m6b_benchmark.py all --reset --runs 10`：通过；冷启动 p95 588.4 ms，
  搜索 p95 0.61 ms，`rows=1`。
- 独立启动隔离 HOME 中的 release app：书库 3 本、长书恢复 32.3%、字号 110%、黑夜设置
  和 1 条书签均恢复；远程资源书的本地图可见、远程图显示 alt 占位，`lsof -a -p <pid> -i`
  无连接。
- `/tmp/qurner-review-m6c/m6b-recorder --samples 20`：连续两次失败；录制器选中
  66×20 pt 窗口，得到无效正文 ROI `[40,128]-[92,-8]`、0 个有效样本并返回退出码 1。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `docs/acceptance/TASK-M6C-REPORT.md:71`、`README.md:29` | NFR-001 要求 macOS 14+ Apple Silicon 可启动并完成核心流程；当前唯一运行证据是 macOS 26.6。`Info.plist` 的 `LSMinimumSystemVersion=14.0` 只声明目标，不能证明 macOS 14 兼容性，且该缺口未列入已知限制。 | 在 macOS 14 Apple Silicon 上运行发布候选核心流程并记录系统版本、结果和截图；若无法取得环境，交由用户决定是否正式修改支持平台基线及 SRS。 |
| MAJOR | `docs/acceptance/TASK-M6C-REPORT.md:7`、`docs/tasks/TASK-M6C.md:52` | 报告称产品实现对应 commit `5da5013`，但当前 HEAD 为 `e01bf7d`，且两者之间修改了产品文件 `crates/epub-core/src/content.rs`；当前工作区也不是任务要求的干净工作区。产物哈希可复现，但源码与 commit 的映射不成立。 | 以当前实际 HEAD `e01bf7d` 为基线，记录产品源文件相对 HEAD 的差异和构建命令，证明 release 输入可追溯；若坚持“当前 commit 干净工作区”硬门槛，需由用户决定如何处理未提交工作。 |
| MAJOR | `scripts/m6b_turnpage_recorder.swift:273` | NFR-005 的仓库内录制器只取 bundle id 匹配的第一个窗口，未校验窗口尺寸或主窗口属性；独立复跑连续两次选中 66×20 pt 错误窗口，正文 ROI 反向，20 个样本全部无效。M6C 因此没有可重复的当前构建 NFR-005 证据。 | 过滤为可见、足够尺寸的主阅读窗口，并在启动录制前校验窗口尺寸与正文 ROI；修复后以当前 release 重新采集 20 个有效样本和 p95。 |
| MAJOR | `docs/acceptance/TASK-M6C-REPORT.md:56`、`/tmp/qurner-acceptance/m6b/shots/104-settings-night.png`、`/tmp/qurner-acceptance/m6b/shots/106-restored-reader.png` | AC-100 引用截图标注“黑夜可读”，但截图实际为浅色背景上的近白色文字，正文和顶部控件难以辨认。独立启动当前 release 时也能复现约半秒的浅底白字，随后才稳定为深色正文；现有自动化未覆盖恢复黑夜主题的首次呈现。 | 定位主题异步应用时序或样式覆盖根因，补充真实 app 恢复黑夜主题的可见性回归测试；重新截图并仅引用可读的稳定状态。 |
| MINOR | `docs/acceptance/TASK-M6C-REPORT.md:28`、`docs/tasks/TASK-M6C.md:122` | Vitest 实际为 17 个测试文件，报告写 19 个文件。测试数 220 正确。 | 更正为 17 个文件。 |
| MINOR | `README.md:7`、`README.md:19` | 当前任务尚未验收即写“M0–M6 已完成”；已知限制也未包含 macOS 14 未验证和黑夜主题首次呈现问题。 | 在本任务接受前改为 M6 待验收；补齐所有已确认限制并在验收报告中同步。 |

**AC 结果：**

- P0 FR → AC：静态核对为 87/87 覆盖，通过。
- 最终门禁、发布产物架构/大小/哈希、远程资源阻断、同 HOME 书库/进度/设置/书签恢复：
  通过。
- AC-NFR-002、AC-NFR-003、AC-NFR-004、AC-NFR-005：沿用已验收证据，未发现新问题。
- AC-NFR-001：**未通过验收**，macOS 14 实机/等价环境未验证。
- NFR-003、NFR-004：当前 release 独立基准通过。
- NFR-005：历史视频仍满足报告数值，但仓库录制器当前无法稳定选中主窗口，**待复验**。
- AC-100：当前截图证据与“黑夜可读”结论不符，**待复验**。

**发布建议：**

**NO-GO（暂不可接受为发布候选）**。修复上表 MAJOR 并更新证据后再审查；是否提交、签名、
公证或发布仍由用户决定。

### 第二轮审查（返工复验）

**结论：** 阻塞
**审查日期：** 2026-11-17

**独立执行的检查：**

- `npm run check`：通过，Vitest 17 个文件、220 项通过；文件数已更正。
- `npm run test:e2e`：首次完整运行 45/46，通过“沉浸式隐藏控件”用例等待热区时超时；
  单独复跑该用例通过，随后再次完整运行 46/46 通过。
- `npm run build`、`cargo fmt --all --check`、Clippy、`cargo test --workspace`：
  全部通过；Rust 为 320 passed、0 failed、1 ignored。
- `npm run tauri build`：通过；产物为 8,153,200 B、arm64，SHA-256
  `e4127257a9a72438cadd2a1a1ec40b103fb73beaa0f34c69fc77339d14d08684`；
  `Info.plist` 和 `LC_BUILD_VERSION` 的最小系统版本均为 14.0。
- `python3 scripts/m6b_benchmark.py all --reset --runs 10`：通过；冷启动 p95 595.9 ms，
  搜索 p95 0.48 ms，`rows=1`。
- 修复后的录制器：通过；选中 `1000x720` 主窗口，正文 ROI
  `[40,128]-[1960,1392]`，20/20 样本，58.00 fps，p95 75.59 ms，退出码 0。
- `git diff --name-only`：tracked 差异中没有产品运行时代码；产品实现与 HEAD `e01bf7d`
  的映射已基本可追溯。工作区仍不“干净”，但差异为文档/测试。
- 复核 `m6b/shots/109-night-stable.png`：正文最终为深底浅字且可读，但顶部“返回书库”、
  目录、书签、设置等控件仍为浅底上的近白色文字，稳定状态对比度不足。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `docs/acceptance/TASK-M6C-REPORT.md:74`、`docs/acceptance/TASK-M6C-REPORT.md:121`、`docs/acceptance/TASK-M6C-REPORT.md:145` | NFR-001 仍明确为“未在 macOS 14 硬件实测”，但同报告第 7 节又写 AC-NFR-001–005 与 SRS 7.3 全部通过，并给出带两项必办事项的条件 GO，验收结论自相矛盾。 | 取得 macOS 14 Apple Silicon 实机冒烟证据，或由用户明确接受当前仅静态部署目标的限制并相应修改验收/支持平台口径。 |
| MAJOR | `/tmp/qurner-acceptance/m6b/shots/109-night-stable.png`、`docs/acceptance/TASK-M6C-REPORT.md:109` | 夜间主题稳定后正文可读，但顶部阅读控件仍为浅色背景上的近白色文字，实际不可辨认；报告只记录主题切换瞬态，没有覆盖稳定态控件对比度问题。 | 修复夜间主题下壳层控件的背景/文字配色并补真实 app 视觉回归；本任务禁止修改产品实现，需要用户授权另开返工或接受该已知限制。 |
| MINOR | `scripts/m6b_turnpage_recorder.swift:282` | Swift 编译对可选窗口标题的字符串插值产生两条 warning；不影响运行和门禁。 | 明确处理可选标题，消除编译警告。 |
| MINOR | `README.md:22` | README 的翻页 p95 仍写历史值 60.75 ms，而本轮复验为 75.59 ms；两者都满足门槛，但证据数字未同步。 | 改为最新可复核结果，或明确标注为某次历史测量。 |
| MINOR | `tests/e2e/library.spec.ts:340` | 沉浸式热区用例首次完整运行超时，后续单独和全量复跑均通过；当前属于间歇性测试不稳定。 | 若再次复现，固定等待阅读器 ready/沉浸式状态后再查询热区，避免竞态。 |

**AC 结果：**

- 基线、Vitest 文件数和 NFR-005 录制器选择问题：已修复并独立复验。
- P0 FR-AC 覆盖、所有自动化门禁、产物哈希、当前 release 的 NFR-003/004/005：通过。
- AC-NFR-001 / NFR-001：仍未实际验证 macOS 14，不能标记通过。
- AC-100：正文最终可读，但稳定夜间主题下阅读控件对比度不足，不能按当前证据标记完整通过。

**发布建议：**

**阻塞，暂不计为已接受的发布候选。** 需要用户决定是否补 macOS 14 实机验证和夜间壳层
控件修复；若选择接受为已知限制，必须由用户明确确认，并同步修订报告、README 和 SRS
支持口径后再进行最终验收。

### 第三轮审查（最终验收）

**结论：** 通过
**审查日期：** 2026-11-18

**独立执行的检查：**

- `npm run check`：通过，Vitest 17 个文件、220 项；夜间→白天恢复断言通过。
- `npm run test:e2e`：首次完整运行 45/46，沉浸式用例因热区状态竞态失败；关闭真实 app
  后完整复跑 46/46，通过该用例单独 `--repeat-each=10` 仍为 10/10。
- `npm run build`、`npm run tauri build`：通过；产物 8,153,200 B、arm64、7.9 MB，
  SHA-256 `f9c0909abdf0a18785217d8b3f6926e313f527eaafda4fcb5db8004511a515da`，
  与 README/验收报告一致。
- `cargo fmt --all --check`、Clippy、`cargo test --workspace`：通过；320 passed、
  0 failed、1 ignored。
- `python3 scripts/m6b_benchmark.py all --reset --runs 10`：通过；冷启动 p95 593.9 ms，
  搜索 p95 0.8 ms，`rows=1`。
- 60 fps 录制：首次因正文未获焦点未检出变化，点击正文聚焦后 20/20 样本有效，58.34 fps，
  p95 69.20 ms，退出码 0。
- 当前隔离 HOME release app：夜间正文为深底浅字，顶部按钮与设置抽屉为浅底深字；
  日/夜切换即时可读，20 张白天/夜间截图均存在。
- 产物 `LSMinimumSystemVersion=14.0`、`LC_BUILD_VERSION minos=14.0`；macOS 14 实机
  证据按任务记录的 2026-11-18 用户豁免口径验收。
- `git diff --check`：通过。按用户“不创建 commit”的约束，验收对象是当前工作区快照；
  报告已明确列出未提交的主题修复文件。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| NIT | `scripts/m6b_turnpage_recorder.swift:282-286` | 可选窗口标题插值产生 Swift warning；录制前若正文未获焦点会得到 0 样本并正确失败。 | 后续工具维护时消除 warning，并把“先点击正文聚焦”写入脚本说明。 |
| NIT | `tests/e2e/library.spec.ts:340` | 5-worker 全量运行已出现一次热区竞态；关闭真实 app 后全量、独立重复均通过，没有观察到产品行为失败。 | 后续维护时可加固沉浸式用例的指针/热区等待条件。 |

**AC 结果：**

- P0 FR 87/87 覆盖、全部 P0 AC、AC-NFR-001–005：按正式基线及已记录的用户豁免口径通过。
- NFR-001：静态部署目标 14.0，macOS 26.6 实机核心流程通过；macOS 14 实机由用户豁免。
- NFR-003、NFR-004、NFR-005：当前 release 独立复验通过。
- AC-100：正文、顶部控件、设置和目录在日/夜主题下均可读，切换瞬态已修复。
- NFR-011：沿用进程级断网与 `lsof` 无连接证据，用户已接受。

**发布建议：**

**GO（发布候选就绪）**。没有未关闭的 BLOCKER 或 MAJOR。签名、公证与对外发布不由本任务
执行；发布继续由用户决定。录制器 warning 与 E2E 竞态仅作为非阻塞维护项。
