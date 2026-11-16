# TASK-M5R2：真实应用网格书库布局修复

**状态：** accepted

**里程碑：** M5 rework

**负责人：** DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-16

**依赖：** TASK-M5E-C blocked

## 1. 目标

在真实 `Qurner.app` 默认网格视图中，单本和多本书籍都显示尺寸合理的卡面、封面、书名和
作者，不被超大占位封面或固定行高裁切；列表视图保持现状。

## 2. 已知证据

- `shots/13-library-loaded.png`：单本书在默认网格中，占位封面几乎填满书库区域，标题不可见。
- `shots/17-listview.png`：同一本书列表视图正常。
- `LibraryPage` 的列数初始为 1，网格行固定为 368px；`.book-cover` 使用 100% 宽和
  2:3 比例，真实 WKWebView 中发生纵向溢出裁切。

## 3. 范围

### 必须完成

- 修复真实 app 网格列测量、单卡宽度/高度和封面约束，避免封面决定整张卡尺寸。
- 单本、2–4 本及多本网格中，封面、书名、作者均可见；宽窗口呈现合理多列。
- 列表视图、搜索、排序、软移除、打开书籍不回归。
- 增加真实窗口尺寸/单本网格的 WebKit 或 Qurner.app 视觉证据。

### 不属于本任务

- M5 阅读器握手；由 `TASK-M5R1` 处理。
- 记住上次视图（P1）、新书库外观或分页。
- 1,000 本完整性能重测；M5E-C 修复后重跑。

## 4. 需求追踪

| 类型 | 编号 | 验收要求 |
|---|---|---|
| FR | FR-030, FR-031, FR-032, FR-033, FR-035 | 默认网格和列表都完整展示封面、书名、作者且可切换 |
| AC | AC-030 | 网格/列表视图可切换，信息完整，软移除书籍不显示 |
| NFR | NFR-003, NFR-004 | 冷启动和搜索门槛不得回退 |

## 5. 文件权限

### 允许修改

- `src/styles.css`
- `src/features/library/LibraryPage.tsx`、`BookCover.tsx` 及其测试
- `tests/e2e/library.spec.ts`
- `docs/tasks/TASK-M5R2.md`

### 禁止修改

- 后端命令、数据库、schema、SRS/SDS/PLAN/ADR/其他任务
- 依赖、package/lock 文件、原始 EPUB 或真实用户数据

## 6. 测试与真实验收

- 覆盖单本、2 本、多本、宽/窄窗口和列表切换，断言卡面、封面、标题、作者都在容器内。
- 真实 Qurner.app 隔离 HOME 中导入合成 EPUB，截图默认网格和列表视图。
- 保留 1,000 本虚拟化自动化与当前性能门槛。

## 7. 必跑命令

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

## 8. 禁止事项

- 不用缩小窗口或强制列表视图掩盖默认网格缺陷。
- 不改变书籍数据或视图选择含义。
- 不提交、推送或发布。

## 9. DeepSeek 开发报告

**完成摘要：**

修复真实 `Qurner.app` 默认网格书库布局。

- **根因**：网格列测量在数据加载前执行（`scrollRef` 为 null 时直接返回），列数长期停留在 1，
  卡片被容器宽度拉伸；行高固定 368/352px 与封面 `width:100% + aspect-ratio:2/3` 冲突，
  宽卡面时封面纵向溢出、标题/作者被裁。滚动区 `max-height:68vh` 在较矮窗口超出可视区，
  网格无法滚动到卡片底部。
- **修复**：
  - 列测量：初始用窗口宽度计算，数据加载后（`showToolbar`）重测并监听 resize/ResizeObserver；
    `columns = floor((width + gap) / (minWidth + gap))`，卡宽按列宽计算。
  - 卡面/行高：行高由卡宽推导（封面 2:3 + 卡面 chrome + 标题/作者/移除按钮安全高度），
    并将行高限制在滚动区可视高度内；虚拟行 `height` 与 `ul` 总高使用同一行高，虚拟化
    不再依赖过期 estimate。
  - 封面：新增 `BookCover.coverWidth`，网格内按可用高度换算封面宽度并保持 2:3；
    标题/作者网格单行省略（clamp 1），移除按钮完整可见。
  - 滚动区：`max-height: min(68vh, calc(100vh - 30rem))`，较矮窗口下网格在可视区内滚动；
    同时收紧页面顶部间距（padding 2rem / gap 1.5rem）为书库让出空间。
- **测试**：E2E 新增“默认网格在单本、四本与宽窄窗口下完整显示封面/书名/作者”：
  断言单本/4 本/1600px/360px 下封面、书名、作者、移除可见；卡片内容不溢出
  （`scrollHeight - clientHeight ≤ 4`）；封面宽高比 2:3；宽窗口多列、窄窗口单列；
  切换窗口后重新布局。1,000 本虚拟化与性能用例保留（44→45 项 E2E）。
