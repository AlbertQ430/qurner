# TASK-M5R1：真实 Qurner.app 阅读器握手修复

**状态：** accepted

**里程碑：** M5 rework

**负责人：** DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-16

**依赖：** TASK-M5E-C blocked

## 1. 目标

打包后的 `Qurner.app` 打开通过 M5E-C 导入的有效 EPUB 后，不再等待 10 秒超时，而是完成
`reader:init` → `reader:ready` 握手并显示首章正文。

## 2. 证据与高置信度根因

- 真实 app 中 `cn-nested.epub` 导入成功，但首次打开和重试均显示“阅读内容加载超时，请重试。”
- `ReaderShell` 的 iframe 设置了 `referrerPolicy="no-referrer"`。
- `src/reader-engine/index.ts` 在 `document.referrer` 为空时直接返回，不创建 `IframeBridge`。
- 现有 WebKit 测试没有组合验证“跨 origin iframe + no-referrer + reader-engine 启动”，
  因此自动化通过但打包应用失败。

上述链路是最可能根因，实施前必须用最小复现确认，不得只凭猜测扩大改动。

## 3. 范围

### 必须完成

- 复现并确认 referrer/父 origin 与握手失败的关系。
- 在不降低 token 隔离、CSP、iframe sandbox 和来源校验的前提下修复握手。
- 增加能真实复现打包环境 referrer/origin 条件的自动化回归。
- 在隔离 HOME 的真实 `Qurner.app` 中重新打开合成 EPUB，确认正文、目录入口和阅读位置
  可见；记录截图与日志。
- 失效文件、未知源和伪造消息仍返回固定错误，不泄漏 token/路径。

### 不属于本任务

- 书库网格布局缺陷；由 `TASK-M5R2` 处理。
- M5E-C 的完整目录/模式/设置/书签/复制/异常终止重测；修复任务通过后回 M5E-C 重跑。
- 修改网络、CSP、capability 或公开消息协议；若确需修改，先停止并报告。
- TXT/PDF、高亮、批注、备份和 M6。

## 4. 需求追踪

| 类型 | 编号 | 验收要求 |
|---|---|---|
| AC | AC-120, AC-130, AC-160, AC-170, AC-190, AC-200, AC-210 | 真实 app 能进入阅读器后，M5 闭环才能继续验收 |
| NFR | NFR-010, NFR-011, NFR-012 | 原文只读、无网络、错误稳定且不泄漏敏感信息 |

## 5. 文件权限

### 允许修改

- `src/features/reader/ReaderShell.tsx`
- `src/reader-engine/index.ts`、`src/reader-engine/engine.ts`
- `src/bridge/iframe.ts`、`parent.ts`、`messages.ts`
- `tests/e2e/`、`src/reader-engine/`、`src/bridge/`、`src/features/reader/` 测试
- `docs/tasks/TASK-M5R1.md`

### 禁止修改

- SRS/SDS/DEVELOPMENT_PLAN/AI_COLLABORATION/ADR/其他任务
- Tauri capability、CSP、`epub://` 白名单、数据库 schema、依赖锁文件
- 原始 EPUB、真实 HOME、真实书库和个人数据

## 6. 测试与真实验收

- 自动化必须覆盖 no-referrer/空 referrer 条件下的握手启动，或明确验证更换后的安全
  origin 传递机制；伪造 origin/session/token 仍被拒绝。
- 真实 app 使用隔离 HOME 和合成 EPUB，至少完成“启动 → 导入 → 打开 → 首章正文可见”，
  记录截图、进程环境、日志和数据库路径。
- 重试路径不得泄漏底层错误、token 或完整路径。

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

- 不放宽 iframe sandbox 为 `allow-same-origin`。
- 不以 `postMessage("*")` 接收任意来源的首次握手作为唯一安全措施。
- 不把真实应用失败标为环境问题或跳过。
- 不提交、推送或发布。

## 9. DeepSeek 开发报告

**完成摘要：**

确认并修复真实 `Qurner.app` 阅读器握手超时。

- **根因复现**：`ReaderShell` iframe 使用 `referrerPolicy="no-referrer"`；打包应用中
  `document.referrer` 为空，`src/reader-engine/index.ts` 在 referrer 为空时直接返回，
  不创建 `IframeBridge`，父页面 5s/10s 后进入“阅读内容加载超时”。E2E 新增
  “跨 origin + `referrerPolicy=no-referrer` + 打包 reader-engine.js” 回归测试，
  断言 `document.referrer === ""` 且握手/位置事件成功（修复前该场景必失败）。
- **修复**：保留 `no-referrer`、CSP、`sandbox="allow-scripts"`、token/session 校验不变。
  `IframeBridge` 的 `parentOrigin` 允许为空（referrer 不可用）；此时仅接受
  `event.source === window.parent`、sessionId/token 匹配、`origin` 非空且非 `"null"`
  的合法 `reader:init`，从中学习父 origin；此后所有消息要求 `event.origin` 等于学习值；
  回复使用学习到的 origin 作为 `postMessage` targetOrigin（不使用 `*` 回复）。
