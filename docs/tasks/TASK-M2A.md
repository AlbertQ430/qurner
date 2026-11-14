# TASK-M2A：SQLite 基础与迁移

**状态：** accepted  
**里程碑：** M2  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** M1 accepted；M2 依赖已由用户在 2026-11-14 授权

## 1. 目标

在 `storage-core` 建立 SQLite schema v1、事务迁移和专用写线程，为 M2 导入与书库查询
提供稳定持久化边界。

本任务不实现 EPUB 导入、书库 UI 或 Tauri command。

## 2. 已授权依赖

- `rusqlite`，启用 `bundled`。
- `reader-model` workspace 依赖。
- `uuid`、`time`、`unicode-collation` 已授权给 M2 后续任务，但本任务不得提前使用。

如实现需要额外依赖，必须停止并请求授权。

## 3. 数据库入口

建议公开接口：

```rust
pub struct StorageService;

impl StorageService {
    pub fn open(path: &Path) -> Result<Self, StorageError>;
    pub fn schema_version(&self) -> Result<u32, StorageError>;
    pub fn shutdown(self) -> Result<(), StorageError>;
}
```

- 专用后台线程持有唯一写连接。
- 调用方通过 channel 提交请求，未完成请求不能阻塞调用方线程之外的其他 UI 任务。
- `StorageService` 可在线程间安全传递。
- shutdown 等待当前事务完成并关闭连接。

## 4. SQLite 设置

每个新连接执行：

- `PRAGMA foreign_keys = ON`
- `PRAGMA journal_mode = WAL`
- `PRAGMA synchronous = NORMAL`
- `PRAGMA busy_timeout = 5000`

迁移必须在事务中执行；迁移失败回滚。不得删除或重建未知表。

## 5. Schema v1

创建 SDS 8.3 中的全部表：

- `schema_migrations`
- `books`
- `reading_states`
- `book_settings`
- `bookmarks`
- `app_settings`

同时创建：

- 外键和 `ON DELETE CASCADE`。
- 搜索与导入时间索引。
- `books.fingerprint` 唯一约束。
- `manifest_json` / `manifest_version`。
- `lifecycle IN ('active', 'removed')`。

实现可增加必要的 NOT NULL 或索引，但不得改变 SDS 字段语义。

## 6. 错误模型

定义 `StorageError`：

- `OpenFailed`
- `MigrationFailed`
- `QueryFailed`
- `TransactionFailed`
- `Busy`
- `UnsupportedSchemaVersion`
- `ThreadUnavailable`

错误不得包含数据库绝对路径或 SQLite 原始错误文本；可保留稳定 code。

## 7. 测试

- 新数据库创建全部表和索引。
- 迁移重复执行保持幂等。
- `schema_version == 1`。
- 外键开启；插入无父记录的子表失败。
- 非法 lifecycle 被拒绝。
- 相同 fingerprint 唯一约束生效。
- 事务失败回滚，无半成品。
- 两个并发请求由写线程串行完成。
- shutdown 后新请求返回 `ThreadUnavailable`。
- 文件路径含中文和空格时可打开。

## 8. 文件权限

### 允许修改

- `Cargo.toml`
- `Cargo.lock`
- `crates/storage-core/Cargo.toml`
- `crates/storage-core/src/`
- `crates/storage-core/tests/`
- 本任务顶部状态和第 10 节开发报告

### 禁止修改

- `src/`
- `src-tauri/`
- `crates/epub-core/`
- `crates/reader-model/`
- `crates/library-core/`
- 所有需求和设计文档
- 其他任务文件

