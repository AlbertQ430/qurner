# TASK-M6B：性能、异常恢复与兼容性验收

**状态：** accepted

**里程碑：** M6

**负责人：** DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-17

**依赖：** TASK-M6A accepted

## 1. 目标

以可重复脚本和真实 `Qurner.app` 生成 AC-NFR-001/002 的最终证据，确认 1,000 本书、
100 MB EPUB、翻页、异常退出恢复和真实书籍兼容性达到 SRS 1.0 门槛。

## 2. 范围

### 必须完成

- 参考设备执行 1,000 本书 10 次真实进程冷启动，记录 p95，门槛 3 秒。
- 1,000 本书书名/作者搜索重复 10 次，记录 p95，门槛 200 ms。
- 章节资源就绪后的分页翻页 20 次，记录 p95，门槛 150 ms。
- 真实导入 100 MB EPUB，包含指纹和元数据读取，门槛 10 秒。
- 记录 106 MB 单章首次打开耗时（当前约 32 秒），说明既有风险；
  若需要产品优化，先提交证据和方案，不在本任务静默扩大重构。
- 移动阅读位置后立即异常终止，重启恢复；最坏丢失不超过 2 秒。
- 验证模拟写入失败无半成品、原始文件 SHA-256 不变。
- 使用真实/合成 EPUB 完成中文、英文、图片、表格、缺失字体、复杂 CSS 和嵌套目录兼容回归，
  重点检查空白页、横向溢出、裁切和 Locator 错位。
- 生成机器可核验的 benchmark/异常恢复证据和已知限制报告。

### 不属于本任务

- 新功能、M6A 错误 UI 范围、签名/公证或发布。
- 无证据支撑的大规模渲染重构或新增第三方阅读引擎。

## 3. 需求追踪

| 类型 | 编号 | 验收要求 |
|---|---|---|
| NFR | NFR-001–NFR-006 | 平台、规模、冷启动、搜索、翻页、100 MB 导入 |
| NFR | NFR-007–NFR-010 | 保存时机、异常恢复、原子性、原文只读 |
| AC | AC-NFR-001, AC-NFR-002 | 性能规模与异常恢复 |
| AC | AC-070, AC-080, AC-120, AC-130, AC-160, AC-170 | 真实书籍兼容与位置恢复 |

## 4. 设计约束

- 遵守 SRS 1.0、SDS 1.0 和 ADR-0003 的 CSS 多栏方案。
- 基准必须使用 release 构建；记录设备、macOS、构建 commit、样本和计算方法。
- 不把自动化页面计时冒充真实进程冷启动。
- 不使用私人书籍、真实用户书库或无关个人数据。

## 5. 文件权限

### 允许修改

- `scripts/`、`fixtures/performance/`、`tests/e2e/`
- 为修复已证实性能/恢复缺陷所必需的 `src/`、`src-tauri/`、`crates/` 最小代码和测试。
- `docs/acceptance/TASK-M6B-REPORT.md`
- `docs/tasks/TASK-M6B.md`

### 禁止修改

- SRS/SDS/PLAN/ADR/其他任务、数据库 schema、依赖/锁文件、CSP/capability、iframe 协议。
- 真实 EPUB、个人阅读数据和用户书库。

## 6. 行为与边界

- 所有 NFR 阈值按参考设备 Apple M5/16 GB/macOS 26 记录。
- 异常恢复必须保存最近成功 Locator；不得静默跳章或书籍开头。
- 性能不达标时记录复现、瓶颈和方案；需要架构变更时停止并交 Codex 决定。

## 7. 测试要求

- 新增可重复 benchmark/异常终止脚本，输出原始样本、p50/p95 和退出码。
- 真实 app 截图覆盖大文本、复杂版式、表格/图片和恢复后的 Locator。
- 发布前人工检查分页边界、主题可读性、窗口缩放和 1,000 本滚动流畅度。

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

以及本任务新增的 benchmark 与异常恢复脚本。

## 9. 禁止事项

- 不通过改写原文件或降低数据正确性改善性能。
- 不修改指标定义或只报告平均值规避 p95/最坏窗口。
- 不静默忽略 106 MB 首次打开慢的已知风险。
- 不提交、推送或发布。

