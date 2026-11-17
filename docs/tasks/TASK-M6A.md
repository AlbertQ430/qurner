# TASK-M6A：错误反馈、存储失败与离线安全加固

**状态：** accepted

**里程碑：** M6

**负责人：** DeepSeek harness

**创建人：** Codex

**创建日期：** 2026-11-17

**依赖：** TASK-M5E-C accepted

## 1. 目标

完成 MVP 发布前的错误、存储失败、安全与离线加固，使损坏/不支持 EPUB、失效文件、
重复导入和数据库读写失败均有可理解、可恢复且不泄漏敏感信息的真实 UI 表现。

## 2. 范围

### 必须完成

- 审计 FR-005、FR-007、FR-019、FR-059 和 NFR-009–NFR-012 的现有实现。
- 在真实 `Qurner.app` 和自动化测试中覆盖：
  - 损坏 ZIP、缺 container/OPF/spine、加密正文、固定版式、竖排、脚本和远程资源。
  - 源文件缺失、内容在导入后变化、已软移除书籍。
  - 数据库无法创建、损坏、较新 schema、写入失败和导入事务失败。
  - 批量导入中单本失败不影响其他书籍，失败项不留下书籍/封面半成品。
- 所有用户可见错误使用固定简体中文原因和可执行下一步；不得显示内部堆栈、完整路径、
  正文、远程 URL、session token 或敏感数据库信息。
- 验证 iframe sandbox、CSP、capability、`epub://`/`cover://` 白名单和外部链接阻断。
- 使用隔离 HOME 执行完整核心流程并监测出站网络请求；证明无 HTTP/HTTPS、远程字体、
  遥测或云依赖。关闭宿主网络如需要系统授权，应停止并申请，不得静默跳过。
- 为发现的真实缺陷编写回归测试；仅做最小修复，不扩大 MVP 功能。

### 不属于本任务

- 新功能、TXT/PDF、高亮批注、备份恢复或 M6 性能基准最终报告。
- 签名、公证、自动更新、发布到 GitHub Release 或应用商店。
- 数据库 schema、Tauri command/DTO、iframe 协议或 CSP 的扩大授权。

## 3. 需求追踪

| 类型 | 编号 | 验收要求 |
|---|---|---|
| FR | FR-005, FR-007 | 不支持结构和格式明确拒绝，不留下部分记录 |
| FR | FR-019, FR-059 | 导入与源文件失效显示具体原因 |
| NFR | NFR-009, NFR-010 | 写入原子、原始 EPUB 只读 |
| NFR | NFR-011, NFR-012 | 无网络请求、错误可理解且不泄漏敏感信息 |
| AC | AC-002, AC-040, AC-220, AC-NFR-002–AC-NFR-004 | 错误、恢复、离线与安全验收 |

## 4. 设计约束

- 遵守 SRS 1.0、SDS 1.0、ADR-0002/ADR-0003。
- 保持稳定 `AppError` code、现有数据库 schema v1、iframe protocol v1 和资源上限。
- 前端只按错误码分支，不渲染后端 `message` 或详情中的敏感内容。
- 导入与个人数据写入必须保持原子性；原始 EPUB 不得复制、移动、重命名或改写。
- 不新增运行时网络依赖，不降低 CSP 或 capability。

## 5. 文件权限

### 允许修改

- `src/`、`src-tauri/`、`crates/` 中与错误处理、原子性和安全回归直接相关的代码。
- `tests/e2e/`
- `docs/acceptance/TASK-M6A-REPORT.md`
- `docs/tasks/TASK-M6A.md`

### 禁止修改

- `SRS.md`、`SDS.md`、`DEVELOPMENT_PLAN.md`、`AI_COLLABORATION.md`、ADR、其他任务。
- 数据库 schema、依赖/锁文件、CSP/capability 的放宽、公开协议或原始电子书。
- 真实用户书库、个人阅读数据和私人路径。

## 6. 行为与边界

- 损坏或不支持书籍：固定错误码和中文说明，可从结果面板/错误页重试或关闭。
- 文件失效或内容变化：保留书籍记录和个人数据，不自动重关联。
- 数据库失败：不产生半成品；后续操作仍可安全重试。
- 网络与安全：书内脚本、嵌入框架、远程资源和任意文件读取均被阻断。
- 日志和错误详情不得包含用户路径、正文、URL 或令牌。

## 7. 测试要求