## 9. 必跑命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p storage-core
cargo test --workspace
npm run check
```

## 10. DeepSeek 开发报告

> 任务转为 `ready` 后，DeepSeek 开始实现。完成后填写本节并改为
> `ready_for_review`。

**完成摘要：**

在 `storage-core` 建立 SQLite schema v1、事务迁移和专用写线程：

- 接入 `rusqlite 0.42.0`（启用 `bundled`），未新增其他依赖；未提前使用
  `uuid`、`time` 或 `unicode-collation`。
- `StorageService::open(path)` 在专用 `qurner-storage` 线程中持有唯一写连接，
  调用方通过 channel 提交请求；`StorageService` 可克隆、`Send + Sync`。
- 每个连接执行 `foreign_keys=ON`、`journal_mode=WAL`（并验证返回 `wal`）、
  `synchronous=NORMAL`、`busy_timeout=5000`。
- schema v1 创建 SDS 8.3 的全部表与索引，包含外键、`ON DELETE CASCADE`、
  `fingerprint` 唯一约束、`lifecycle` CHECK、`manifest_json`/`manifest_version`。
- 迁移在事务中执行，重复执行幂等；更高 schema 版本返回
  `UnsupportedSchemaVersion`；迁移失败完整回滚。
- `StorageError` 提供七个稳定错误类别（camelCase code + 中文 Display），不包含数据库
  路径或 SQLite 原始错误文本。
- `shutdown(self)` 等待已排队请求完成、关闭连接并结束线程；shutdown 后其他克隆句柄
  的请求返回 `ThreadUnavailable`。

**修改文件：**

- `Cargo.toml`、`Cargo.lock`（rusqlite bundled 依赖）
- `crates/storage-core/Cargo.toml`
- `crates/storage-core/src/lib.rs`
- `crates/storage-core/src/error.rs`（新增）
- `crates/storage-core/src/schema.rs`（新增）
- `crates/storage-core/src/service.rs`（新增）
- `crates/storage-core/tests/public_api.rs`（新增）
- `docs/tasks/TASK-M2A.md`（顶部状态与本第 10 节）

未修改 `src/`、`src-tauri/`、`crates/epub-core/`、`crates/reader-model/`、需求/设计文档
和其他任务文件，未创建 commit。

**Schema 与迁移：**

- 创建表：`schema_migrations`、`books`、`reading_states`、`book_settings`、`bookmarks`、
  `app_settings`。
- 创建索引：`idx_books_title_search`、`idx_books_author_search`、
  `idx_books_imported_at`、`idx_bookmarks_book_created`。
- 约束验证：
  - 外键开启；插入无父记录的 `reading_states` 失败；
  - 删除 `books` 记录级联删除 `bookmarks`；
  - 非法 `lifecycle`（`broken`）被 CHECK 拒绝；
  - 相同 `fingerprint` 的第二条记录被 UNIQUE 拒绝。
- 迁移行为：
  - 同一数据库连续迁移三次，`schema_migrations` 仅一行，`schema_version == 1`；
  - 手动插入 version 99 后迁移返回 `UnsupportedSchemaVersion`；
  - 预先创建结构冲突的 `books` 表时迁移返回 `MigrationFailed`，且事务回滚后不存在
    `schema_migrations` 表（无半成品）；
  - 中文和空格路径（`中文 路径/书库 数据.sqlite3`）可正常打开与迁移。
- PRAGMA 实测：`foreign_keys=1`、`journal_mode=wal`、`synchronous=1`、
  `busy_timeout=5000`。

**写线程与错误：**

- 写线程 test-only 钩子 `TestWork` 记录并发数：两个请求的最大并发为 1，证明请求由
  单写线程串行完成。
- shutdown 等待已入队 `TestWork` 完成后才返回（完成信号在 shutdown 返回前已收到）。
- shutdown 后克隆句柄调用 `schema_version()` 返回 `ThreadUnavailable`；错误 Display
  不含数据库路径和 `sqlite` 文本。
- 编译期断言 `StorageService: Send + Sync`。
- 新增 `tests/public_api.rs` 覆盖公开入口、克隆句柄 shutdown 可见性、中文空格路径和
  `Send + Sync`。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p storage-core` | 0 | 19 passed（lib 15、public_api 4、doc-tests 0） |
| `cargo test --workspace` | 0 | 147 passed，0 failed，1 ignored |
| `npm run check` | 0 | Biome 14 个文件无修复；Vitest 2/2 通过 |

依赖：`rusqlite 0.42.0`（bundled），`Cargo.lock` 已更新；未新增其他依赖。

**未验证项：**

- 未构造真实 SQLITE_BUSY 场景验证 `StorageError::Busy` 映射（并发锁冲突/超时）。
- 未验证同一文件被多个 `StorageService` 实例同时打开时的 WAL/锁行为。
- 未测试强制终止进程后的 WAL 恢复或崩溃一致性。
- 未测试磁盘满、只读文件、权限拒绝等 I/O 失败路径。
- 迁移目前只有 v1；未来迁移脚本与升级回滚测试需新增任务。
- 写线程当前串行处理所有请求（含 schema 查询）；M2B 增加查询时可能需要只读连接池，
  尚不确定。
- 未测量 bundled SQLite 对构建时间与二进制体积的具体影响。

**风险与待决问题：**

- WAL 模式要求文件系统支持共享内存；网络卷等场景可能导致打开失败（返回
  `OpenFailed`）。
- 数据库存在同名但结构不兼容的 `books` 等表时，迁移会失败并回滚，不会自动修复；
  需用户处理或后续迁移脚本。
- `shutdown(self)` 消耗调用方句柄；其他克隆在 shutdown 后不可用，调用方需自行协调
  生命周期。
- 错误模型不保留底层错误详情，排障只能依赖稳定 code 与外部日志；后续如需要可在
  内部日志记录（不进入公开错误）。
- 写线程使用标准 channel，无超时；若 worker 卡在 SQLite 锁上，调用方可能无限等待
  （受 `busy_timeout=5000` 限制）。
- 测试中的 `TestWork` 钩子仅在 `cfg(test)` 编译，不进入生产 API。

## 11. Codex 审查

**结论：** 通过并开后续任务  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p storage-core`：19 passed。
- `cargo test --workspace`：147 passed，1 ignored。
- `npm run check`：通过，Vitest 2/2。
- schema、索引、PRAGMA、迁移幂等、回滚、外键、CHECK、UNIQUE、并发串行和 shutdown
  测试全部通过。
- `cargo tree -p storage-core` 仅新增 `rusqlite 0.42.0` 及其 bundled SQLite 依赖。
- 未发现受保护文件被 DSH 修改；没有新增 commit。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| `MINOR` | `crates/storage-core/Cargo.toml` / `description` | 描述仍为“M0 仅建立 crate 边界”，与当前实现不符。 | 并入 M2B，更新为 SQLite 存储、迁移和写线程核心。 |

**M2A 结果：**

SQLite schema v1、事务迁移、PRAGMA、错误模型和专用写线程满足任务要求，M2A 验收通过。

**返工或后续任务：**

1. M2B 同步修正 crate description。
2. M2B 增加具体书籍/事务请求类型，继续复用当前单写线程。
