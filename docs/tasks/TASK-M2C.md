# TASK-M2C：书库查询与 1,000 本性能

**状态：** accepted  
**里程碑：** M2  
**负责人：** DeepSeek harness  
**创建人：** Codex  
**创建日期：** 2026-11-14  
**依赖：** TASK-M2B accepted

## 1. 目标

在 `library-core` 和 `storage-core` 实现书库列表、Unicode 搜索、稳定排序和 1,000 本
性能验证。

## 2. 查询接口

```rust
pub fn list_books(query: ListBooksQuery) -> Result<Vec<BookSummary>, BookError>;
```

`ListBooksQuery` 包含：

- `query: Option<String>`
- `sort: BookSort`（title/author/importedAt）
- `direction: asc/desc`

只返回 lifecycle=active 的记录。

在 `reader-model` 增加共享 DTO，全部使用 camelCase serde：

```rust
pub enum BookSort {
    Title,
    Author,
    ImportedAt,
}

pub enum SortDirection {
    Asc,
    Desc,
}

pub struct ListBooksQuery {
    pub query: Option<String>,
    pub sort: BookSort,
    pub direction: SortDirection,
}

pub struct BookSummary {
    pub id: String,
    pub title: String,
    pub author: String,
    pub cover_cache_key: Option<String>,
    pub imported_at: String,
    pub last_opened_at: Option<String>,
}
```

`coverUrl` 的协议 URL 由 M2D Tauri 层生成，本任务只返回内部缓存键。

## 3. 搜索与排序

- 搜索使用 Unicode 小写后的书名/作者包含匹配。
- 查询先 trim；空查询返回全部。
- `%` 和 `_` 按用户可见字符匹配，不得作为 SQL 通配符；使用参数化 `LIKE` 和显式
  `ESCAPE '\'`。
- 中文、英文、数字和特殊字符不得导致 SQL 注入。
- 排序键在写入时预计算，不在每次查询时重算。
- 主排序只使用对应列：title_sort_key、author_sort_key 或 imported_at。
- 主排序相同时固定以 id ASC 作为最终稳定次序；direction 只影响主排序。
- sort 枚举通过受控 match 映射到固定 ORDER BY 子句，绝不能拼接用户输入。
- 只返回 active；removed 不参与搜索和排序。

## 4. 性能

参考设备 Apple M5 / 16 GB：

- 1,000 条 active 记录。
- 搜索/排序命令 p95 ≤ 200 ms，重复 10 次。
- 返回 1,000 条摘要时内存可控，不读取原始 EPUB。
- 在基准报告记录 `EXPLAIN QUERY PLAN`；包含搜索的 LIKE 扫描可接受，但不得出现
  非预期的 N+1 查询或逐行读取磁盘文件。

## 5. 测试

- title/author/importedAt 三排序 ×升降序。
- 大小写不敏感英文、中文部分匹配、无结果。
- 查询只匹配 title/author，不匹配路径或 manifest。
- query 中的 `%`、`_`、反斜杠作为普通字符匹配。
- SQL 注入字符串仅作为搜索文本处理。
- 软移除记录不出现。
- 同名记录次序稳定。
- 1,000 条性能测试与查询计划检查。
- `BookSummary` camelCase JSON round-trip。
- 参数化 SQL，无拼接用户输入。

## 6. 文件权限

### 允许修改

- `crates/library-core/`
- `crates/storage-core/`
- `crates/reader-model/`
- 测试与 Cargo 锁文件
- 本任务状态和报告

### 禁止修改

- `src/`
- `src-tauri/`
- `crates/epub-core/`
- 需求、设计和协作文档
- 其他任务文件

## 7. 必跑命令

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p reader-model
cargo test -p storage-core
cargo test -p library-core
cargo test --workspace
npm run check
```

## 8. 开发报告

**完成摘要：**

在 `reader-model` 增加共享 DTO，`storage-core` 实现参数化列表查询，`library-core`
提供 `list_books` 入口：

- `reader-model::library` 新增 `BookSort`（title/author/importedAt）、`SortDirection`
  （asc/desc）、`ListBooksQuery`、`BookSummary`，全部 camelCase serde。
- `storage-core::StorageService::list_books` 经专用写线程执行参数化 SQL：
  - 只返回 `lifecycle = 'active'`；
  - 搜索使用 `title_search` / `author_search` 的 `LIKE ?1 ESCAPE '\'`，`None` 或空白
    查询不过滤；
  - `%`、`_`、`\` 在进入 LIKE 前显式转义，作为普通字符匹配；
  - ORDER BY 由受控 match 映射到六条固定子句（三字段 × 升降序），主排序键分别为
    `title_sort_key` / `author_sort_key` / `imported_at`，末尾固定 `id ASC`；
    direction 只影响主排序。
- `library-core::LibraryService::list_books(ListBooksQuery) -> Result<Vec<BookSummary>,
  BookError>` 仅做错误映射，不直接使用 SQL。
- 查询不读取原始 EPUB、不访问路径字段；`BookSummary` 只包含 id/title/author/
  cover_cache_key/imported_at/last_opened_at。

**修改文件：**

- `crates/reader-model/src/library.rs`（新增）、`crates/reader-model/src/lib.rs`
- `crates/storage-core/Cargo.toml`（reader-model 依赖）
- `crates/storage-core/src/books.rs`（列表查询与测试）
- `crates/storage-core/src/service.rs`（`ListBooks` 请求与方法）
- `crates/library-core/src/service.rs`（`list_books`）
- `crates/library-core/tests/list.rs`（新增）
- `docs/tasks/TASK-M2C.md`（顶部状态与本第 8 节）

未修改 `src/`、`src-tauri/`、`crates/epub-core/`、需求/设计/协作文档或其他任务文件，
未创建 commit，未新增依赖（storage-core 仅新增 workspace 内的 reader-model 依赖）。

**查询与排序：**

- 三排序 × 升降序：title/author/importedAt 各 2 个方向顺序断言通过；同名同键记录在
  ASC/DESC 下均按 id 升序稳定排列。
- 搜索：大小写不敏感英文（`rust`/`RUST`）、中文部分匹配（`中文`、`张三`）、无结果、
  空白查询返回全部；仅匹配 title/author，`source_path` 含 `needle` 不会命中。
- 通配符转义：`a_b` 只匹配含下划线的记录，`50%` 按字面匹配，`back\slash` 按反斜杠
  字面匹配。
- SQL 注入字符串 `'; DROP TABLE books; --` 仅作为搜索文本返回空结果，后续查询仍正常。
- `soft_remove_book` 后记录从列表与搜索中消失。
- `BookSummary` 与查询 DTO 的 camelCase JSON round-trip 通过（`coverCacheKey`、
  `importedAt`、`lastOpenedAt`）。
