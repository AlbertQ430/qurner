//! 书籍记录的持久化操作。
//!
//! 仅供写线程调用；外部 crate 通过 `StorageService` 使用。

use rusqlite::{Connection, ErrorCode, OptionalExtension, params};

use crate::StorageError;
use reader_model::{BookSort, BookSummary, ListBooksQuery, SortDirection};

use crate::records::{ExistingBookRecord, NewBookRecord, ReactivateOutcome};

/// 按指纹查询书籍摘要。
pub(crate) fn find_by_fingerprint(
    connection: &Connection,
    fingerprint: &str,
) -> Result<Option<ExistingBookRecord>, StorageError> {
    let mut statement = connection
        .prepare("SELECT id, source_path, lifecycle FROM books WHERE fingerprint = ?1")
        .map_err(|_| StorageError::QueryFailed)?;
    let mut rows = statement
        .query([fingerprint])
        .map_err(|_| StorageError::QueryFailed)?;

    let Some(row) = rows.next().map_err(|_| StorageError::QueryFailed)? else {
        return Ok(None);
    };
    Ok(Some(ExistingBookRecord {
        id: row.get(0).map_err(|_| StorageError::QueryFailed)?,
        source_path: row.get(1).map_err(|_| StorageError::QueryFailed)?,
        lifecycle: row.get(2).map_err(|_| StorageError::QueryFailed)?,
    }))
}

/// 在事务中插入一本新书；UNIQUE 冲突映射为 `Conflict` 且不留下部分写入。
pub(crate) fn insert_new_book(
    connection: &mut Connection,
    record: NewBookRecord,
) -> Result<(), StorageError> {
    let transaction = connection.transaction().map_err(map_transaction_error)?;
    transaction
        .execute(
            "INSERT INTO books(
                id, fingerprint, source_path, title, author,
                title_search, author_search, title_sort_key, author_sort_key,
                cover_cache_key, manifest_json, manifest_version, lifecycle, imported_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'active', ?13
             )",
            params![
                record.id,
                record.fingerprint,
                record.source_path,
                record.title,
                record.author,
                record.title_search,
                record.author_search,
                record.title_sort_key,
                record.author_sort_key,
                record.cover_cache_key,
                record.manifest_json,
                record.manifest_version,
                record.imported_at,
            ],
        )
        .map_err(map_insert_error)?;
    transaction.commit().map_err(map_transaction_error)?;
    Ok(())
}

/// 仅更新 lifecycle、source_path 与 imported_at；不触碰个人阅读数据。
pub(crate) fn reactivate_book(
    connection: &Connection,
    id: &str,
    source_path: &str,
    imported_at: &str,
) -> Result<ReactivateOutcome, StorageError> {
    let lifecycle = current_lifecycle(connection, id)?;
    match lifecycle.as_deref() {
        None => Ok(ReactivateOutcome::NotFound),
        Some("active") => Ok(ReactivateOutcome::AlreadyActive),
        Some(_) => {
            let updated = connection
                .execute(
                    "UPDATE books
                     SET lifecycle = 'active', source_path = ?1, imported_at = ?2
                     WHERE id = ?3 AND lifecycle = 'removed'",
                    params![source_path, imported_at, id],
                )
                .map_err(map_query_error)?;
            if updated == 1 {
                return Ok(ReactivateOutcome::Reactivated);
            }

            // 并发恢复：另一个导入可能刚刚激活了记录。
            match current_lifecycle(connection, id)?.as_deref() {
                Some("active") => Ok(ReactivateOutcome::AlreadyActive),
                Some(_) => Ok(ReactivateOutcome::NotFound),
                None => Ok(ReactivateOutcome::NotFound),
            }
        }
    }
}