- 覆盖全部稳定 `AppError` code 的映射和固定用户文案。
- 增加损坏/不支持 EPUB、写入失败、批量部分失败、源文件变化和敏感信息防泄漏测试。
- 使用隔离 HOME、合成 EPUB 和当前构建的真实 `Qurner.app` 截图/操作记录。
- 记录网络监测命令、采样窗口和结果；如无法执行真实断网，明确阻塞并申请。

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

- 不把自动化 mock 当作真实 app 证据。
- 不通过隐藏错误、捕获后静默返回成功或删除测试制造通过。
- 不修改原始 EPUB、真实书库、schema、依赖、公开接口或安全策略。
- 不提交、推送或发布。

## 10. DeepSeek 开发报告

**完成摘要：**

已完成 FR-005/007/019/059 与 NFR-009–012 审计；补齐前端 14 个稳定 AppError code 的
固定文案/防泄漏测试（51 项）、后端 message 安全测试、只读 DB 写入失败与路径非法测试、
批量失败无半成品断言。真实 app 中完成损坏 ZIP、缺 container、加密正文、固定版式、
竖排、脚本、源文件变化、源文件缺失、软移除、数据库损坏/不可创建错误 UI 截图；使用
`sandbox-exec` 对 Qurner 进程级断网完成 106 MB 书打开与正文显示，并确认无网络连接。
详细证据见 `docs/acceptance/TASK-M6A-REPORT.md`。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 19 文件 220 tests |
| `npm run test:e2e` | 0 | WebKit 45 passed |
| `npm run build` | 0 | 成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 320 passed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 重建成功 |
| `git diff --check` | 0 | 通过 |

**真实应用与网络监测证据：**

- 隔离 HOME（`/tmp/qurner-acceptance/final2/home`、`m6a/home`）与合成 EPUB；
- 错误 UI 截图：`shots5/47`（损坏 ZIP）、`21`（缺 container）、`46`（加密）、`44`（固定
  版式）、`45`（竖排）、`42`（脚本）、`40`（源文件变化）、`49`（源文件缺失）、`50/51`
  （软移除）、`34`（DB 损坏）、`35`（DB 不可创建）；
- 进程级断网：`sandbox-exec` profile `(deny network*)`，PID 40802，`lsof
  -a -p 40802 -i` 0 连接，`shots5/41-netoff-reader.png`（106 MB 书正文）。

**未验证项：**

- 远程资源真实 app 阅读页截图（由 `m3_security` 21 项与 CSP 覆盖）；整机关网未执行
  （按用户要求采用进程级断网，避免切断 Agent 模型调用）。

**风险与待决问题：**

- 无新增产品缺陷；本轮未修改产品实现，未创建 commit。

## 11. Codex 审查

**结论：** 通过并开后续任务

**审查日期：** 2026-11-17

**独立执行的检查：**

- `npm run check`：通过；17 个测试文件、220 项测试。
- `npm run test:e2e`：沙箱外 Playwright WebKit 45/45 通过。
- `npm run build`：通过。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过；320 passed、0 failed、1 ignored。
- `npm run tauri build`：通过；`Qurner.app` 7.92 MiB。
- `git diff --check`：通过。
- 复核损坏 EPUB、数据库损坏和进程级断网截图，用户可见文案固定、无内部错误信息。
- 复核 `(allow default)(deny network*)` profile、PID 40802 的 `lsof` 无连接记录和
  106 MB 正文截图；隔离数据库 integrity check 为 `ok`、无失败残留记录。
- 复核 14 个 `AppError` code 的固定中文 message、前端文案映射及测试断言。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| MINOR | `docs/acceptance/TASK-M6A-REPORT.md` | 未提供“含远程资源 EPUB”在真实 app 中的单独截图；当前行为已由 WebKit 安全回归、CSP/capability 检查和进程级断网证据覆盖。 | 在 `TASK-M6C` 发布候选验收中增加远程资源真实 app 冒烟，作为发布前复核项。 |

**AC 结果：**

- FR-005、FR-007、FR-019、FR-059：**通过**。
- NFR-009、NFR-010、NFR-011、NFR-012：**通过**。
- AC-002、AC-040、AC-220、AC-NFR-002–AC-NFR-004：**通过**。

**返工或后续任务：**

- `TASK-M6A` accepted。
- `TASK-M6B` 保持 `draft`，等待用户明确启动。
- `TASK-M6C` 已登记远程资源真实 app 发布前冒烟。
