# TASK-M2B：导入与软移除恢复

**状态：** accepted  
**里程碑：** M2  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** TASK-M2A accepted；用户已批准 `icu_collator 2.4.0`

## 1. 目标

新增 `library-core` 应用用例层，串联 EPUB Core 和 Storage Service，完成单本/批量导入、
SHA-256 判重、软移除恢复和原子写入。同时修正 `storage-core` 中仍写着 M0 边界状态的
crate description。

## 2. 已授权依赖

- `uuid 1.x`，启用 `v4`，用于书籍 ID。
- `time 0.3.x`，启用 `formatting`，用于 UTC RFC 3339 时间。
- `icu_collator 2.4.0`（Unicode-3.0），默认 `compiled_data`，用于排序键。
- 不再使用不存在的 `unicode-collation`。

依赖版本必须记录在锁文件并通过许可证检查。

## 3. 导入结果

```rust
pub enum ImportStatus {
    Imported,
    Reactivated,
    Duplicate,
    Failed,
}

pub struct ImportItemResult {
    pub status: ImportStatus,
    pub book_id: Option<String>,
    pub error: Option<BookError>,
}
```

批量逐本处理，单本失败不中止其他文件。结果保留输入顺序。

## 4. 单本流程

1. 计算 SHA-256。
2. 查询 fingerprint。
3. active 记录返回 duplicate，不覆盖数据。
4. removed 记录重新激活，更新 source path 和 imported_at，保留个人数据。
5. 新书解析 EPUB。
6. 解析结束后再次计算 SHA-256。
7. 前后不一致返回 `SourceChanged`，不写任何记录。
8. 生成 UUID、时间、搜索键和排序键。
9. 单个事务写 books 与 manifest。
10. 返回 Imported。

cover cache 本任务可保持 `NULL`；M4 前补封面缓存任务。

## 5. 存储接口

在 `storage-core` 增加类型化请求，不允许 `library-core` 直接使用 `rusqlite::Connection`：

```rust
pub struct NewBookRecord {
    pub id: String,
    pub fingerprint: String,
    pub source_path: String,
    pub title: String,
    pub author: String,
    pub title_search: String,
    pub author_search: String,
    pub title_sort_key: Vec<u8>,
    pub author_sort_key: Vec<u8>,
    pub manifest_json: String,
    pub manifest_version: u32,
    pub imported_at: String,
}

pub struct ExistingBookRecord {
    pub id: String,
    pub source_path: String,
    pub lifecycle: String,
}
```

`StorageService` 至少增加：

- `find_by_fingerprint(&str) -> Result<Option<ExistingBookRecord>, StorageError>`
- `insert_new_book(NewBookRecord) -> Result<(), StorageError>`
- `reactivate_book(id, source_path, imported_at) -> Result<(), StorageError>`
- `soft_remove_book(id) -> Result<(), StorageError>`

要求：

- 所有操作通过专用写线程。
- `insert_new_book` 在事务中写入；UNIQUE 冲突不得留下部分写入。
- 若并发导入同一 fingerprint，后完成者返回 Duplicate。
- `soft_remove_book` 只把 lifecycle 更新为 removed，不删除关联数据。
- Reactivate 只更新 lifecycle、source_path、imported_at，不重置进度、书签或设置。

## 6. 字段规则

- ID：`Uuid::new_v4().to_string()`，小写标准 UUID。
- 时间：UTC RFC 3339，精确到秒，例如 `2026-11-14T07:00:00Z`。
- fingerprint：`BookFingerprint` 的 64 字符小写十六进制。
- `source_path`：绝对路径 UTF-8；非 UTF-8 返回 `FileUnreadable`。
- title：缺失时使用文件 stem；为空时“未命名书籍”。
- author：有多个作者时按顺序用 `", "` 连接；无作者时“未知作者”。
- `title_search` / `author_search`：Unicode `to_lowercase()` 后的文本。
- 排序键：使用 `icu_collator::Collator::try_new(Default::default(), Default::default())`
  和 `write_sort_key_to` 生成 BLOB；中英文使用同一确定性配置。