- **真实应用**：隔离 HOME + 合成 EPUB（`cn-nested.epub`，数据库仅合成书），重新
  `npm run tauri build` 后默认网格显示书名“中文嵌套图片”、作者“合成作者”、移除按钮
  与完整 2:3 小封面（`/tmp/qurner-acceptance/shots/51-m5r2-grid-final2.png`）；列表视图
  正常（`52-m5r2-list.png`）。修复前对照：`40`–`50` 系列截图（封面占满、标题被裁）。

**修改文件：**

- `src/features/library/LibraryPage.tsx`（列测量、动态行高、封面宽度）
- `src/features/library/BookCover.tsx`（coverWidth）
- `src/styles.css`（网格行/卡片/封面/滚动区/页面间距）
- `tests/e2e/library.spec.ts`（新增网格布局回归）
- `docs/tasks/TASK-M5R2.md`

未修改后端命令、数据库、schema、依赖锁文件、SRS/SDS/PLAN/ADR/其他任务；未改真实书库；
未提交 commit。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 16 文件 167 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 45 passed |
| `npm run build` | 0 | 生产前端构建成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 300 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 重建成功（7.92 MiB） |
| `git diff --check` | 0 | 通过 |

**真实应用验收：**

- 环境：macOS 26.6 arm64；`HOME=/tmp/qurner-acceptance/home` 隔离；合成 EPUB
  `cn-nested.epub`（数据库仅 1 本合成书）。
- 默认网格：书名/作者/移除完整可见，封面按 2:3 完整显示（小尺寸以适配较矮窗口）；
  列表视图：封面/书名/作者/移除完整（回归通过）。
- 证据：`shots/51-m5r2-grid-final2.png`、`shots/52-m5r2-list.png`；修复前
  `shots/40`、`45`、`46` 等。

**未验证项：**

- 真实 app 中 **2–4 本/多本网格截图**未完成：文件对话框批量导入的 CGEvent 自动化在
  当前窗口/焦点竞争下未成功（e2e 已覆盖 4 本与 1600/360px 宽窄窗口的完整性与列数，
  但不能替代真实 app 证据）。
- 真实 app 最大化宽窗口截图未获得（自动化双击标题栏被前台 Harness 窗口抢占；
  e2e 1600px 视口覆盖多列断言）。
- 真实 app 中搜索/排序/软移除/打开书籍未逐项截图回归（既有 Vitest/E2E 覆盖，M5R1
  已验证真实 app 打开书籍；本任务仅验证书库布局）。
- 封面在默认窗口下较小（受可视高度约束）；是否满足视觉期望需 Codex 确认。

**风险与待决问题：**

- 网格行高随滚动区可视高度/列宽变化，每行等高；窗口过矮时封面会缩得较小以换取
  书名/作者完整。
- 标题/作者网格单行省略；更长标题需悬浮/详情查看（P1，不在本任务）。
- 真实 app 多本截图依赖后续人工/自动化环境；如需必须在验收时补拍。

**依赖与禁止路径：**

- 未修改后端、数据库、schema、CSP/capability、依赖锁文件、SRS/SDS/PLAN/ADR/其他任务、
  真实书库/原文。
- 未启动 M5E-C 重跑/M6；未提交/推送。

## 10. Codex 审查

**结论：** 通过

**审查日期：** 2026-11-16

**独立执行的检查：**

- `npm run check`：通过，14 个文件、167 项测试。
- `npm run test:e2e`：通过，WebKit 45 项；新增单本/四本、1600/360px 宽窄窗口网格
  完整性、列数与封面比例回归。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，300 passed、0 failed、1 ignored。
- `npm run build`、`npm run tauri build`：通过，`Qurner.app` 7.92 MiB。
- 独立查看 `51-m5r2-grid-final2.png` 和 `52-m5r2-list.png`：真实 app 默认网格已显示完整
  封面、书名、作者和移除按钮；列表视图正常。
- `git diff --check`：通过；无新增仓库根目录临时文件。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MINOR | 真实 app 较矮窗口 / 网格封面 | 为同时保留书名、作者和移除按钮，封面会缩小以适配可视高度。 | 当前尺寸完整且可操作，不阻塞 AC-030；后续视觉优化另行评估，不扩大本任务。 |

**AC 结果：**

- FR-030/031/032/033/035、AC-030：通过。
- 默认网格与列表切换、封面、书名、作者、软移除入口均保持可用。
- 1,000 本虚拟化和性能用例保持通过。

**后续：**

- M5R2 accepted。
- M5E-C 的阅读器与网格阻塞均已解除，等待用户启动 M5E-C 全量复验。
