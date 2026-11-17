# TASK-M6A 开发/验收报告

**状态：** accepted

**日期：** 2026-11-17

**环境：** macOS 26.6 arm64；隔离 HOME `/tmp/qurner-acceptance/final2/home`（1 本书）与
`/tmp/qurner-acceptance/m6a/home`；合成夹具：截断 ZIP、缺 container、固定版式、竖排、
脚本、加密正文、源文件副本；当前构建 `Qurner.app`（commit 5da5013 + M5R4 修复）。

## 1. 审计结论（FR-005/007/019/059、NFR-009–012）

- 后端 14 个稳定 `AppError` code 与固定中文 message（`src-tauri/src/error.rs`），
  `from_book_error`/`from_epub_code` 映射完整；`details` 仅标量。
- 前端只按 code 分支显示固定文案（`import/error-text.ts`、`library/library-errors.ts`、
  `reader/reader-errors.ts`），不渲染后端 `message`/stack/路径。
- 导入/软移除/恢复/进度写入由 storage-core 事务与唯一指纹约束保证原子性；原始 EPUB
  只读；失败导入不留下书籍/封面记录。
- CSP、capability、`epub://`/`cover://` 白名单、iframe sandbox 与脚本剥离由
  `m3_security.rs`（21 项）、`m4_cover.rs`、`m4_dialog.rs` 覆盖，本轮未修改安全策略。
- 网络：真实 app 运行期 `lsof -a -p <qurner> -i` 无任何 TCP/UDP 连接。

## 2. 本轮新增回归测试

| 文件 | 内容 |
|---|---|
| `src/features/import/error-text.test.tsx` | 14 个稳定 code 导入文案固定中文、无路径/URL/内部 code；未知 code 兜底；启动错误文案 |
| `src/features/library/library-errors.test.tsx` | 14 个 code 移除/加载文案；错误分类；无泄漏 |
| `src/features/reader/reader-errors.test.tsx` | 14 个 code 打开文案；重试策略；iframe 文案；会话 token 白名单 |
| `src-tauri/src/error.rs` | 14 个 message 固定中文、长度受限、不含路径/URL/sqlite/内部 code |
| `crates/storage-core/tests/public_api.rs` | 文件路径当数据库打开失败；只读数据库写入失败且错误不含路径/sqlite |
| `crates/library-core/tests/import.rs` | 批量导入失败/重复项后书库仅 1 条记录（无半成品） |

## 3. 真实 app 证据（隔离 HOME，截图 `/tmp/qurner-acceptance/shots5/`）

| 场景 | 结果 | 截图 |
|---|---|---|
| 损坏 ZIP（截断 EPUB） | 失败面板：“文件不是有效或完整的 EPUB，请检查文件。” | `47-corrupt-import.png` |
| 缺 container.xml | 失败面板：同上固定文案，无记录 | `21-no-container-panel-result.png` |
| 加密正文（META-INF/encryption.xml） | 失败面板：“书籍受 DRM 或加密保护，Qurner 不支持。” | `46-encrypted-import.png` |
| 固定版式 | 失败面板：“固定版式 EPUB 暂不支持。” | `44-fixed-import2.png` |
| 竖排 | 失败面板：“竖排 EPUB 暂不支持。” | `45-vertical-import.png` |
| 脚本依赖 | 失败面板：“书籍依赖脚本，Qurner 出于安全原因不支持。” | `42-scripted-import.png` |
| 源文件内容变化（导入后追加字节） | 阅读错误页：“文件内容与书库记录不一致，需要重新导入。”；书库记录保留 | `40-card-click.png` |
| 源文件缺失（移走文件） | 阅读错误页：“原始文件已被移动、删除或无法读取。”；记录保留 | `49-missing-file2.png` |
| 软移除 | 确认对话框（原文说明）+ DB `lifecycle=removed`、书库移除 | `50-remove-dialog.png`、`51-removed.png` |
| 数据库损坏（随机字节 DB） | 启动后错误面板：“无法加载书库，请稍后重试。”+“重试”；进程存活；日志 `E_STORAGE` | `34-db-corrupt.png` |
| 数据库不可创建（数据目录只读） | 同上固定错误面板；日志 `E_STORAGE` | `35-db-denied.png` |
| 进程级断网核心流程 | `sandbox-exec` 仅对该进程 `(deny network*)`；打开 106 MB 书并显示正文；`lsof -a -p <pid> -i` 0 连接 | `41-netoff-reader.png` |

断网证据（`/tmp/qurner-acceptance/m6a/netoff-evidence.txt`）：

- profile：`(version 1)(allow default)(deny network*)`；
- 启动：`HOME=... sandbox-exec -f net-off.sb <Qurner.app>/Contents/MacOS/qurner`；
- PID 40802，`lsof -a -p 40802 -i` 0 行（无连接）；截图 `41-netoff-reader.png`；
- 仅约束 Qurner 进程，宿主机与 Agent 模型调用未受影响（未关闭整机网络）。

远程资源：真实 app 未单独导入“含远程 img 的书”截图；`epub-core` 对远程资源只发出
`RemoteResourceBlocked` 警告，阅读器 CSP `img-src`/`frame-src` 仅允许 `self`/`epub:`/
`cover:`，由 `m3_security.rs` 21 项（脚本剥离、相对 URL 保留、远程资源移除、CSP nonce）
与 `m4_cover.rs` 覆盖。

## 4. 门禁（本轮实际执行）

| 命令 | 退出码 | 结果 |
|---|---:|---|
| `npm run check` | 0 | tsc、Biome、Vitest 19 文件 220 tests（新增 51） |
| `npm run test:e2e` | 0 | Playwright WebKit 45 passed |
| `npm run build` | 0 | 生产前端构建成功 |
| `cargo fmt --all --check` | 0 | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test --workspace` | 0 | 320 passed，0 failed，1 ignored |
| `npm run tauri build` | 0 | Qurner.app 重建成功 |
| `git diff --check` | 0 | 通过 |

## 5. 未验证项

- 远程资源真实 app 阅读页截图（安全行为由自动化覆盖，见第 3 节说明）。
- 真实断网采用 **进程级沙箱断网**（非整机关网），满足“不切断 Agent 模型调用”的约束；
  未验证整机关网场景。
- 1,000 本 / 100 MB 性能属于 M5E-C，不在本任务范围。

## 6. 交付文件

- 新增测试：`src/features/import/error-text.test.tsx`、
  `src/features/library/library-errors.test.tsx`、
  `src/features/reader/reader-errors.test.tsx`；
- 修改测试：`src-tauri/src/error.rs`（测试）、`crates/storage-core/tests/public_api.rs`、
  `crates/library-core/tests/import.rs`；
- 报告：`docs/acceptance/TASK-M6A-REPORT.md`、`docs/tasks/TASK-M6A.md`。

未修改 SRS/SDS/PLAN/ADR/其他任务、数据库 schema、依赖、CSP/capability/iframe 协议、
原始 EPUB 与真实书库；未提交 commit。