- `manifest_json`：`serde_json::to_string(&ParsedBook)`。
- `manifest_version`：`ParsedBook.schema_version`。
- `cover_cache_key`：本任务写 `NULL`。

## 7. 元数据回退

- 缺失 title：非空文件名 stem；仍为空则“未命名书籍”。
- 缺失 author：持久化显示字段使用“未知作者”。
- manifest JSON 使用 `ParsedBook` camelCase 序列化与 schema version。
- 单书设置不预填，由设置继承逻辑处理。

## 8. 错误模型

`library-core::BookError` 至少包含：

- FileNotFound
- FileUnreadable
- InvalidInput
- Epub(EpubErrorCode)
- SourceChanged
- Storage(StorageErrorKind)
- Internal

不得泄漏绝对路径、rbook 或 SQLite 类型。

## 9. 测试

- 6 个有效夹具导入。
- 相同字节不同文件名 duplicate。
- 内容一字节变化生成新书。
- removed 后重新导入 reactivated。
- duplicate 不覆盖进度/书签。
- 解析失败无 books/manifest 残留。
- 源文件解析前后变化返回 SourceChanged。
- 批量混合成功/重复/失败，顺序稳定。
- 中文和英文标题/作者排序键稳定。

## 10. 文件权限

允许创建/修改：

- `Cargo.toml`
- `Cargo.lock`
- `crates/library-core/`
- `crates/storage-core/`（仅 M2A 声明的持久化接口）
- `crates/epub-core/`（仅必要的读资源接口）
- `crates/storage-core/Cargo.toml` 的 crate description
- 本任务状态和报告

禁止修改需求、设计、其他任务和 UI。

## 11. 必跑命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p library-core
cargo test -p storage-core
cargo test --workspace
npm run check
```

## 12. 开发报告

**完成摘要：**

新增 `library-core` 用例层，串联 EPUB Core 与 Storage Service：

- `LibraryService::{import, import_batch, soft_remove}`；`ImportStatus` 覆盖
  `Imported` / `Reactivated` / `Duplicate` / `Failed`，单本失败不中止批量，结果保持输入
  顺序。
- 单本流程：SHA-256 → 查询 fingerprint → active 返回 Duplicate / removed 重新激活 /
  新书解析 EPUB → 二次 SHA-256 校验（变化返回 `SourceChanged` 且不写记录）→ 生成
  UUID v4、UTC RFC3339 秒级时间、小写搜索键与 ICU 排序键 → 单事务写入 books。
- `storage-core` 增加类型化请求 `find_by_fingerprint`、`insert_new_book`（事务、
  UNIQUE 冲突返回 `Conflict`）、`reactivate_book`（返回 `Reactivated` /
  `AlreadyActive` / `NotFound`）、`soft_remove_book`；所有操作经专用写线程，
  `library-core` 不直接使用 `rusqlite`。
- `reactivate_book` 只更新 lifecycle、source_path 与 imported_at；`soft_remove_book`
  只把 lifecycle 改为 removed，均不触碰 reading_states/bookmarks/book_settings。
- 元数据回退：缺失 title → 文件 stem → “未命名书籍”；无作者 → “未知作者”；
  多作者按顺序用 `, ` 连接。
- 修正 `storage-core` crate description（移除 M0 边界描述）。
- 依赖：`uuid 1.29.2`（v4）、`time 0.3.55`（formatting）、`icu_collator 2.4.0`
  （默认 compiled_data）；均已写入 `Cargo.lock`。

本轮返工按第 13 节修复并发恢复与恢复前校验：

- `storage-core::reactivate_book` 返回类型化 `ReactivateOutcome`：成功恢复、已被并发
  激活（`AlreadyActive`）、记录不存在（`NotFound`）。
- `library-core` 将 `AlreadyActive` 映射为 `Duplicate` 并返回同一 book_id；
  `NotFound` 映射为稳定 `BookError::Internal`。
- removed 分支在写库前重新计算 SHA-256；与初始 fingerprint 不一致返回
  `SourceChanged`，数据库记录保持 removed。
- 新增并发恢复测试：两个线程同时导入同一 removed 书籍，结果为一个 `Reactivated`、
  一个 `Duplicate`，两者 book_id 相同，最终仅一条 active 记录。
- 新增恢复前文件变化测试：返回 `SourceChanged`，原记录仍为 removed。

**修改文件：**

- `Cargo.toml`、`Cargo.lock`（workspace 成员与 uuid/time/icu_collator 依赖）
- `crates/library-core/Cargo.toml`、`crates/library-core/src/lib.rs`、
  `crates/library-core/src/error.rs`、`crates/library-core/src/keys.rs`、
  `crates/library-core/src/service.rs`、`crates/library-core/tests/import.rs`（新增 crate）
- `crates/storage-core/Cargo.toml`（description）
- `crates/storage-core/src/error.rs`（新增 `Conflict`）
- `crates/storage-core/src/lib.rs`、`crates/storage-core/src/records.rs`（新增，
  含 `ReactivateOutcome`）、`crates/storage-core/src/books.rs`（新增）、
  `crates/storage-core/src/service.rs`
- `docs/tasks/TASK-M2B.md`（顶部状态与本第 12 节）

未修改 Tauri、UI、需求/设计文档或其他任务文件，未创建 commit。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p library-core` | 0 | 14 passed（lib 6、import 集成 8） |
| `cargo test -p storage-core` | 0 | 23 passed（lib 19、public_api 4） |
| `cargo test --workspace` | 0 | 165 passed，0 failed，1 ignored |
| `npm run check` | 0 | Biome 14 个文件无修复；Vitest 2/2 通过 |

