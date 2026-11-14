//! SQLite schema v1 与事务迁移。

use rusqlite::{Connection, ErrorCode};

use crate::StorageError;

/// 当前支持的 schema 版本。
pub(crate) const SCHEMA_VERSION: u32 = 1;

const SCHEMA_V1: &str = r#"
CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

CREATE TABLE books (
  id TEXT PRIMARY KEY,
  fingerprint TEXT NOT NULL UNIQUE,
  source_path TEXT NOT NULL,
  title TEXT NOT NULL,
  author TEXT NOT NULL,
  title_search TEXT NOT NULL,
  author_search TEXT NOT NULL,
  title_sort_key BLOB NOT NULL,
  author_sort_key BLOB NOT NULL,
  cover_cache_key TEXT,
  manifest_json TEXT NOT NULL,
  manifest_version INTEGER NOT NULL,
  lifecycle TEXT NOT NULL CHECK (lifecycle IN ('active', 'removed')),
  imported_at TEXT NOT NULL,
  last_opened_at TEXT
);

CREATE TABLE reading_states (
  book_id TEXT PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
  locator_json TEXT NOT NULL,
  locator_version INTEGER NOT NULL,
  global_progression REAL NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE book_settings (
  book_id TEXT PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
  mode TEXT,
  font_scale REAL,
  theme TEXT,
  updated_at TEXT NOT NULL
);

CREATE TABLE bookmarks (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  locator_json TEXT NOT NULL,
  locator_version INTEGER NOT NULL,
  excerpt TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_books_title_search ON books(title_search);
CREATE INDEX idx_books_author_search ON books(author_search);
CREATE INDEX idx_books_imported_at ON books(imported_at);
CREATE INDEX idx_bookmarks_book_created ON bookmarks(book_id, created_at);
"#;

/// 为每个新连接设置 PRAGMA。
pub(crate) fn configure(connection: &Connection) -> Result<(), StorageError> {
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|_| StorageError::OpenFailed)?;

    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(|_| StorageError::OpenFailed)?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(StorageError::OpenFailed);
    }
    Ok(())
}

/// 在事务中执行缺失的迁移；重复执行保持幂等。
pub(crate) fn migrate(connection: &mut Connection) -> Result<(), StorageError> {
    let current = current_version(connection)?;
    if current > SCHEMA_VERSION {
        return Err(StorageError::UnsupportedSchemaVersion);
    }
    if current == SCHEMA_VERSION {
        return Ok(());
    }

    let transaction = connection
        .transaction()
        .map_err(|error| map_sql_error(error, StorageError::TransactionFailed))?;
    transaction
        .execute_batch(SCHEMA_V1)
        .map_err(|error| map_sql_error(error, StorageError::MigrationFailed))?;
    transaction
        .execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, datetime('now'))",
            [i64::from(SCHEMA_VERSION)],
        )
        .map_err(|_| StorageError::MigrationFailed)?;
    transaction
        .commit()
        .map_err(|_| StorageError::MigrationFailed)?;
    Ok(())
}

/// 读取当前 schema 版本；没有 `schema_migrations` 表时为 0。
pub(crate) fn current_version(connection: &Connection) -> Result<u32, StorageError> {
    let has_table: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |row| row.get(0),
        )
        .map_err(|_| StorageError::QueryFailed)?;
    if has_table == 0 {
        return Ok(0);
    }

    let version: Option<i64> = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(|_| StorageError::QueryFailed)?;
    match version {
        None => Ok(0),
        Some(value) if value >= 0 && value <= i64::from(u32::MAX) => Ok(value as u32),
        Some(_) => Err(StorageError::UnsupportedSchemaVersion),
    }
}