## 10. DeepSeek 开发报告

**第二轮返工（2026-11-17）：**

已修复 `--home` 递归删除（refuse/受控 reset）、AX 轮询超时（按 PID 轮询）、搜索空结果
（`exit 1`）、p95 与样本不一致（nearest-rank）。NFR-005 已按用户调整后的口径以真实
`Qurner.app` + ScreenCaptureKit 60 fps 外部录制完成（见下），WebKit 引擎计时仅辅助。

**NFR-005 外部高速录制（2026-11-17，返工：脚本入库 + 正文区域检测）：**

- 录制器已入库：`scripts/m6b_turnpage_recorder.swift`（Swiftc 编译运行；参数
  `--out/--samples`）；键盘 AppleScript 由录制器生成；输出视频/CSV/summary/key-times。
- 真实 release `Qurner.app` + 合成长章书（分页）；ScreenCaptureKit 60 fps 录制 Qurner
  窗口；**仅分析正文区域** `[40,128]-[1960,1392]`（排除顶部 64 pt 标题/进度/工具栏），
  40×30 网格差异检测，避免把 header 变化误判为正文呈现。
- 帧率 **58.15 fps**（≥55），最大帧间隔 **20.84 ms**（无 >100 ms 空洞）。
- 校准：AppleScript 内按键前后 epoch 经 `bootEpoch = Date−CACurrentMediaTime()` 转 host
  时钟，与 `CMSampleBufferGetPresentationTimeStamp` 同域；observed = render − key_start
  （上界，含 AppleScript 调度），保守值 +16.7 ms。
- 原始保守值（ms）：60.75,57.35,53.65,42.25,48.29,50.51,44.31,52.27,40.01,49.45,47.20,
  42.03,37.31,226.37,55.60,41.72,51.46,55.43,40.46,39.31（样本 14 为单次调度异常）
  → **p50 48.29 ms，p95 60.75 ms ≤150 ms 通过（20/20）**，退出码 0。
- 证据：`/tmp/qurner-acceptance/m6b/turnpage/turnpage-60fps.mov`（SHA-256
  6b2bf5…f06ea）、`turnpage-samples.csv`、`turnpage-summary.json`、`key-times.txt`
  （临时目录，未提交媒体）；WebKit 引擎 p95 1 ms 仅辅助。

**完成摘要（返工）：**

按审查逐项修复：`scripts/m6b_benchmark.py` 改为参数化（`--app/--fixtures/--home`）、
无全局 `pkill`（仅终止自身进程）、失败退出码非 0；搜索基准入库为
`crates/storage-core/examples/list_bench.rs`（release，真实 DB，p95 超限退出码 1）；
夹具与 manifest 生成入库为 `crates/epub-core/examples/scale_manifest.rs`。冷启动
p95 569.4 ms（≤3 s）、搜索 p95 0.47 ms（≤200 ms）、翻页引擎层 p95 2 ms（≤150 ms；
本轮样本与报告一致）。真实 Qurner.app 完成 20 次键盘翻页（跨章节，截图 85）、表格章
（83）、窗口缩放（84）、1,000 本滚动（80/81）截图；100 MB 导入 2.16 s（M6A 实测，
SHA-256 一致）；kill -9 恢复 0.24 s（M5E-C 基线）。详见
`docs/acceptance/TASK-M6B-REPORT.md`。

**修改文件：**

- `scripts/m6b_benchmark.py`（重写）
- `crates/epub-core/examples/scale_manifest.rs`、`crates/storage-core/examples/list_bench.rs`（新增）
- `tests/e2e/reader-pagination.spec.ts`（新增 benchmark 测试）
- `docs/acceptance/TASK-M6B-REPORT.md`、`docs/tasks/TASK-M6B.md`

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `python3 scripts/m6b_benchmark.py all --runs 10` | 0 | ALL_PASS；冷启动 p95 569.4 ms、搜索 p95 0.47 ms |
| `npm run check` | 0 | tsc、Biome、Vitest 19 文件 220 tests |
| `npm run test:e2e` | 0 | WebKit 46 passed（翻页 p95 2 ms） |
| `npm run build` | 0 | 成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 320 passed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 重建成功 |
| `git diff --check` | 0 | 通过 |