**测试覆盖：**

- 6 个不同合成 EPUB 全部 Imported，且各自获得不同 UUID。
- 相同字节、不同文件名 → 第二次 Duplicate，book_id 与首次一致。
- 内容变化（章节正文不同）→ 新书 Imported，book_id 不同。
- 导入 → `soft_remove` → lifecycle removed → 重新导入 → Reactivated，book_id 不变，
  lifecycle 恢复 active。
- 两个线程同时导入同一 active 文件 → 一个 Imported、一个 Duplicate。
- 两个线程同时导入同一 removed 文件 → 一个 Reactivated、一个 Duplicate，book_id
  相同，最终仅一条 active 记录。
- removed 文件在恢复前被修改 → `SourceChanged`，记录保持 removed（单元测试）。
- 解析失败（缺 OPF）→ Failed，fingerprint 查询无记录；错误不包含路径。
- 批量混合成功/重复/失败 → 状态顺序与输入一致。
- 二次指纹变化（新书导入）→ `SourceChanged`，未写入记录。
- storage 层：UNIQUE 冲突无第二行；`reactivate_book` 对 active/缺失记录分别返回
  `AlreadyActive`/`NotFound`；soft_remove/reactivate 后 reading_states、bookmarks、
  book_settings 均保留。
- 排序键：中文/英文输入重复生成一致 BLOB，英文与中文键不同；时间为 20 字符
  `...Z` RFC3339。

**未验证项：**

- “Duplicate 不覆盖进度/书签”未做端到端验证：library-core 尚无写个人数据的公开接口；
  由 duplicate 分支只执行 SELECT（无写操作）与 storage 层“reactivate/soft_remove 保留
  个人数据”测试共同覆盖。
- 非 UTF-8 源路径（macOS 上难以构造）未实测 `FileUnreadable`。
- title/author 回退后的持久化值未通过公开查询验证（storage 无查询接口）；仅 keys 单元
  测试覆盖回退函数。
