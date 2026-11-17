# TASK-M6C SRS 全追踪与发布候选验收报告

**状态：** accepted

**日期：** 2026-11-18

**基线：** SRS 1.0、SDS 1.0、ADR-0002/0003；**HEAD `e01bf7d`**，外加本次经用户授权的
最小修复：`src/reader-engine/engine.ts`（夜间主题切换顺序）、重建后的
`src-tauri/resources/reader-engine.js`、`src/styles.css`（系统深色外观下阅读器外壳/抽屉
保持浅底深字）；工作区其余差异为验收脚本、测试与文档。

**参考环境：** macOS 26.6，Mac17,3，Apple Silicon arm64，16 GB；隔离 HOME 与合成 EPUB。

## 1. 发布候选产物

| 项 | 值 |
|---|---|
| 构建基线 | HEAD `e01bf7d` + 用户授权的最小主题修复；工作区其余未提交差异为验收脚本、测试与文档 |
| 路径 | `target/release/bundle/macos/Qurner.app` |
| 可执行文件 | `Contents/MacOS/qurner`，8,153,200 B（7.9 MB 整包） |
| 架构 | Mach-O 64-bit executable **arm64**（thin） |
| Bundle Identifier | `com.qurner.reader`（codesign Identifier `qurner`） |
| 版本 | 0.1.0 |
| SHA-256 | `f9c0909abdf0a18785217d8b3f6926e313f527eaafda4fcb5db8004511a515da` |
| 部署目标 | `LSMinimumSystemVersion = 14.0`；`LC_BUILD_VERSION minos 14.0`（SDK 26.5） |
| macOS 14 证据 | **用户明确豁免实机实测**（2026-11-18）：以 macOS 26.6 运行证据 + 14.0 部署目标为准；风险见第 8 节 |
| 签名/公证 | 未签名、未公证（不在本任务范围；发布前需用户处理） |

## 2. 最终门禁（本轮独立执行）

| 命令 | 退出码 | 结果 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 17 文件 220 tests（含 engine 主题测试） |
| `npm run test:e2e` | 0 | Playwright WebKit 46 passed |
| `npm run build` | 0 | 生产前端构建成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 320 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 重建成功（产物见第 1 节） |
| `git diff --check` | 0 | 通过 |

## 3. P0 FR → AC 追踪（SRS 1.0）

SRS 共 166 条 FR，其中 **P0 87 条**；P0 FR 全部被验收矩阵 AC 覆盖（脚本核对：87/87，
无遗漏）。P1/P2 79 条不阻塞 MVP，目标版本见 SRS 第 9 节。