- SQL 使用 `?1` 参数绑定，ORDER BY 只来自受控 match 常量，未拼接用户输入。

**1,000 本性能：**

测试数据：直接通过 `StorageService::insert_new_book` 插入 1,000 条 active 记录（跳过
导入解析），查询条件为 `query=书名`、`sort=title`、`asc`，返回 1,000 条摘要；重复 10 次，
记录每次耗时（debug 构建测试）。

| 指标 | 实测 | 门槛 |
|---|---:|---:|
| p95 query（10 次运行的最大值） | 1.179 ms | ≤ 200 ms |
| 返回条数 | 1,000 | 1,000 |

`EXPLAIN QUERY PLAN`（title asc + LIKE 搜索）：

```text
SCAN books
USE TEMP B-TREE FOR ORDER BY
```

计划为单表扫描 + 临时排序，无 N+1 查询；LIKE 前缀通配无法使用索引，排序键当前无独立
索引，1,000 条规模下耗时可接受。查询只读数据库，不打开原始 EPUB。

**运行命令与结果：**

| 命令 | 退出码 | 结果摘要 |
|---|---:|---|
| `cargo fmt --all --check` | 0 | 无格式差异 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | 无 warning |
| `cargo test -p reader-model` | 0 | 9 passed |
| `cargo test -p storage-core` | 0 | 30 passed（lib 26、public_api 4） |
| `cargo test -p library-core` | 0 | 19 passed（lib 6、import 8、list 5） |
| `cargo test --workspace` | 0 | 180 passed，0 failed，1 ignored |
| `npm run check` | 0 | Biome 14 个文件无修复；Vitest 2/2 通过 |

**未验证项：**

- 性能测试运行在 debug 构建（测试二进制），p95 取 10 次运行的最大值；release 构建预期
  更快，但未单独测量。
- 1,000 条记录由测试直接插入，未走完整导入流程；真实数据库的排序键分布（ICU BLOB
  长度差异）未单独评估。
- 未测试 1,000 条以上（如 10,000 条）的列表查询；未新增排序键索引。
- 查询在唯一写线程串行执行；未测试列表查询与导入写并发时的延迟。
- `cover_cache_key` 与 `last_opened_at` 当前均为 NULL/None；非空映射未在列表结果中
  验证（字段读取代码已覆盖类型映射）。
- 未测试分页/虚拟列表；本任务一次返回全部匹配记录。
- 未测试 `last_opened_at` 排序（不在三排序范围内）。

**风险与待决问题：**

- 列表查询与写操作共用唯一写线程；大量查询会阻塞导入。M2D/M4 可考虑只读连接池或
  读写分离。
- LIKE `%...%` 全表扫描；数据量增长后需 FTS 或搜索键优化；排序依赖临时 B-tree。
- `BookSort`/`SortDirection` 为 `non_exhaustive`，ORDER BY match 对未知变体回退到
  title asc；新增枚举值时必须同步更新该 match。
- `BookSummary` 含书名/作者（可能敏感），但无路径与正文；日志/遥测不得记录这些字段。
- 搜索使用 `to_lowercase()` 后的预计算列；写入路径（M2B）已按相同规则生成搜索键，
  若后续修改大小写规则需迁移搜索键。
- UI 的 `coverUrl` 由 M2D 生成，本任务只返回内部缓存键。
- 分页与 1,000 条以上规模属于后续任务；当前一次返回 1,000 条摘要，内存可控。

## 9. Codex 审查

**结论：** 通过  
**审查日期：** 2026-11-14

**独立执行的检查：**

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p reader-model`：9 passed。
- `cargo test -p storage-core`：30 passed。
- `cargo test -p library-core`：19 passed。
- `cargo test --workspace`：180 passed，1 ignored。
- 1,000 本查询 p95 复测为 1.145 ms（10 次最大值），满足 ≤ 200 ms。
- LIKE 通配符转义、SQL 注入文本、active 过滤、六种排序/tie-break 全部通过。
- `EXPLAIN QUERY PLAN` 为 `SCAN books` + 临时 B-tree 排序，无 N+1。
- `npm run check`：通过，Vitest 2/2。
- 没有新增依赖或 commit，未修改受保护文档。

**结果：**

书库查询与 1,000 本性能满足任务要求，M2C 验收通过。