fn current_lifecycle(connection: &Connection, id: &str) -> Result<Option<String>, StorageError> {
    connection
        .query_row("SELECT lifecycle FROM books WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(map_query_error)
}

/// 软移除：只把 lifecycle 改为 removed，不删除关联数据。
pub(crate) fn soft_remove_book(connection: &mut Connection, id: &str) -> Result<(), StorageError> {
    let transaction = connection.transaction().map_err(map_transaction_error)?;
    let updated = transaction
        .execute(
            "UPDATE books SET lifecycle = 'removed' WHERE id = ?1 AND lifecycle = 'active'",
            [id],
        )
        .map_err(map_query_error)?;
    if updated == 0 {
        return Err(StorageError::QueryFailed);
    }
    transaction.commit().map_err(map_transaction_error)?;
    Ok(())
}

/// 参数化书库列表查询：只返回 active，按预计算排序键排序并用 id 稳定收尾。
pub(crate) fn list_books(
    connection: &Connection,
    query: &ListBooksQuery,
) -> Result<Vec<BookSummary>, StorageError> {
    let pattern = query
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{}%", escape_like(&value.to_lowercase())));

    let sql = list_books_sql(query.sort, query.direction);
    let mut statement = connection.prepare(&sql).map_err(map_query_error)?;
    let rows = statement
        .query_map([pattern], |row| {
            Ok(BookSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                author: row.get(2)?,
                cover_cache_key: row.get(3)?,
                imported_at: row.get(4)?,
                last_opened_at: row.get(5)?,
            })
        })
        .map_err(map_query_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(map_query_error)
}

fn list_books_sql(sort: BookSort, direction: SortDirection) -> String {
    format!(
        "SELECT id, title, author, cover_cache_key, imported_at, last_opened_at
         FROM books
         WHERE lifecycle = 'active'
           AND (?1 IS NULL
                OR title_search LIKE ?1 ESCAPE '\\'
                OR author_search LIKE ?1 ESCAPE '\\')
         ORDER BY {}",
        order_clause(sort, direction)
    )
}

fn order_clause(sort: BookSort, direction: SortDirection) -> &'static str {
    match (sort, direction) {
        (BookSort::Title, SortDirection::Asc) => "title_sort_key ASC, id ASC",
        (BookSort::Title, SortDirection::Desc) => "title_sort_key DESC, id ASC",
        (BookSort::Author, SortDirection::Asc) => "author_sort_key ASC, id ASC",
        (BookSort::Author, SortDirection::Desc) => "author_sort_key DESC, id ASC",
        (BookSort::ImportedAt, SortDirection::Asc) => "imported_at ASC, id ASC",
        (BookSort::ImportedAt, SortDirection::Desc) => "imported_at DESC, id ASC",
        // 共享枚举为 non_exhaustive；未知变体回退到确定的标题升序。
        _ => "title_sort_key ASC, id ASC",
    }
}