| AC | 覆盖 FR（SRS 1.0） | 任务/证据（日期） | 自动化 | 真实 app |
|---|---|---|---|---|
| AC-001 平台/界面/正文语言 | FR-260, FR-263 | M5E-C 第三轮（2026-11-16）；M6C 重启（本次） | Vitest/Playwright | `shots5/105`、`106`（中文正文、系统字体回退） |
| AC-002 格式边界 | FR-001, FR-005, FR-006, FR-007 | M5R4（2026-11-16，content 安全扫描）；M6A（2026-11-17） | epub-core 24 项、src-tauri 导入错误测试 | `shots5/42`（脚本）、`44`（固定版式）、`45`（竖排）、`46`（加密）、`47`（损坏）、`21`（缺结构） |
| AC-010 单本/批量导入 | FR-010–FR-014 | M5E-C 第三轮；M6A | library-core import 8 项 | `shots3/05`（真实导入）、`shots5/102`（远程书导入） |
| AC-011 重复与失败汇总 | FR-016–FR-019 | M5E-C 第三轮；M6A | import 测试（判重/部分失败） | `shots4/05`（导入结果）、`shots5/21`（失败项） |
| AC-012 元数据与回退 | FR-020–FR-022 | M4/M5E-C | library-core 测试 | `shots3/05`（书名/作者）、占位封面截图 |
| AC-030 书库视图 | FR-030–FR-033, FR-035 | M5R2；M5E-C | Playwright 网格/列表 | `shots3/12`、`shots4/22` |
| AC-031 搜索与排序 | FR-037–FR-039, FR-042–FR-045 | M5E-C 第四轮；M6B | list 测试、搜索基准（p95 0.53 ms，rows=1） | `shots4/25`（搜索 0999） |
| AC-032 软移除与恢复 | FR-057, FR-058 | M5E-C 第三轮 | storage/books 测试 | `shots5/50`、`51`（软移除）、`19`（恢复导入） |
| AC-040 原始文件失效 | FR-059 | M5E-C 第三轮 | open 测试 | `shots5/49`（缺失文件错误页） |
| AC-070 阅读模式 | FR-070–FR-073 | M5E-C 第三轮 | reader 测试 | `shots3/08`（滚动模式） |
| AC-080 字号/原始样式 | FR-080–FR-084, FR-087 | M5E-C 第三轮 | reader 测试 | `shots3/08`（字号 110%）、`shots5/106`（1.1） |
| AC-090 全局/单书设置 | FR-090–FR-093 | M6C 重启恢复（本次） | settings 测试 | `shots5/106`（book_settings 1.1/night） |
| AC-100 昼夜主题 | FR-100, FR-101 | M6C 重启（本次） | reader 测试 | `m6b/shots/109-night-stable.png`（稳定深底浅字；104/106 为切换瞬态） |
| AC-110 沉浸式 | FR-110–FR-114, FR-116 | M5R3（2026-11-17） | fake-timer/E2E | `shots2/32`、`33`、`34` |
| AC-120 目录 | FR-120–FR-125 | M5E-C 第三轮 | reader 测试 | `shots3/08`、`m6b/shots/74`（嵌套目录） |
| AC-130 正文插图 | FR-130 | M5E-C 第三轮；M6C 远程冒烟 | content-safety 测试 | `shots3/29`（本地图）、`shots5/102`（本地图显示/远程阻断） |
| AC-160 进度保存/恢复 | FR-160–FR-169 | M5E-C 第四轮（kill -9 0.24 s）；M6C 重启 | progress 测试、翻转测试 | `shots3/21`（kill 恢复）、`shots5/106`（32.9% 恢复） |
| AC-170 书签 | FR-170–FR-174 | M5E-C 第三轮；M6C 重启 | bookmark 测试 | `shots3/08`（跨设置跳回）、`shots5/107`（重启后书签） |
| AC-190 文本选择/复制 | FR-190, FR-191 | M5E-C 第三轮 | Playwright 复制事件 | `shots3/07`（选择） |
| AC-200 键盘 | FR-200, FR-201 | M5E-C 第三轮；M6B 翻页 | reader-navigation E2E | `shots3/08`、`m6b/shots/85`（真实翻页） |
| AC-210 打开书籍 | FR-210 | M6C 重启（本次） | library 测试 | `shots5/106` |
| AC-220 本地/离线 | FR-220–FR-224 | M6A（进程级 sandbox 断网）；M6C lsof | 安全协议测试 | `shots5/41`（断网进程内阅读）、`shots5/102`（远程阻断） |

## 4. NFR 追踪