fn map_sql_error(error: rusqlite::Error, fallback: StorageError) -> StorageError {
    if let rusqlite::Error::SqliteFailure(code, _) = error
        && code.code == ErrorCode::DatabaseBusy
    {
        return StorageError::Busy;
    }
    fallback
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn temp_db(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "qurner-storage-schema-{}-{name}",
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

    fn open_migrated(path: &Path) -> Connection {
        let mut connection = Connection::open(path).expect("应能打开数据库");
        configure(&connection).expect("应能设置 PRAGMA");
        migrate(&mut connection).expect("应能迁移");
        connection
    }

    #[test]
    fn creates_all_tables_and_indexes() {
        let path = temp_db("tables");
        let connection = open_migrated(&path);

        let tables: Vec<String> = {
            let mut statement = connection
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .expect("prepare");
            statement
                .query_map([], |row| row.get(0))
                .expect("query")
                .collect::<Result<_, _>>()
                .expect("collect")
        };
        assert_eq!(
            tables,
            vec![
                "app_settings",
                "book_settings",
                "bookmarks",
                "books",
                "reading_states",
                "schema_migrations",
            ]
        );

        let indexes: Vec<String> = {
            let mut statement = connection
                .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx_%' ORDER BY name")
                .expect("prepare");
            statement
                .query_map([], |row| row.get(0))
                .expect("query")
                .collect::<Result<_, _>>()
                .expect("collect")
        };
        assert_eq!(
            indexes,
            vec![
                "idx_bookmarks_book_created",
                "idx_books_author_search",
                "idx_books_imported_at",
                "idx_books_title_search",
            ]
        );

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn configures_pragmas() {
        let path = temp_db("pragmas");
        let connection = open_migrated(&path);

        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign_keys");
        assert_eq!(foreign_keys, 1);

        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal_mode");
        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");

        let synchronous: i64 = connection
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .expect("synchronous");
        assert_eq!(synchronous, 1);

        let busy_timeout: i64 = connection
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .expect("busy_timeout");
        assert_eq!(busy_timeout, 5000);

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn migration_is_idempotent_and_reports_version_one() {
        let path = temp_db("idempotent");
        let mut connection = Connection::open(&path).expect("open");
        configure(&connection).expect("configure");

        migrate(&mut connection).expect("first migration");
        migrate(&mut connection).expect("second migration");
        migrate(&mut connection).expect("third migration");

        assert_eq!(current_version(&connection).expect("version"), 1);
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("count");
        assert_eq!(rows, 1);

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn rejects_unknown_newer_schema_version() {
        let path = temp_db("newer");
        let connection = open_migrated(&path);
        connection
            .execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (99, '2026-11-14T00:00:00Z')",
                [],
            )
            .expect("insert");

        let mut connection = connection;
        assert_eq!(
            migrate(&mut connection).expect_err("更高版本应拒绝"),
            StorageError::UnsupportedSchemaVersion
        );

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn enforces_foreign_keys_and_cascade() {
        let path = temp_db("foreign-keys");
        let connection = open_migrated(&path);

        let missing_parent = connection.execute(
            "INSERT INTO reading_states(book_id, locator_json, locator_version, global_progression, updated_at)
             VALUES ('missing', '{}', 1, 0.0, '2026-11-14T00:00:00Z')",
            [],
        );
        assert!(missing_parent.is_err(), "缺少父记录应失败");

        insert_book(&connection, "book-1", "fingerprint-1", "active");
        connection
            .execute(
                "INSERT INTO bookmarks(id, book_id, locator_json, locator_version, created_at)
                 VALUES ('mark-1', 'book-1', '{}', 1, '2026-11-14T00:00:00Z')",
                [],
            )
            .expect("insert bookmark");

        connection
            .execute("DELETE FROM books WHERE id = 'book-1'", [])
            .expect("delete book");
        let remaining: i64 = connection
            .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))
            .expect("count");
        assert_eq!(remaining, 0, "级联删除应生效");

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn rejects_invalid_lifecycle() {
        let path = temp_db("lifecycle");
        let connection = open_migrated(&path);
        let result = connection.execute(
            "INSERT INTO books(id, fingerprint, source_path, title, author, title_search, author_search,
                               title_sort_key, author_sort_key, manifest_json, manifest_version,
                               lifecycle, imported_at)
             VALUES ('book-1', 'fingerprint-1', '/tmp/a.epub', 'Title', 'Author', 'title', 'author',
                     x'00', x'00', '{}', 1, 'broken', '2026-11-14T00:00:00Z')",
            [],
        );
        assert!(result.is_err(), "非法 lifecycle 应被拒绝");

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn enforces_unique_fingerprint() {
        let path = temp_db("fingerprint");
        let connection = open_migrated(&path);

        insert_book(&connection, "book-1", "same-fingerprint", "active");
        let duplicate = connection.execute(
            "INSERT INTO books(id, fingerprint, source_path, title, author, title_search, author_search,
                               title_sort_key, author_sort_key, manifest_json, manifest_version,
                               lifecycle, imported_at)
             VALUES ('book-2', 'same-fingerprint', '/tmp/b.epub', 'Title', 'Author', 'title', 'author',
                     x'00', x'00', '{}', 1, 'active', '2026-11-14T00:00:00Z')",
            [],
        );
        assert!(duplicate.is_err(), "重复 fingerprint 应被拒绝");

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn failed_migration_rolls_back_without_partial_schema() {
        let path = temp_db("rollback");
        let mut connection = Connection::open(&path).expect("open");
        configure(&connection).expect("configure");

        // 预先创建一个结构不兼容的 books 表，使 v1 迁移在中途失败。
        connection
            .execute("CREATE TABLE books (id TEXT PRIMARY KEY)", [])
            .expect("create conflicting table");

        assert_eq!(
            migrate(&mut connection).expect_err("迁移应失败"),
            StorageError::MigrationFailed
        );

        let has_migrations: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
                [],
                |row| row.get(0),
            )
            .expect("query");
        assert_eq!(has_migrations, 0, "迁移事务应回滚");

        drop(connection);
        cleanup(&path);
    }

    #[test]
    fn opens_path_with_chinese_characters_and_spaces() {
        let directory = std::env::temp_dir().join(format!(
            "qurner-storage-schema-{}-中文 路径",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("创建中文路径");
        let path = directory.join("阅读 数据.sqlite3");

        let connection = open_migrated(&path);
        assert_eq!(current_version(&connection).expect("version"), 1);

        drop(connection);
        cleanup(&path);
    }

    fn insert_book(connection: &Connection, id: &str, fingerprint: &str, lifecycle: &str) {
        connection
            .execute(
                "INSERT INTO books(id, fingerprint, source_path, title, author, title_search, author_search,
                                   title_sort_key, author_sort_key, manifest_json, manifest_version,
                                   lifecycle, imported_at)
                 VALUES (?1, ?2, '/tmp/a.epub', 'Title', 'Author', 'title', 'author',
                         x'00', x'00', '{}', 1, ?3, '2026-11-14T00:00:00Z')",
                rusqlite::params![id, fingerprint, lifecycle],
            )
            .expect("insert book");
    }
}
