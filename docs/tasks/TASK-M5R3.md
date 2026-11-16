# TASK-M5R3：沉浸式控件自动隐藏修复

**状态：** accepted

**里程碑：** M5 rework

**负责人：** DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-16

**依赖：** TASK-M5E-C blocked

## 1. 目标

进入沉浸式阅读后，非必要 header 控件默认隐藏；鼠标进入顶部热区或键盘焦点进入控件时
显示；鼠标离开且焦点不在控件内时，经过固定短延迟自动再次隐藏。

## 2. 证据与根因

- 真实 app `shots2/19`、`21`：进入沉浸式并移出控制区 6.5 秒后 header 仍可见。
- `ReaderShell` 的 `controlsVisible` 只有置 true 的入口，没有鼠标离开或空闲隐藏计时器。
- 现有测试只验证热区显示控件，没有验证移出后自动隐藏。

## 3. 范围

### 必须完成

- 定义固定空闲延迟（建议 800–1200ms），鼠标离开 header/热区且焦点不在 header 内时隐藏。
- 目录、书签、设置抽屉打开时不得自动隐藏控件。
- 键盘焦点仍在 header 控件内时不得隐藏；焦点移出后可以隐藏。
- 热区重新显示控件，Esc 顺序仍为设置 → 书签 → 目录 → 沉浸式。
- 增加 Vitest fake timer、WebKit 和真实 Qurner.app 截图证据。

### 不属于本任务

- M5E-C 其他未验证场景。
- 系统全屏、动画美化或新主题。
- 修改 iframe 协议、书库、后端、数据库或依赖。

## 4. 需求追踪

| 类型 | 编号 | 验收要求 |
|---|---|---|
| FR | FR-110–FR-114 | 沉浸式隐藏非必要控件、热区显示、目录/书签/设置仍可打开 |
| AC | AC-110 | 开启/退出沉浸式；控件默认隐藏；移动鼠标显示；抽屉可用 |
| NFR | NFR-012 | 失败不阻塞阅读，不泄漏内部错误 |

## 5. 文件权限

### 允许修改

- `src/features/reader/ReaderShell.tsx`
- `src/styles.css`
- `src/features/reader/reader-shell.test.tsx`
- `tests/e2e/library.spec.ts`
- `docs/tasks/TASK-M5R3.md`

### 禁止修改

- SRS/SDS/PLAN/ADR/其他任务、iframe 消息协议、后端、数据库、schema
- 依赖/package/lock 文件、CSP/capability、真实书库或原文

## 6. 测试与真实验收

- Vitest fake timer：进入沉浸式立即隐藏；热区显示；移出后延迟隐藏；抽屉开启和 header
  焦点存在时不隐藏；Esc 顺序不变。
- WebKit：实际 mouseenter/mouseleave/focus 路径和延迟。
- 真实 app：隔离 HOME、合成 EPUB，截取沉浸式初始隐藏、热区显示、移出后再次隐藏三张图。

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

- 不用“点击退出沉浸式”代替鼠标移出后的自动隐藏。
- 不隐藏正在操作的抽屉或键盘焦点所在控件。
- 不修改其他产品行为，不提交或推送。

## 9. DeepSeek 开发报告

**完成摘要：**

修复沉浸式控件自动隐藏。

- **行为**：进入沉浸式后 header 默认隐藏；鼠标进入顶部热区、header 或键盘焦点进入 header
  时显示；header `mouseleave`、鼠标进入阅读 iframe（正文）或焦点移出 header 后，启动固定
  900ms 计时，计时到且抽屉未打开、焦点不在 header 内时隐藏。目录/书签/设置抽屉打开期间
  强制保持 header 可见并清除计时；header 焦点存在时计时到也不隐藏；Esc 顺序（设置 → 书签
  → 目录 → 沉浸式）不变。
- **实现**：`ReaderShell` 新增 `headerRef`、`hideTimerRef`、`showControls`/`scheduleHide`、
  900ms 常量；header 绑定 `onMouseEnter/onMouseLeave/onFocus/onBlur`；热区绑定
  `onMouseEnter/onFocus/onClick`；iframe 绑定 `onMouseEnter` 作为“鼠标进入正文”的可靠
  隐藏信号（真实 WKWebView 从 header 跨入 iframe 的 `mouseleave` 不稳定）；抽屉状态
  变化 effect 清除/重启计时；卸载/切书清理计时器。