| NFR | 要求 | 证据（日期/任务） | 结果 |
|---|---|---|---|
| NFR-001 平台 | macOS 14+ Apple Silicon | `file`：arm64；`LSMinimumSystemVersion=14.0`；`LC_BUILD_VERSION minos=14.0`；macOS 26.6 实机完成核心流程；**macOS 14 实机由用户明确豁免（2026-11-18）** | 通过（用户豁免 14 实测） |
| NFR-002 规模 | 1,000 本/100 MB | M6B（2026-11-17）：1,000 本冷启动/搜索；M5R4 106 MB | 通过 |
| NFR-003 冷启动 | p95 ≤3 s | M6B：p95 562.4 ms（nearest-rank，=最大样本） | 通过 |
| NFR-004 搜索 | p95 ≤200 ms | M6B：p95 0.53 ms（真实 1,000 本 DB，`rows=1` 断言） | 通过 |
| NFR-005 翻页 | p95 ≤150 ms | M6B 返工（2026-11-17）：60 fps 外部录制、正文 ROI，**p95 54.19 ms** 保守值；录制器已修复窗口选择（标题 Qurner + ≥800×500 + 最大面积） | 通过 |
| NFR-006 100 MB 导入 | ≤10 s | M6A：2.16 s；M5E-C 第四轮 | 通过 |
| NFR-007 保存时机 | 停止约 1 s/翻页/退出立即 | M5E-C 第三轮；progress 测试 | 通过 |
| NFR-008 异常恢复 | 丢失 ≤2 s | M5E-C 第四轮：0.24 s | 通过 |
| NFR-009 原子写入 | 失败保留原状态 | M6A：只读 DB/批量失败测试 | 通过 |
| NFR-010 原文只读 | SHA-256 不变 | M5E-C/M6A：导入前后 SHA-256 一致 | 通过 |
| NFR-011 隐私/离线 | 无出站请求 | M6A 进程级 sandbox 断网 + lsof；M6C 远程书 lsof 无连接 | 通过（进程级） |
| NFR-012 错误反馈 | 具体原因/下一步 | M6A：14 码固定中文文案 + 错误 UI | 通过 |
| NFR-013 数据版本 | 记录带版本 | storage schema v1、locator v1（M2–M5 测试） | 通过 |
| NFR-014 升级兼容 | 不丢旧数据 | storage 迁移回滚/较新 schema 测试 | 通过 |
| NFR-015 格式可替换 | 数据语义独立 | epub-core/manifest JSON 分离（SDS/M1） | 通过 |

## 5. AC-NFR-001–005 独立验证

- **AC-NFR-001**：性能规模部分（NFR-002–006：冷启动 p95 562.4 ms、搜索 p95 0.53 ms、
  翻页 p95 54.19 ms、100 MB 导入 2.16 s）通过；平台部分（NFR-001）采用 macOS 26.6
  实机运行 + 14.0 静态部署目标，macOS 14 实机由用户明确豁免。**通过（豁免）**。
- **AC-NFR-002**：M6B/M5E-C：kill -9 丢失 0.24 s；写入失败无半成品；原文 SHA-256 不变。
- **AC-NFR-003**：M6A 进程级 `sandbox-exec` 断网（Net OFF profile）完成 106 MB 书打开
  （截图 41）；M6C 远程资源书打开后 `lsof -a -p <pid> -i` 0 连接（截图 102）。
  真实整机关网未执行（用户已接受进程级方案；另见 M6A 记录）。
- **AC-NFR-004**：M6A 14 个稳定错误码固定中文文案/无泄漏；损坏/不支持/失效/重复 UI
  截图（shots5 42/44/45/46/47/21/49）。
- **AC-NFR-005**：schema v1、manifest_version、locator_version 校验；升级/较新 schema
  拒绝测试；epub-core 与 storage 边界（M1/M2 测试）。

## 6. 最终真实 app 冒烟（本次，隔离 HOME `/tmp/qurner-acceptance/final2/home`）

1. **远程资源 EPUB**（`remote-smoke.epub`：本地 PNG + `https://example.com/remote.png`）：
   导入成功；打开后本地图片正常显示，远程图片显示 **alt 占位“远程图”**（资源被阻断、
   页面稳定、正文后续正常）；`lsof -a -p <qurner> -i` **0 连接**。
   截图 `/tmp/qurner-acceptance/m6b/shots/102-remote-reader.png`。
2. **系统深色外观下外壳/抽屉浅底浅字修复**（用户复现）：`@media (prefers-color-scheme:
   dark)` 此前只覆盖书库/导入等组件，未覆盖阅读器外壳；系统深色外观下 `body` 文字变浅，
   `.reader-shell`/`.reader-header`/`.reader-settings`/`.reader-toc`/`.reader-bookmarks`
   仍为浅色背景 → 浅底浅字。已在 dark media 中为这些容器固定 `background:#f5f3ee` /
   `#fffdf8` + `color:#26221c`（常态浅底深字）；真实 app 截图
   `m6b/shots/120-dark-settings-fixed.png`（header/设置）、`121-dark-toc-fixed.png`（目录）。