fn escape_like(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn map_insert_error(error: rusqlite::Error) -> StorageError {
    match error {
        rusqlite::Error::SqliteFailure(code, _) if code.code == ErrorCode::ConstraintViolation => {
            StorageError::Conflict
        }
        rusqlite::Error::SqliteFailure(code, _) if code.code == ErrorCode::DatabaseBusy => {
            StorageError::Busy
        }
        _ => StorageError::QueryFailed,
    }
}

fn map_query_error(error: rusqlite::Error) -> StorageError {
    if let rusqlite::Error::SqliteFailure(code, _) = error
        && code.code == ErrorCode::DatabaseBusy
    {
        return StorageError::Busy;
    }
    StorageError::QueryFailed
}

fn map_transaction_error(error: rusqlite::Error) -> StorageError {
    if let rusqlite::Error::SqliteFailure(code, _) = error
        && code.code == ErrorCode::DatabaseBusy
    {
        return StorageError::Busy;
    }
    StorageError::TransactionFailed
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::schema;

    fn temp_db(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "qurner-storage-books-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("应能创建测试目录");
        directory.join("reader.sqlite3")
    }

    fn cleanup(path: &Path) {
        if let Some(directory) = path.parent() {
            let _ = std::fs::remove_dir_all(directory);
        }
    }

    fn open(path: &Path) -> Connection {
        let mut connection = Connection::open(path).expect("open");
        schema::configure(&connection).expect("configure");
        schema::migrate(&mut connection).expect("migrate");
        connection
    }

    fn record(id: &str, fingerprint: &str) -> NewBookRecord {
        NewBookRecord {
            cover_cache_key: None,
            id: id.to_string(),
            fingerprint: fingerprint.to_string(),
            source_path: "/tmp/book.epub".to_string(),
            title: "Title".to_string(),
            author: "Author".to_string(),
            title_search: "title".to_string(),
            author_search: "author".to_string(),
            title_sort_key: vec![1, 2, 3],
            author_sort_key: vec![4, 5, 6],
            manifest_json: "{}".to_string(),
            manifest_version: 1,
            imported_at: "2026-11-14T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn inserts_finds_reactivates_and_soft_removes() {
        let path = temp_db("roundtrip");
        let mut connection = open(&path);

        insert_new_book(&mut connection, record("book-1", "fp-1")).expect("insert");
        let found = find_by_fingerprint(&connection, "fp-1")
            .expect("find")
            .expect("exists");
        assert_eq!(found.id, "book-1");
        assert_eq!(found.lifecycle, "active");

        soft_remove_book(&mut connection, "book-1").expect("remove");
        assert_eq!(
            find_by_fingerprint(&connection, "fp-1")
                .expect("find")
                .expect("exists")
                .lifecycle,
            "removed"
        );

        assert_eq!(
            reactivate_book(
                &connection,
                "book-1",
                "/tmp/moved.epub",
                "2026-11-14T01:00:00Z",
            )
            .expect("reactivate"),
            ReactivateOutcome::Reactivated
        );
        let found = find_by_fingerprint(&connection, "fp-1")
            .expect("find")
            .expect("exists");
        assert_eq!(found.lifecycle, "active");
        assert_eq!(found.source_path, "/tmp/moved.epub");

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn duplicate_insert_returns_conflict_without_partial_write() {
        let path = temp_db("conflict");
        let mut connection = open(&path);

        insert_new_book(&mut connection, record("book-1", "fp-1")).expect("insert");
        assert_eq!(
            insert_new_book(&mut connection, record("book-2", "fp-1")).expect_err("重复指纹应冲突"),
            StorageError::Conflict
        );

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM books", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 1, "冲突不应留下第二行");

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn reactivate_reports_already_active_and_not_found() {
        let path = temp_db("reactivate-outcomes");
        let mut connection = open(&path);
        insert_new_book(&mut connection, record("book-1", "fp-1")).expect("insert");

        assert_eq!(
            reactivate_book(&connection, "book-1", "/tmp/a.epub", "2026-11-14T01:00:00Z")
                .expect("already active"),
            ReactivateOutcome::AlreadyActive
        );
        assert_eq!(
            reactivate_book(
                &connection,
                "missing",
                "/tmp/a.epub",
                "2026-11-14T01:00:00Z"
            )
            .expect("missing"),
            ReactivateOutcome::NotFound
        );

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn soft_remove_and_reactivate_preserve_personal_data() {
        let path = temp_db("personal");
        let mut connection = open(&path);
        insert_new_book(&mut connection, record("book-1", "fp-1")).expect("insert");

        connection
            .execute(
                "INSERT INTO reading_states(book_id, locator_json, locator_version, global_progression, updated_at)
                 VALUES ('book-1', '{\"chapter\":1}', 1, 0.25, '2026-11-14T00:00:00Z')",
                [],
            )
            .expect("reading state");
        connection
            .execute(
                "INSERT INTO bookmarks(id, book_id, locator_json, locator_version, created_at)
                 VALUES ('mark-1', 'book-1', '{}', 1, '2026-11-14T00:00:00Z')",
                [],
            )
            .expect("bookmark");
        connection
            .execute(
                "INSERT INTO book_settings(book_id, mode, font_scale, updated_at)
                 VALUES ('book-1', 'paged', 1.0, '2026-11-14T00:00:00Z')",
                [],
            )
            .expect("settings");

        soft_remove_book(&mut connection, "book-1").expect("remove");
        assert_eq!(
            reactivate_book(
                &connection,
                "book-1",
                "/tmp/book.epub",
                "2026-11-14T01:00:00Z",
            )
            .expect("reactivate"),
            ReactivateOutcome::Reactivated
        );

        let states: i64 = connection
            .query_row("SELECT COUNT(*) FROM reading_states", [], |row| row.get(0))
            .expect("states");
        let bookmarks: i64 = connection
            .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))
            .expect("bookmarks");
        let settings: i64 = connection
            .query_row("SELECT COUNT(*) FROM book_settings", [], |row| row.get(0))
            .expect("settings");
        assert_eq!((states, bookmarks, settings), (1, 1, 1));

        drop(connection);
        cleanup(&path);
    }

    fn insert_list_book(
        connection: &mut Connection,
        id: &str,
        title: &str,
        author: &str,
        imported_at: &str,
        title_key: u32,
        author_key: u32,
    ) {
        let record = NewBookRecord {
            cover_cache_key: None,
            id: id.to_string(),
            fingerprint: format!("fp-{id}"),
            source_path: format!("/tmp/{id}.epub"),
            title: title.to_string(),
            author: author.to_string(),
            title_search: title.to_lowercase(),
            author_search: author.to_lowercase(),
            title_sort_key: title_key.to_be_bytes().to_vec(),
            author_sort_key: author_key.to_be_bytes().to_vec(),
            manifest_json: "{}".to_string(),
            manifest_version: 1,
            imported_at: imported_at.to_string(),
        };
        insert_new_book(connection, record).expect("insert list book");
    }

    fn ids(connection: &Connection, sort: BookSort, direction: SortDirection) -> Vec<String> {
        list_books(
            connection,
            &ListBooksQuery {
                query: None,
                sort,
                direction,
            },
        )
        .expect("list")
        .into_iter()
        .map(|summary| summary.id)
        .collect()
    }

    fn search(connection: &Connection, text: &str) -> Vec<String> {
        list_books(
            connection,
            &ListBooksQuery {
                query: Some(text.to_string()),
                sort: BookSort::Title,
                direction: SortDirection::Asc,
            },
        )
        .expect("search")
        .into_iter()
        .map(|summary| summary.id)
        .collect()
    }

    #[test]
    fn sorts_by_title_author_and_imported_at_both_directions() {
        let path = temp_db("sort");
        let mut connection = open(&path);
        insert_list_book(
            &mut connection,
            "a",
            "Alpha",
            "Zed",
            "2026-01-02T00:00:00Z",
            2,
            2,
        );
        insert_list_book(
            &mut connection,
            "b",
            "Beta",
            "Amy",
            "2026-01-01T00:00:00Z",
            1,
            3,
        );
        insert_list_book(
            &mut connection,
            "c",
            "Gamma",
            "Bob",
            "2026-01-03T00:00:00Z",
            3,
            1,
        );

        assert_eq!(
            ids(&connection, BookSort::Title, SortDirection::Asc),
            vec!["b", "a", "c"]
        );
        assert_eq!(
            ids(&connection, BookSort::Title, SortDirection::Desc),
            vec!["c", "a", "b"]
        );
        assert_eq!(
            ids(&connection, BookSort::Author, SortDirection::Asc),
            vec!["c", "a", "b"]
        );
        assert_eq!(
            ids(&connection, BookSort::Author, SortDirection::Desc),
            vec!["b", "a", "c"]
        );
        assert_eq!(
            ids(&connection, BookSort::ImportedAt, SortDirection::Asc),
            vec!["b", "a", "c"]
        );
        assert_eq!(
            ids(&connection, BookSort::ImportedAt, SortDirection::Desc),
            vec!["c", "a", "b"]
        );

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn searches_title_and_author_case_insensitively() {
        let path = temp_db("search");
        let mut connection = open(&path);
        insert_list_book(
            &mut connection,
            "a",
            "Rust Book",
            "Alice",
            "2026-01-01T00:00:00Z",
            1,
            1,
        );
        insert_list_book(
            &mut connection,
            "b",
            "中文书名",
            "张三",
            "2026-01-02T00:00:00Z",
            2,
            2,
        );
        insert_list_book(
            &mut connection,
            "c",
            "Other",
            "Bob",
            "2026-01-03T00:00:00Z",
            3,
            3,
        );

        assert_eq!(search(&connection, "rust"), vec!["a"]);
        assert_eq!(search(&connection, "RUST"), vec!["a"]);
        assert_eq!(search(&connection, "中文"), vec!["b"]);
        assert_eq!(search(&connection, "ali"), vec!["a"]);
        assert_eq!(search(&connection, "张三"), vec!["b"]);
        assert_eq!(search(&connection, "missing"), Vec::<String>::new());
        assert_eq!(search(&connection, "   "), vec!["a", "b", "c"]);
        assert_eq!(search(&connection, "needle"), Vec::<String>::new());

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn treats_like_wildcards_and_backslashes_as_literals() {
        let path = temp_db("escape");
        let mut connection = open(&path);
        insert_list_book(
            &mut connection,
            "a",
            "a_b",
            "作者",
            "2026-01-01T00:00:00Z",
            1,
            1,
        );
        insert_list_book(
            &mut connection,
            "b",
            "axb",
            "作者",
            "2026-01-02T00:00:00Z",
            2,
            2,
        );
        insert_list_book(
            &mut connection,
            "c",
            "50%",
            "作者",
            "2026-01-03T00:00:00Z",
            3,
            3,
        );
        insert_list_book(
            &mut connection,
            "d",
            "back\\slash",
            "作者",
            "2026-01-04T00:00:00Z",
            4,
            4,
        );

        assert_eq!(search(&connection, "a_b"), vec!["a"]);
        assert_eq!(search(&connection, "50%"), vec!["c"]);
        assert_eq!(search(&connection, "back\\slash"), vec!["d"]);
        assert_eq!(search(&connection, "_"), vec!["a"]);
        assert_eq!(search(&connection, "%"), vec!["c"]);

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn sql_injection_text_is_treated_as_search_text() {
        let path = temp_db("injection");
        let mut connection = open(&path);
        insert_list_book(
            &mut connection,
            "a",
            "Rust",
            "Alice",
            "2026-01-01T00:00:00Z",
            1,
            1,
        );

        assert_eq!(
            search(&connection, "'; DROP TABLE books; --"),
            Vec::<String>::new()
        );
        assert_eq!(
            ids(&connection, BookSort::Title, SortDirection::Asc),
            vec!["a"]
        );

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn removed_books_are_excluded() {
        let path = temp_db("removed-list");
        let mut connection = open(&path);
        insert_list_book(
            &mut connection,
            "a",
            "Keep",
            "作者",
            "2026-01-01T00:00:00Z",
            1,
            1,
        );
        insert_list_book(
            &mut connection,
            "b",
            "Remove",
            "作者",
            "2026-01-02T00:00:00Z",
            2,
            2,
        );

        soft_remove_book(&mut connection, "b").expect("soft remove");
        assert_eq!(
            ids(&connection, BookSort::Title, SortDirection::Asc),
            vec!["a"]
        );
        assert_eq!(search(&connection, "remove"), Vec::<String>::new());

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn equal_primary_keys_are_ordered_by_id_ascending() {
        let path = temp_db("tie-break");
        let mut connection = open(&path);
        insert_list_book(
            &mut connection,
            "b",
            "Same",
            "作者",
            "2026-01-01T00:00:00Z",
            1,
            1,
        );
        insert_list_book(
            &mut connection,
            "a",
            "Same",
            "作者",
            "2026-01-01T00:00:00Z",
            1,
            1,
        );

        assert_eq!(
            ids(&connection, BookSort::Title, SortDirection::Asc),
            vec!["a", "b"]
        );
        assert_eq!(
            ids(&connection, BookSort::Title, SortDirection::Desc),
            vec!["a", "b"]
        );

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn records_query_plan_for_list_query() {
        let path = temp_db("query-plan");
        let connection = open(&path);
        let sql = format!(
            "EXPLAIN QUERY PLAN {}",
            list_books_sql(BookSort::Title, SortDirection::Asc)
        );
        let mut statement = connection.prepare(&sql).expect("prepare");
        let plan: Vec<String> = statement
            .query_map([Some("%example%")], |row| row.get(3))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("collect");
        eprintln!("QUERY PLAN: {plan:?}");
        assert!(!plan.is_empty());

        cleanup(&path);
    }
}