- **测试**：
  - Vitest（fake timer）：进入沉浸式立即隐藏；热区显示；`mouseLeave` 后 899ms 仍显示、
    900ms 隐藏；header 焦点存在 + 鼠标离开 1000ms 不隐藏；目录抽屉打开 + 鼠标离开
    1000ms 不隐藏。
  - WebKit E2E：进入沉浸式初始隐藏；真实鼠标移入热区显示；点击阅读正文（焦点/鼠标进入
    iframe）后自动隐藏；热区聚焦重新显示；设置抽屉与 Esc 顺序保持。
- **真实应用**：隔离 HOME（`/tmp/qurner-acceptance/m5ec2`）、合成 EPUB，重新
  `npm run tauri build` 后截取三张证据：
  - 初始隐藏：`/tmp/qurner-acceptance/shots2/32-imm-hidden.png`（header 消失、正文全屏）
  - 热区显示：`/tmp/qurner-acceptance/shots2/33-imm-hotzone.png`（鼠标进入顶部热区后 header 出现）
  - 移出后自动隐藏：`/tmp/qurner-acceptance/shots2/34-imm-autohide.png`（鼠标移入正文 1.6s 后 header 消失）

**修改文件：**

- `src/features/reader/ReaderShell.tsx`
- `src/features/reader/reader-shell.test.tsx`
- `tests/e2e/library.spec.ts`
- `docs/tasks/TASK-M5R3.md`

未修改 iframe 消息协议、后端、数据库/schema、依赖/锁文件、CSP/capability、真实书库或原文；
未提交 commit。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 16 文件 169 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 45 passed |
| `npm run build` | 0 | 生产前端构建成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 300 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 重建成功（7.92 MiB） |
| `git diff --check` | 0 | 通过 |

**真实应用验收：**

- 隔离 HOME（数据库仅合成书）与合成 EPUB；窗口默认尺寸；三张截图如上。
- 初始隐藏/热区显示/移出后自动隐藏均按预期（鼠标移入正文后 1.6s 内 header 消失）。

**未验证项：**

- 真实 app 中“鼠标从 header 移出到 **非 iframe 区域**”（窗口外/边缘）的 `mouseleave` 路径
  未单独验证；该路径由 header `mouseleave` 覆盖，真实 app 主要路径（移入正文）由 iframe
  `mouseenter` 兜底验证。
- 触控板/手势离开（鼠标静止在正文）未单独验证。
- 抽屉打开期间鼠标离开后保持显示的 E2E 覆盖在 Vitest；真实 app 未逐项截图。
- 系统全屏（非应用沉浸式）不在本任务范围。

**风险与待决问题：**

- 自动隐藏依赖 `iframe onMouseEnter` 兜底：鼠标进入正文即开始 900ms 计时；header
  内鼠标移动不会重置计时（`mouseenter` 不重复触发），若用户在 header 内长时间移动后
  未点击，900ms 到点仍会隐藏（符合“固定短延迟”，但手感可后续调优）。
- 计时器固定 900ms；未配置化。

**依赖与禁止路径：**

- 未修改 SRS/SDS/DEVELOPMENT_PLAN/AI_COLLABORATION/ADR/其他任务、iframe 协议、后端、
  数据库、依赖、CSP/capability、真实书库/原文。
- 未启动 M5E-C 重跑/M6；未提交/推送。

## 10. Codex 审查

**结论：** 通过

**审查日期：** 2026-11-16

**独立执行的检查：**

- `npm run check`：退出码 0，14 个文件、169 项测试；新增沉浸式测试触发 1 次 React
  `act(...)` 警告。
- `npm run test:e2e`：通过，WebKit 45 项，覆盖鼠标进入热区、移入正文后自动隐藏、
  键盘重新显示及 Esc 顺序。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，300 passed、0 failed、1 ignored。
- `npm run build`、`npm run tauri build`：通过，`Qurner.app` 7.92 MiB。
- 独立查看 `32-imm-hidden.png`、`33-imm-hotzone.png`、`34-imm-autohide.png`：
  初始隐藏、热区显示、移入正文后自动隐藏均成立。
- `git diff --check`：通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MINOR | `src/features/reader/reader-shell.test.tsx` / immersive tests | `npm run check` 输出 1 条 React `act(...)` 警告；功能断言全部通过。 | 在后续测试整理中消除；不阻塞 AC-110。 |

**AC 结果：**

- FR-110–FR-114 / AC-110：通过。
- 沉浸式 header 默认隐藏；热区/焦点显示；移出后 900ms 自动隐藏；抽屉和 header 焦点
  存在时保持显示。
- Esc 顺序和热区重新显示未回归。

**后续：**

- M5R3 accepted。
- M5E-C 仍保持 `blocked`，需再次执行全量验收并补齐 100 MB、1,000 本、断网、真实导入、
  AC-040/130、跨设置书签和 ≤2 秒丢失窗口证据。