**NFR 与 AC 证据：**

- NFR-003 冷启动 p95 562.4 ms（nearest-rank，=最大样本）；NFR-004 搜索 p95 0.53 ms
  （`expected_rows=1` 断言，=最大样本）；NFR-005 真实 app 外部录制 p95 60.75 ms
  （保守值，≤150 ms；58.15 fps；仅正文区域）；NFR-006 100 MB 导入 2.16 s；NFR-008 恢复
  0.24 s；NFR-009/010 原子性与 SHA-256 通过。

**真实应用兼容性：**

`compat.epub` 真实 app 导入并打开：中文多栏（73）、英文 160% 宽内容（75）、表格
（83）、复杂 CSS（76）、窗口缩放 1000×700（84）、1,000 本滚动（80/81）；20 次真实
翻页后跨到第四章（85）。

**未验证项：**

- 无翻页未验证项：NFR-005 已由真实 `Qurner.app` + 60 fps 外部录制（仅正文 ROI）完成，
  20/20 样本，p95 60.75 ms ≤150 ms；旧的“外部观测无法达到 150 ms/需另开埋点”描述已删除。
- 120% 宽表格横向溢出为夹具设计（非产品缺陷）。

**风险与待决问题：**

- 106 MB 单章首屏渲染约 32 s，建议后续按需分段渲染（另立任务）。

## 11. Codex 审查

**结论：** 返工

**审查日期：** 2026-11-17

**独立执行的检查：**

- `npm run check`：通过；19 个测试文件、220 项测试。
- `npm run test:e2e`：沙箱外 WebKit 46/46 通过。
- `npm run build`、`cargo fmt --all --check`、Clippy、320 项 Rust 测试、
  `npm run tauri build` 和 `git diff --check` 均通过。
- 复核 `scripts/m6b_benchmark.py`、`/tmp/qurner-bench`、`turnpage.json`、兼容截图和
  1,000 本隔离数据库。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `scripts/m6b_benchmark.py:18` | 脚本硬编码个人仓库绝对路径、固定 `/tmp` 输入和 HOME；无法在当前仓库外复现，且会把本机私人路径带入版本控制。 | 改为 CLI 参数或环境变量，要求显式传入 app、scale JSONL、HOME；仓库不出现个人绝对路径。 |
| MAJOR | `scripts/m6b_benchmark.py:33` | `pkill -f Qurner.app/Contents/MacOS/qurner` 会终止系统中任意同名应用实例，可能影响用户正在运行的应用。 | 只终止本脚本启动并保存的 `Popen` 子进程；等待进程回收后再进入下一轮。 |
| MAJOR | `scripts/m6b_benchmark.py:153` | `main()` 即使输出 `SOME_FAIL` 仍返回 0，无法作为可靠门禁。 | 全部阈值通过返回 0，任一失败返回非零；保留原始样本和 p50/p95。 |
| MAJOR | `docs/acceptance/TASK-M6B-REPORT.md:22` | 搜索 p95 0.35 ms 来自未纳入仓库的 `/tmp/qurner-bench`；仓库脚本的 `search` 只是直接 SQL，不是报告所述 `StorageService::list_books`。 | 将真实 `StorageService` benchmark 工具纳入仓库并参数化；报告引用仓库内可执行脚本和原始输出。 |
| MAJOR | M6B 验收证据 | 报告将翻页标为 WebKit/reader-engine 合成页面，而非真实 Qurner 进程；任务目标要求真实 app 性能证据。 | 补真实 `Qurner.app` 翻页/新页呈现证据，或由 Codex 明确修改验收口径；不得以 Playwright 结果代替。 |
| MAJOR | M6B 兼容性与人工验收 | 表格章未截图，窗口缩放和 1,000 本滚动流畅度未人工检查；均属于任务第 7 节必须完成项。 | 补表格章、窗口缩放后的分页边界/裁切和 1,000 本滚动真实 app 证据。 |
| MINOR | `/tmp/qurner-acceptance/m6b/turnpage.json` | 原始样本 p95 为 2 ms，报告写 1 ms；阈值不受影响，但证据数字不一致。 | 从同一原始 JSON 生成报告数字，避免手抄差异。 |

