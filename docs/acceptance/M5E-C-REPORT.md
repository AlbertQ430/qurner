# TASK-M5E-C 验收报告：M5 阅读器闭环总验收

**日期：** 2026-11-16
**执行人：** DeepSeek harness（自动化 CGEvent + 真实 Qurner.app）
**结论：** **不通过（BLOCKER 未关闭）**。真实 Qurner.app 无法进入阅读器；建议先创建 rework 修复真实 app 的 `epub://` 阅读内容加载与书库网格视图问题，再重新执行本验收。

## 1. 环境与隔离

| 项目 | 值 |
|---|---|
| 系统 | macOS 26.6（arm64） |
| Qurner.app | `target/release/bundle/macos/Qurner.app`（`npm run tauri build` 产物，7.92 MiB） |
| Rust | 1.100.0 |
| Node | 26.13.0 |
| WebKit | 系统 WebKit（Playwright WebKit 43 项自动化通过） |
| 数据隔离 | `HOME=/tmp/qurner-acceptance/home` 启动真实 app；应用数据目录 `home/Library/Application Support/com.qurner.reader/`，未使用/未修改真实用户书库 |
| 操作方式 | Swift + CGEvent 鼠标/键盘自动化（System Events 无辅助访问权限，CGEvent 可用）；截图 `/tmp/qurner-acceptance/shots/` |
| 合成夹具 | `/tmp/qurner-acceptance/fixtures/`（中文嵌套图片 EPUB3、English EPUB2、损坏 ZIP）、`big/book-100mb.epub`（101.13 MB）、`thousand/`（1,000 本）；SHA-256 见夹具生成日志 |

## 2. 真实应用场景结果

### 场景 A：完整主流程

| 步骤 | 结果 | 证据 |
|---|---|---|
| 启动 Qurner.app | 通过 | `shots/01-launch.png` |
| 导入合成 EPUB3（中文/嵌套目录/内嵌 CSS/图片/缺失字体） | 通过（“已导入 1 已恢复 0 重复 0 失败 0”） | `shots/04`–`shots/11` |
| 书库显示 | **失败（MAJOR）**：默认网格视图卡片异常，条纹封面占位几乎占满视口，标题/作者不可见；切列表视图正常 | `shots/12`–`shots/17`、`shots/22`–`shots/25` |
| 打开书籍、首章 | **失败（BLOCKER）**：阅读器显示“阅读内容加载超时，请重试。”，无正文 | `shots/18`；重试仍超时 `shots/19` |
| 目录、分页/滚动、字号/主题、沉浸式、书签、复制、进度恢复 | **未验证**（阅读器不可用；不得用 mock 结果替代真实 app 证据） | — |
| 失效文件（移动源文件后打开） | 通过：显示“原始文件已被移动、删除或无法读取。”并可返回书库 | `shots/29` |
| 关闭/再次启动 | 应用可启动；书库记录保留（SQLite 查询 1 本）；阅读位置恢复未验证（BLOCKER） | `shots/15`、DB 查询 |

### 场景 B：异常终止

- 读取位置变化后 `kill -9` 终止，重启后 SQLite 中书籍记录保留（AC 数据不丢）。
- 阅读进度恢复（≤2 秒）**未验证**：阅读器不可用，无法产生阅读位置。

### 场景 C：规模与性能

- 1,000 本：真实 app 批量导入未执行（自动化 E2E 1,000 本性能用例通过；不能替代真实 app 证据）→ **未验证**。
- 100 MB：首次导入因**夹具缺陷**失败（100 MB 夹具 nav.xhtml 缺少 `xmlns:epub`，报“文件不是有效或完整的 EPUB”）；修复夹具后真实 app 导入未成功触发，**未验证**。
- 分页/滚动/模式切换响应：**未验证**（BLOCKER）。

### 场景 D：安全与离线

- `lsof -p <qurner pid>` 三轮采样：qurner 进程**无 TCP/UDP 网络连接**（仅有本地 SQLite、unix socket、系统库）→ 无遥测/远程字体/HTTP 请求证据。
- 真实断网（关闭 Wi-Fi/网络）未执行（避免影响宿主环境）→ **未验证**。
- CSP：`frame-src epub:`、`connect-src ipc: http://ipc.localhost`、`script-src 'self'`；iframe `sandbox="allow-scripts"`（代码审查）。
- token/路径未出现在 DOM/日志的自动化覆盖见 M5A/M5B 测试；真实 app 无法进一步验证（阅读器不可用）。