- ICU 排序键在不同 ICU 数据版本/平台上的稳定性未验证；当前锁定 2.4.0。
- 批量导入没有进度事件或取消能力（后续任务）。
- 未新增书库列表/搜索接口；M2C 负责。
- 封面缓存键保持 NULL（M4 前补任务）。

**风险与待决问题：**

- `storage-core` 新增 `StorageError::Conflict` 与 `ReactivateOutcome`；其他调用方需要
  处理。
- 并发恢复依赖 UPDATE 的 `lifecycle='removed'` 条件与二次查询；若记录在两次查询之间
  被删除（当前无删除接口），返回 `NotFound` 并映射为 `Internal`。
- removed 分支会在写库前再次读取文件计算 SHA-256，导入大文件时增加一次完整读取；
  与“解析前后指纹校验”一致，属于任务要求。
- `source_path` 使用 `canonicalize()`，会解析符号链接或相对路径，可能与用户输入文本
  不同。
- 时间精确到秒，同秒内多次导入共享相同 `imported_at`（排序稳定性由 ID/其他键保证）。
- ICU 排序键是二进制 BLOB，升级 `icu_collator` 后可能需要重建；锁文件固定版本。
- `LibraryService` 没有 shutdown 方法；调用方需保留 `StorageService` 句柄以关闭写线程。
- 导入在调用线程同步执行（哈希与解析阻塞）；Tauri 层必须调度到阻塞线程。

## 13. Codex 审查

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p library-core`：14 passed。
- `cargo test -p storage-core`：23 passed。
- `cargo test --workspace`：165 passed，1 ignored。
- `npm run check`：通过，Vitest 2/2。
- 依赖树仅包含已授权的 uuid、time、icu_collator、rusqlite 链。
- 未发现受保护文件被 DSH 修改，无新增 commit。

**问题：**

| 严重级别 | 文件/位置 | 问题 | 处理 |
|---|---|---|---|
| `MAJOR`（已修复） | `crates/library-core/src/service.rs` / removed 分支；`crates/storage-core/src/books.rs` / `reactivate_book` | 并发恢复时后一个操作可能返回 Failed。 | 已增加 `ReactivateOutcome::Reactivated/AlreadyActive/NotFound`；并发测试稳定得到一个 Reactivated、一个 Duplicate，并返回同一 book_id。 |
| `MAJOR`（已修复） | `crates/library-core/src/service.rs` / `reactivate` | removed 分支缺少恢复前二次指纹校验。 | 已重新计算 SHA-256；不一致返回 SourceChanged，测试确认记录保持 removed。 |

**返工范围：**

1. 为存储层 reactivate 操作定义类型化结果，区分成功恢复、已激活和记录不存在。
2. library-core 将“同时被另一导入激活”映射为 Duplicate，与相同 fingerprint 已存在规则一致。
3. reactivate 写库前二次计算 fingerprint；不一致返回 SourceChanged。
4. 增加两个并发线程恢复同一 removed 记录的测试：结果为
   `Reactivated + Duplicate`，同一 book_id，最终仅一条 active 记录。
5. 增加 removed 文件在恢复前变化返回 SourceChanged，记录仍为 removed 的测试。
6. 不修改 Tauri、UI、需求或设计文档，不创建 commit。

**M2B 结果：**

导入、判重、事务、排序键、软移除、并发恢复和恢复前源文件校验均满足任务要求，
M2B 验收通过。

### 复验记录

**复验日期：** 2026-11-14  
**结论：** 通过

- `cargo test -p library-core`：14 passed。
- `cargo test -p storage-core`：23 passed。
- `cargo test --workspace`：所有非 ignored 测试通过。
- 两个线程恢复同一 removed 记录：一个 Reactivated、一个 Duplicate，同一 book_id。
- removed 文件在恢复前变化：SourceChanged，记录保持 removed。
- `npm run check`：通过，Vitest 2/2。
- 没有新增依赖或 commit，未修改受保护文档。