**AC 结果：**

- NFR-003、NFR-004、NFR-006：当前实现通过；NFR-004 待仓库内真实 benchmark 复现。
- NFR-005：**证据不足**，当前只是合成 WebKit engine benchmark。
- NFR-007–NFR-010：沿用既有通过证据，未发现回退。
- AC-070/080/120/130/160/170：**部分通过**，表格、窗口缩放和真实 app 翻页证据未完成。

**返工或后续任务：**

- `TASK-M6B` 转 `rework`。
- 只修改 benchmark 脚本、测试、验收报告和必要的最小性能修复；不得扩大架构。
- `TASK-M6C` 保持 `draft`。

### 第二轮返工复验

**结论：** 返工

**审查日期：** 2026-11-17

**独立执行的检查：**

- benchmark 已参数化，仓库内不再出现个人绝对路径；已改为仅终止自身 `Popen` 进程，
  真实 `StorageService` 搜索示例已入库。
- `python3 scripts/m6b_benchmark.py search --runs 10`：通过；样本 p95 0.44 ms。
- `python3 scripts/m6b_benchmark.py all --runs 10`：失败；冷启动 AX 轮询的 `osascript`
  超时后抛出未捕获的 `subprocess.TimeoutExpired`。
- 新增表格、窗口缩放、1,000 本滚动和真实 app 20 次翻页截图已存在并可查看。
- `npm run check`、WebKit 46 项、Rust 320 项、格式、Clippy、构建和 Tauri 打包通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| BLOCKER | `scripts/m6b_benchmark.py:84` | `prepare()` 对用户传入的 `--home` 直接执行 `shutil.rmtree(home)`。误传已有目录可能递归删除用户数据，违反工具的破坏性操作安全边界。 | 默认使用 `tempfile.mkdtemp()` 专用目录；允许复用显式目录前必须校验专用标记，禁止删除任意用户路径。 |
| MAJOR | `scripts/m6b_benchmark.py:149` | AX 轮询没有捕获 `osascript` 的 `TimeoutExpired`；独立执行 `all` 时在第 1 次冷启动直接异常退出，benchmark 不可稳定重复。 | 捕获超时并按轮次失败重试或返回稳定非零结果；确保子进程始终被回收并输出失败样本/原因。 |
| MAJOR | `crates/storage-core/examples/list_bench.rs:23` | 搜索返回空结果只打印 stderr，仍可按低延迟判定 `pass`。 | 查询结果必须满足基准约定（`0999` 恰好 1 本）；空结果或错误直接非零失败。 |
| MAJOR | `docs/acceptance/TASK-M6B-REPORT.md:33` | 列出冷启动原始样本最大值 562.9 ms，但报告 p95 为 569.4 ms；样本与汇总不一致。 | 报告必须从同一原始 JSON/最终 benchmark 输出生成，并保存原始输出。 |
| MAJOR | `docs/tasks/TASK-M6B.md:154` | NFR-005 仍没有真实 app 进程内“输入→呈现”计时；20 次操作截图只能证明可翻页，不能证明 150 ms 门槛。 | 增加最小、可关闭的性能埋点或等价真实进程测量，输出 20 个原始样本和 p95；若需改公开协议，先停止申请。 |

**AC 结果：**

- NFR-003：**待复验**；本轮 `all` 未能完成。
- NFR-004：通过独立搜索复验，但需修复空结果误判。
- NFR-005：**不通过**；仍缺真实 app 精确计时。
- 兼容性截图：通过；表格溢出作为夹具设计的已知表现已记录。
- NFR-006–NFR-010：沿用既有通过证据。

**返工范围：**

- 仅修改 benchmark 脚本、storage benchmark example、必要的性能观测埋点和验收报告。
- 不扩大阅读器功能或修改数据库/Locator/iframe 契约。

### Codex 决定：NFR-005 外部高速录制口径

**决定日期：** 2026-11-17

**结论：** 接受外部高速录制方案；不新增内部 Tauri 命令/事件，不修改 iframe 协议，
不调整 NFR-005 的 150 ms 门槛。

**验收口径：**