## 3. 缺陷清单

| 级别 | 现象 | 复现步骤 | 证据 | 影响 |
|---|---|---|---|---|
| **BLOCKER** | 打开有效 EPUB 后阅读器 10 秒超时，显示“阅读内容加载超时，请重试。” | 隔离 HOME 启动 Qurner.app → 导入 `cn-nested.epub`（EPUB3 有效）→ 列表视图打开 → 阅读器空白/超时；点“重新加载阅读页”仍超时 | `shots/18-retry/19` | 无法阅读：AC-120/130/160/170/190/200/210 及 M5 主流程全部阻塞 |
| **MAJOR** | 书库默认网格视图卡片尺寸异常：条纹封面占位几乎占满视口，仅底部露出书名；列表视图正常 | 导入 1 本后默认网格视图；切“列表视图”恢复 | `shots/12`–`shots/17` | 默认视图无法浏览书籍（AC-040/210 书库显示） |
| MINOR（夹具） | 101 MB 合成 EPUB 首次导入报“文件不是有效或完整的 EPUB” | 夹具 `nav.xhtml` 未声明 `xmlns:epub`；修复后未完成真实导入验证 | `shots/26` | 非产品缺陷；100 MB 真实导入仍未验证 |
| 环境 | `osascript System Events` 无辅助访问权限 | — | 以 CGEvent 自动化绕过 | 不影响产品 |

## 4. AC 追踪（M5 闭环）

| AC | 自动化证据（M5A–M5E-B） | 真实 app 证据 | 结论 |
|---|---|---|---|
| AC-040 失效文件 | Rust/e2e 错误映射 | 缺文件打开显示固定中文错误（`shots/29`） | **通过（真实 app）**；软移除真实 app 未验证 |
| AC-070 模式切换与持久化 | Vitest/WebKit 通过 | 阅读器不可用 | **阻塞** |
| AC-080 字号与原文样式 | Vitest/WebKit 通过 | 阅读器不可用 | **阻塞** |
| AC-090 全局/单书设置 | Vitest/e2e 通过 | 阅读器不可用 | **阻塞** |
| AC-100 昼夜主题 | Vitest/WebKit 通过 | 阅读器不可用 | **阻塞** |
| AC-110 沉浸式 | Vitest/e2e 通过 | 阅读器不可用 | **阻塞** |
| AC-120 目录 | Vitest/WebKit 通过 | 阅读器不可用 | **阻塞** |
| AC-130 正文图片 | WebKit e2e 通过 | 阅读器不可用 | **阻塞** |
| AC-160 进度保存/恢复/异常退出 | Rust/Vitest/e2e 通过 | 阅读器不可用；仅数据保留 | **阻塞** |
| AC-170 书签 | Rust/Vitest/e2e 通过 | 阅读器不可用 | **阻塞** |
| AC-190 选择/复制 | WebKit copy 事件通过（剪贴板人工） | 阅读器不可用 | **阻塞** |
| AC-200 键盘 | Vitest/WebKit 通过 | 阅读器不可用 | **阻塞** |
| AC-210 打开书籍 | Vitest/WebKit mock 通过 | 真实 app 阅读器超时（BLOCKER） | **失败（真实 app）** |
| AC-220 离线 | 无网络代码/请求；lsof 无连接 | 未断网实测 | **部分证据（lsof）**；断网未验证 |

## 5. 门禁结果（独立执行）

| 命令 | 退出码 | 结果 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 16 文件 165 tests |
| `npm run test:e2e` | 0 | Playwright WebKit 43 passed |
| `npm run build` | 0 | 生产前端构建成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 300 passed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 7.92 MiB |
| `git diff --check` | 0 | 通过 |

## 6. 结论与建议

- M5 阅读器闭环**不通过**：真实应用主流程在“打开书籍”处被 BLOCKER 阻断，书库默认网格视图另有 MAJOR 布局问题。
- 建议：
  1. 新建 rework 任务修复真实 app 中 `epub://` iframe/reader-engine 加载（检查 WKWebView 自定义 scheme、iframe `sandbox`、CSP `frame-src`、协议响应/时序）。
  2. 修复书库网格视图卡片尺寸/虚拟行高度（列表视图正常对照）。
  3. 修复后重新执行 M5E-C 真实 app 旅程（目录、设置、书签、复制、进度、异常终止、100 MB/1,000 本、断网）。
- 本报告不修改产品实现；未提交 commit。