3. **夜主题瞬态修复**：`engine.ts applySettingsStyle` 改为先同步设置正文背景/文字色，
   再切换 `qurner-night` 类，消除“浅底+浅字”过渡帧；`engine-settings` 测试新增
   夜间→白天恢复断言。重建后真实 app 连续切换白天/夜间 **20 次**，切换后立即截图均
   正常（白天浅底深字、夜间深底浅字，`m6b/shots/111-theme-day-1.png`、
   `112-theme-night-1.png`）。
4. **退出/重启/同 HOME 恢复**：在长书翻 3 页（32.3%）、添加书签、设置字号 1.1/黑夜，
   正常退出后重新启动：
   - 书库恢复（`shots5/105`）；
   - 打开长书恢复到 **第 1 章 32.9%**（DB `global_progression=0.3234`），字号/黑夜保持
     （DB `book_settings.font_scale=1.1, theme=night`），截图 `106`；
   - 书签抽屉仍显示该条书签（DB `bookmarks=1`），截图 `107`。
5. **构建产物**：见第 1 节（arm64、7.9 MB、SHA-256 `f9c0909a…15da`、bundle id）。

## 7. SRS 7.3 发布门槛逐条

| 门槛 | 结果 |
|---|---|
| 1. 全部 P0 AC 通过 | **通过**（AC-001–220，第 3 节；P0 FR 87/87 覆盖） |
| 2. AC-NFR-001–005 通过 | **通过**：AC-NFR-001 采用用户豁免的 macOS 14 证据口径；AC-NFR-002–005 通过 |
| 3. 1,000 本/100 MB 夹具可重复 | **通过**（M6B 脚本 + M5R4/导入夹具） |
| 4. 无未关闭数据丢失/原文改写/静默错位 | **通过**（无 BLOCKER/MAJOR；SHA-256 不变、原子性测试） |
| 5. P1/P2 目标版本标明 | **通过**（SRS 第 9 节：V1.x/V2.x） |

## 8. BLOCKER / MAJOR 汇总

- 无未关闭 BLOCKER/MAJOR。NFR-001 的 macOS 14 实机证据由用户明确豁免（风险保留，
  见已知限制）；夜间主题切换瞬态已修复（见下）。
- 历史缺陷已关闭：M5E-C 100 MB 文本导入（M5R4）、1,000 本预置打开（M5E-C 第四轮）、
  M6A 错误 UI/存储失败（M6A）、M6B benchmark 可重复性（M6B 返工）。
- 已知限制（README 同步）：
  1. macOS 14 实机证据由用户豁免（仅 26.6 运行 + 14.0 部署目标），风险自担；
  2. 106 MB 单章首屏渲染约 32 s；
  3. 翻页测量为 60 Hz 外部录制口径（≥55 fps 验证，+16.7 ms 量化保守值）；
  4. 真实整机关网未执行（进程级 sandbox 证据）；
  5. 发布候选未签名/公证，未发布。

## 9. 发布建议

**GO（发布候选就绪）**：SRS 1.0 P0 AC 与 AC-NFR-001–005 通过（NFR-001 采用用户
明确豁免的 macOS 14 证据口径）；夜间主题切换瞬态已修复并验证（20 次切换截图正常）。
`Qurner.app` 为 arm64 发布候选（SHA-256 `f9c0909a…15da`）。签名、公证与对外发布由
用户/Codex 决定；未修改远程状态、未签名词、未发布。

## 10. 交付文件

- `docs/acceptance/TASK-M6C-REPORT.md`（本报告）
- `docs/tasks/TASK-M6C.md`（状态与报告）
- `README.md`（发布候选、隐私/数据、已知限制、构建产物信息）

除用户授权的最小主题修复（`src/reader-engine/engine.ts`、其生成资源
`src-tauri/resources/reader-engine.js` 和 `src/styles.css`）外，未修改 SRS/SDS/PLAN/ADR、
schema、依赖、CSP/capability、iframe 协议、真实 EPUB 与真实书库；未提交 commit。