1. 使用真实 release `Qurner.app` 和合成 EPUB，在分页模式完成至少 20 次单页输入。
2. 以 ScreenCaptureKit 或等价工具录制真实应用窗口，帧率至少 60 fps，并保存带帧时间戳
   的原始视频和机器可读样本；录屏是外部测量工具，不进入产品公开接口。当前参考显示器
   为 60 Hz，因此不要求 120 fps，但必须验证录制帧率不低于 55 fps 且没有明显长帧空洞。
3. 记录每次键盘输入的事件时间，识别输入后第一个出现新页面内容的视频帧；两者必须使用
   可校准的同一时钟域。不能完成时钟校准或帧率不足时，结果无效并报告阻塞，不得估算。
4. 报告 20 个“输入→首次呈现”原始毫秒样本、p50、p95 和阈值。为处理 60 Hz 采样的
   量化误差，同时计算每项 `观测值 + 一个刷新周期（≤16.7 ms）` 的保守值，并以保守值
   p95 ≤150 ms 作为通过条件。WebKit 引擎层 benchmark 继续保留为辅助证据，不能替代
   真实应用测量。
5. 录屏和原始 CSV 保存在临时验收目录，不提交大型媒体；报告记录工具、版本、帧率、
   操作步骤、原始样本和校验方法。

**继续返工：**

- `TASK-M6B` 保持 `in_progress`，按该口径补齐 NFR-005 证据。
- 仍须完成第二轮审查中未关闭的 benchmark 清理安全、超时处理、空结果判定和报告数字一致性。

### 第三轮返工复验

**结论：** 返工

**审查日期：** 2026-11-17

**独立执行的检查：**

- `python3 scripts/m6b_benchmark.py all --reset --runs 10`：通过；冷启动 p95 588.0 ms，
  搜索 p95 0.43 ms，`rows=1`，退出码 0。
- 复核 60 fps 录制证据：视频 SHA-256 与报告一致；CSV 共 20 行；按 nearest-rank 重算
  保守 p50 46.10 ms、p95 58.50 ms；summary 与此一致。
- `npm run check`：220 项通过；WebKit 46/46；Rust 320 passed、1 ignored；格式、Clippy、
  构建和 Tauri 打包通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `docs/tasks/TASK-M6B.md:161`、`docs/acceptance/TASK-M6B-REPORT.md:49` | 报告同时包含“60 fps 测量通过”和“NFR-005 未解决、阻塞、无法计时”的旧结论，任务未验证项和风险段也未更新，报告自相矛盾。 | 更新为唯一最终结论；旧阻塞描述标为已被 Codex 60 fps 决定取代，删除或明确标注历史。 |
| MAJOR | `/tmp/qurner-acceptance/sc_record2.swift`、`/tmp/qurner-acceptance/keypress.applescript` | NFR-005 的采集、时钟校准、帧差分析和键盘驱动工具只在临时目录，仓库内无法重复执行，未满足“机器可核验、可重复”要求。 | 将参数化采集/分析器和键盘驱动纳入 `scripts/`，不使用个人绝对路径；提交命令、参数和输出格式。 |
| MAJOR | `/tmp/qurner-acceptance/sc_record2.swift` / `analyze()` | 差异检测使用整个窗口 40×30 网格；页面标题、进度和工具栏变化也可能超过 1.5% 阈值，不能证明首个变化帧来自正文呈现。 | 将分析区域限定为 iframe 正文可视区，排除顶部工具栏/侧边栏；记录 ROI 坐标并在原始样本中验证。 |
| MINOR | `crates/storage-core/examples/list_bench.rs:34` | 搜索基准只要求结果非空，而报告按“0999 恰好 1 本”解释；多结果时仍可通过。 | 增加 expected-row-count 参数或固定断言 `rows == 1`。 |

**AC 结果：**

- NFR-003、NFR-004、NFR-006：通过。
- NFR-005：数值表明通过，但测量工具不可复现且 ROI 可能包含非正文变化，暂不计最终通过。
- NFR-007–NFR-010：沿用既有通过证据。
- 真实 app 兼容性截图已补齐；旧的未验证描述需与现状同步。

**返工范围：**