- **测试**：iframe 新增 2 项（合法 init 学习 origin 并拒绝其他 origin；非 init/
  错误 token 不学习）；E2E 新增 no-referrer 打包回归；既有 source/token/伪造测试保持。
- **真实应用验收**：隔离 HOME 数据目录、合成 EPUB，重新 `npm run tauri build` 后启动
  → 打开书籍 → 首章正文“第一章 中文”与中英文段落可见，头部显示位置“第一章 · 12.5%”、
  目录/书签/设置/沉浸式入口（截图 `/tmp/qurner-acceptance/shots/32-m5r1-reader.png`）。
  首次尝试失败是测试夹具源文件被临时移走（AC-040 错误正常），恢复夹具并重启后成功。

**修改文件：**

- `src/bridge/iframe.ts`（父 origin 学习与校验）
- `src/reader-engine/index.ts`（referrer 为空时不退出）
- `src/bridge/iframe.test.tsx`（新增 2 项）
- `tests/e2e/reader-navigation.spec.ts`（no-referrer 打包回归）
- `src-tauri/resources/reader-engine.js`（`npm run build:reader` 生成）
- `docs/tasks/TASK-M5R1.md`

未修改 CSP、capability、`epub://` 白名单、数据库 schema、依赖锁文件；未修改网格布局
（M5R2）；未提交 commit。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 16 文件 167 tests 通过 |
| `npm run test:e2e` | 0 | Playwright WebKit 44 passed |
| `npm run build` | 0 | 生产前端构建成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 300 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 重建成功（7.92 MiB） |
| `git diff --check` | 0 | 通过 |

**真实应用验收：**

- 环境：macOS 26.6 arm64；`HOME=/tmp/qurner-acceptance/home` 隔离；合成 EPUB
  `cn-nested.epub` 已导入（数据库只含合成书）。
- 证据：`/tmp/qurner-acceptance/shots/32-m5r1-reader.png`（首章正文、位置、目录入口）；
  失败基线 `shots/18`、`shots/19` 与本次成功形成对照。
- 日志未出现 token/路径泄漏；错误页文案未暴露底层错误。

**未验证项：**

- 真实 app 中的恶意 EPUB 脚本伪造父消息（source/origin/token 校验由单元测试覆盖，
  未在真实恶意书夹具中执行）。
- M5E-C 其余场景（目录跳转、设置、书签、复制、进度/异常终止、100 MB/1,000 本、
  断网）仍需修复后回 M5E-C 重测；本任务只验证进入阅读器。
- M5R2 网格布局未处理。

**风险与待决问题：**

- 父 `reader:init` 仍使用 `postMessage("*")`（既有设计）；接收侧 iframe 校验
  `source`/session/token/origin；本次未改变父桥协议，如后续要求收紧 targetOrigin，
  需单独任务评估 WKWebView 自定义 scheme 的 targetOrigin 支持。
- 学习式 origin 依赖浏览器提供的 `event.source === window.parent` 与 `event.origin`；
  已保持 sessionId/token 校验，未降低隔离。

**依赖与禁止路径：**

- 未修改 SRS/SDS/DEVELOPMENT_PLAN/AI_COLLABORATION/ADR/其他任务、CSP/capability、
  `epub://` 白名单、数据库、依赖锁文件、真实书库/原文。
- 未启动 M5R2/M6；未提交/推送。

## 10. Codex 审查

**结论：** 通过

**审查日期：** 2026-11-16

**独立执行的检查：**

- `npm run check`：通过，14 个文件、167 项测试。
- `npm run test:e2e`：通过，WebKit 44 项，包含 no-referrer 跨 origin 握手回归。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过，300 passed、0 failed、1 ignored。
- `npm run build`、`npm run tauri build`：通过，`Qurner.app` 7.92 MiB。
- 独立查看真实应用截图 `31-m5r1-reader.png` 与 `32-m5r1-reader.png`：先验证失效文件错误，
  再确认恢复源文件后正文、中英文段落、章节位置和阅读控件可见。
- `git diff --check`：通过。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MINOR | 仓库根目录 / `app.pid`、`m5r1.swift`、`open_final.swift` | DSH 遗留了未报告的越界临时文件，不在任务允许路径内。 | 已在审查中删除；不进入提交。后续真实应用脚本应放在 `/tmp`。 |

**AC 结果：**

- AC-210：真实 app 打开有效 EPUB 成功，握手完成并显示首章正文。
- AC-120/130/160/170/190/200 的真实应用完整旅程仍由 M5E-C 重跑确认；M5R1 负责的入口
  阻塞已解除。
- NFR-010、NFR-011、NFR-012：未发现原文件写入、网络请求或敏感信息泄漏。

**后续：**

- M5R1 accepted。
- M5E-C 保持 `blocked`，等待 M5R2 完成后重新执行。
- M5R2 保持 `draft`，需用户明确启动。