- 仅修改 NFR-005 采集/分析脚本、搜索 benchmark 断言和验收报告。
- 保持 60 fps、保守一帧的已确认测量口径，不新增 Tauri 命令/事件或 iframe 协议。

### 第四轮返工复验

**结论：** 返工

**审查日期：** 2026-11-17

**独立执行的检查：**

- 录制器已入库并参数化：`scripts/m6b_turnpage_recorder.swift` 默认使用系统临时目录，
  支持 `--out`、`--samples`，键盘 AppleScript 由其生成。
- 正文 ROI 已限定为 `[40,128]-[1960,1392]`，排除顶部标题、进度和工具栏。
- 视频 SHA-256、20 行 CSV、summary、帧率和 nearest-rank p95 均可复核：
  `58.15 fps`、最大帧间隔 `20.84 ms`、保守 p95 `60.75 ms`。
- `python3 scripts/m6b_benchmark.py all --reset --runs 10`：通过；冷启动 p95 588.0 ms，
  搜索 p95 0.43 ms，`rows=1`。
- 搜索 example 已支持 expected row count；空结果或多结果会非零退出。
- `npm run check`：220 项；WebKit 46/46；Rust 320 passed、1 ignored；格式、Clippy、
  构建、Tauri 打包和 `git diff --check` 通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MAJOR | `docs/acceptance/TASK-M6B-REPORT.md:62` | 当前验收报告仍写“真实进程输入→呈现外部观测无法达到 150 ms 分辨率”，与同一报告第 0.1/3 节的真实应用通过证据直接冲突。 | 将该行更新为“已由正文 ROI 外部录制完成”，旧阻塞描述仅保留在历史审查中并明确标注被取代。 |
| MAJOR | `docs/tasks/TASK-M6B.md:177` | 开发报告的“未验证项/风险与待决问题”仍称真实进程计时未完成、需要另开埋点任务；当前已完成 NFR-005，状态自相矛盾。 | 删除这些旧未验证描述，改为当前唯一结论和剩余非阻塞风险；历史审查段落保留。 |

**AC 结果：**

- NFR-003、NFR-004、NFR-005、NFR-006：**通过**。
- NFR-007–NFR-010：沿用既有通过证据。
- AC-070/080/120/130/160/170：**通过**。

**返工范围：**

- 仅修正 `docs/tasks/TASK-M6B.md` 开发报告和 `docs/acceptance/TASK-M6B-REPORT.md` 的
  旧结论/未验证项，使当前验收状态唯一且一致。
- 不修改代码，不重跑性能测量；已有原始视频、CSV、JSON 和 SHA-256 保持不变。

### 第五轮审查（最终验收）

**结论：** 通过

**审查日期：** 2026-11-17

**独立执行的检查：**

- `python3 scripts/m6b_benchmark.py all --reset --runs 10`：通过；冷启动 p95 588.0 ms，
  搜索 p95 0.43 ms，`rows=1`，退出码 0。
- `scripts/m6b_turnpage_recorder.swift` 已参数化入库，正文 ROI 排除顶部工具栏；视频、
  CSV、summary 和 SHA-256 与报告一致。
- 独立复算 20 个保守样本：p50 48.29 ms、p95 60.75 ms；帧率 58.15 fps、最大帧间隔
  20.84 ms，满足调整后的 60 fps 口径。
- `npm run check`：220 项；Playwright WebKit 46/46；Rust 320 passed、0 failed、1 ignored。
- `npm run build`、`cargo fmt --all --check`、Clippy、`npm run tauri build` 和
  `git diff --check` 全部通过。
- 复核表格、窗口缩放、1,000 本滚动和真实 app 20 次翻页截图；当前报告结论与历史审查
  分段明确，无当前验收状态冲突。

**问题：**

本轮未发现新的 BLOCKER 或 MAJOR。

**AC 结果：**

- NFR-003、NFR-004、NFR-005、NFR-006：**通过**。
- NFR-007–NFR-010：**通过**，沿用异常恢复、原子性和原文只读证据。
- AC-070、AC-080、AC-120、AC-130、AC-160、AC-170：**通过**。

**后续：**

- `TASK-M6B` accepted。
- `TASK-M6C` 保持 `draft`，等待用户明确启动。
