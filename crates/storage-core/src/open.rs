//! 打开书籍所需的只读记录、设置读取与 last_opened 更新。

use rusqlite::{Connection, OptionalExtension};

use crate::StorageError;

/// active 书籍的打开记录。
#[derive(Debug, Clone, PartialEq)]
pub struct OpenBookRecord {
    pub id: String,
    pub fingerprint: String,
    pub source_path: String,
    pub title: String,
    pub manifest_json: String,
    pub manifest_version: u32,
    pub last_opened_at: Option<String>,
}

/// 已保存的阅读位置。
#[derive(Debug, Clone, PartialEq)]
pub struct ReadingStateRecord {
    pub locator_json: String,
    pub locator_version: u32,
    pub global_progression: f64,
}

/// 单书设置覆盖；`None` 表示继承。
#[derive(Debug, Clone, PartialEq)]
pub struct BookSettingsRecord {
    pub mode: Option<String>,
    pub font_scale: Option<f64>,
    pub theme: Option<String>,
}

/// 读取 active 书籍记录。
pub(crate) fn active_book(
    connection: &Connection,
    id: &str,
) -> Result<Option<OpenBookRecord>, StorageError> {
    connection
        .query_row(
            "SELECT id, fingerprint, source_path, title, manifest_json, manifest_version, last_opened_at
             FROM books
             WHERE id = ?1 AND lifecycle = 'active'",
            [id],
            |row| {
                let manifest_version: i64 = row.get(5)?;
                Ok(OpenBookRecord {
                    id: row.get(0)?,
                    fingerprint: row.get(1)?,
                    source_path: row.get(2)?,
                    title: row.get(3)?,
                    manifest_json: row.get(4)?,
                    manifest_version: u32::try_from(manifest_version)
                        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(5, manifest_version))?,
                    last_opened_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(map_query_error)
}

/// 读取已保存的阅读位置。
pub(crate) fn reading_state(
    connection: &Connection,
    book_id: &str,
) -> Result<Option<ReadingStateRecord>, StorageError> {
    connection
        .query_row(
            "SELECT locator_json, locator_version, global_progression
             FROM reading_states
             WHERE book_id = ?1",
            [book_id],
            |row| {
                let locator_version: i64 = row.get(1)?;
                Ok(ReadingStateRecord {
                    locator_json: row.get(0)?,
                    locator_version: u32::try_from(locator_version).map_err(|_| {
                        rusqlite::Error::IntegralValueOutOfRange(1, locator_version)
                    })?,
                    global_progression: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(map_query_error)
}

/// 读取单书设置覆盖。
pub(crate) fn book_settings(
    connection: &Connection,
    book_id: &str,
) -> Result<Option<BookSettingsRecord>, StorageError> {
    connection
        .query_row(
            "SELECT mode, font_scale, theme FROM book_settings WHERE book_id = ?1",
            [book_id],
            |row| {
                Ok(BookSettingsRecord {
                    mode: row.get(0)?,
                    font_scale: row.get(1)?,
                    theme: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(map_query_error)
}

/// 读取全局设置 JSON。
pub(crate) fn app_setting(
    connection: &Connection,
    key: &str,
) -> Result<Option<String>, StorageError> {
    connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_query_error)
}

/// 原子写入全局设置显式 patch：事务内校验 JSON 并逐字段 `json_set`。
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_app_settings(
    connection: &mut Connection,
    key: &str,
    mode: Option<&str>,
    font_scale: Option<f64>,
    theme: Option<&str>,
    timestamp: &str,
) -> Result<(), StorageError> {
    let transaction = connection.transaction().map_err(map_transaction_error)?;
    let existing: Option<String> = transaction
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_query_error)?;
    match existing {
        None => {
            transaction
                .execute(
                    "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, '{}', ?2)",
                    rusqlite::params![key, timestamp],
                )
                .map_err(map_query_error)?;
        }
        Some(json) => {
            let valid: i64 = transaction
                .query_row("SELECT json_valid(?1)", [&json], |row| row.get(0))
                .map_err(map_query_error)?;
            if valid == 0 || !app_json_is_semantically_valid(&transaction, &json)? {
                return Err(StorageError::QueryFailed);
            }
            transaction
                .execute(
                    "UPDATE app_settings SET updated_at = ?2 WHERE key = ?1",
                    rusqlite::params![key, timestamp],
                )
                .map_err(map_query_error)?;
        }
    }
    if let Some(mode) = mode {
        transaction
            .execute(
                "UPDATE app_settings SET value_json = json_set(value_json, '$.mode', ?2) WHERE key = ?1",
                rusqlite::params![key, mode],
            )
            .map_err(map_query_error)?;
    }
    if let Some(font_scale) = font_scale {
        transaction
            .execute(
                "UPDATE app_settings SET value_json = json_set(value_json, '$.fontScale', ?2) WHERE key = ?1",
                rusqlite::params![key, font_scale],
            )
            .map_err(map_query_error)?;
    }
    if let Some(theme) = theme {
        transaction
            .execute(
                "UPDATE app_settings SET value_json = json_set(value_json, '$.theme', ?2) WHERE key = ?1",
                rusqlite::params![key, theme],
            )
            .map_err(map_query_error)?;
    }
    transaction.commit().map_err(map_transaction_error)?;
    Ok(())
}

fn app_json_is_semantically_valid(
    transaction: &rusqlite::Transaction<'_>,
    json: &str,
) -> Result<bool, StorageError> {
    let valid: i64 = transaction
        .query_row(
            "SELECT CASE WHEN
               json_type(?1) = 'object'
               AND NOT EXISTS (
                 SELECT 1 FROM json_each(?1)
                 WHERE key NOT IN ('mode','fontScale','theme')
               )
               AND (
                 json_type(?1, '$.mode') IS NULL
                 OR (
                   json_type(?1, '$.mode') = 'text'
                   AND json_extract(?1, '$.mode') IN ('paged','scrolling')
                 )
               )
               AND (
                 json_type(?1, '$.fontScale') IS NULL
                 OR (
                   json_type(?1, '$.fontScale') IN ('real','integer')
                   AND json_extract(?1, '$.fontScale') BETWEEN 0.5 AND 3.0
                 )
               )
               AND (
                 json_type(?1, '$.theme') IS NULL
                 OR (
                   json_type(?1, '$.theme') = 'text'
                   AND json_extract(?1, '$.theme') IN ('day','night')
                 )
               )
             THEN 1 ELSE 0 END",
            [json],
            |row| row.get(0),
        )
        .map_err(map_query_error)?;
    Ok(valid == 1)
}

/// 原子写入单书字段 patch：`None` 保留、`Some(None)` 清除、`Some(Some)` 覆盖。
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_book_settings(
    connection: &mut Connection,
    book_id: &str,
    mode: Option<Option<&str>>,
    font_scale: Option<Option<f64>>,
    theme: Option<Option<&str>>,
    timestamp: &str,
) -> Result<(), StorageError> {
    let (mode_provided, mode_value) = match mode {
        Some(value) => (1_i64, value),
        None => (0_i64, None),
    };
    let (font_provided, font_value) = match font_scale {
        Some(value) => (1_i64, value),
        None => (0_i64, None),
    };
    let (theme_provided, theme_value) = match theme {
        Some(value) => (1_i64, value),
        None => (0_i64, None),
    };
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(map_transaction_error)?;
    let existing: Option<(Option<String>, Option<f64>, Option<String>)> = transaction
        .query_row(
            "SELECT mode, font_scale, theme FROM book_settings WHERE book_id = ?1",
            [book_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_query_error)?;
    if let Some((mode, font_scale, theme)) = existing {
        if let Some(mode) = mode
            && mode != "paged"
            && mode != "scrolling"
        {
            return Err(StorageError::QueryFailed);
        }
        if let Some(font_scale) = font_scale
            && (!font_scale.is_finite() || !(0.5..=3.0).contains(&font_scale))
        {
            return Err(StorageError::QueryFailed);
        }
        if let Some(theme) = theme
            && theme != "day"
            && theme != "night"
        {
            return Err(StorageError::QueryFailed);
        }
    }
    transaction
        .execute(
            "INSERT INTO book_settings (book_id, mode, font_scale, theme, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(book_id) DO UPDATE SET
               mode = CASE WHEN ?6 THEN excluded.mode ELSE book_settings.mode END,
               font_scale = CASE WHEN ?7 THEN excluded.font_scale ELSE book_settings.font_scale END,
               theme = CASE WHEN ?8 THEN excluded.theme ELSE book_settings.theme END,
               updated_at = excluded.updated_at",
            rusqlite::params![
                book_id,
                mode_value,
                font_value,
                theme_value,
                timestamp,
                mode_provided,
                font_provided,
                theme_provided
            ],
        )
        .map_err(map_query_error)?;
    transaction.commit().map_err(map_transaction_error)?;
    Ok(())
}

/// 保存阅读位置（单条 UPSERT，原子）。
pub(crate) fn save_reading_state(
    connection: &mut Connection,
    book_id: &str,
    locator_json: &str,
    locator_version: u32,
    global_progression: f64,
    timestamp: &str,
) -> Result<(), StorageError> {
    connection
        .execute(
            "INSERT INTO reading_states
               (book_id, locator_json, locator_version, global_progression, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(book_id) DO UPDATE SET
               locator_json = excluded.locator_json,
               locator_version = excluded.locator_version,
               global_progression = excluded.global_progression,
               updated_at = excluded.updated_at",
            rusqlite::params![
                book_id,
                locator_json,
                locator_version,
                global_progression,
                timestamp
            ],
        )
        .map_err(map_query_error)?;
    Ok(())
}

/// 删除单书设置行，恢复完全继承。
pub(crate) fn delete_book_settings(
    connection: &mut Connection,
    book_id: &str,
) -> Result<(), StorageError> {
    connection
        .execute("DELETE FROM book_settings WHERE book_id = ?1", [book_id])
        .map_err(map_query_error)?;
    Ok(())
}

/// 更新最近打开时间；active 记录不存在时返回查询错误。
pub(crate) fn mark_opened(
    connection: &mut Connection,
    book_id: &str,
    timestamp: &str,
) -> Result<(), StorageError> {
    let transaction = connection.transaction().map_err(map_transaction_error)?;
    let updated = transaction
        .execute(
            "UPDATE books SET last_opened_at = ?2 WHERE id = ?1 AND lifecycle = 'active'",
            [book_id, timestamp],
        )
        .map_err(map_query_error)?;
    if updated == 0 {
        return Err(StorageError::QueryFailed);
    }
    transaction.commit().map_err(map_transaction_error)?;
    Ok(())
}

fn map_query_error(_error: rusqlite::Error) -> StorageError {
    StorageError::QueryFailed
}

fn map_transaction_error(_error: rusqlite::Error) -> StorageError {
    StorageError::TransactionFailed
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    fn setup() -> Connection {
        let connection = Connection::open_in_memory().expect("memory");
        connection
            .execute_batch(
                "CREATE TABLE books (
                    id TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, source_path TEXT NOT NULL,
                    title TEXT NOT NULL, manifest_json TEXT NOT NULL, manifest_version INTEGER NOT NULL,
                    lifecycle TEXT NOT NULL, last_opened_at TEXT
                );
                CREATE TABLE reading_states (
                    book_id TEXT PRIMARY KEY, locator_json TEXT NOT NULL,
                    locator_version INTEGER NOT NULL, global_progression REAL NOT NULL
                );
                CREATE TABLE book_settings (
                    book_id TEXT PRIMARY KEY, mode TEXT, font_scale REAL, theme TEXT
                );
                CREATE TABLE app_settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL);",
            )
            .expect("schema");
        connection
            .execute(
                "INSERT INTO books VALUES ('b1','fp1','/tmp/a.epub','书名','{}',1,'active',NULL)",
                [],
            )
            .expect("insert");
        connection
    }

    #[test]
    fn reads_active_book_only() {
        let connection = setup();
        let record = active_book(&connection, "b1")
            .expect("query")
            .expect("record");
        assert_eq!(record.title, "书名");
        assert_eq!(record.manifest_version, 1);
        assert!(record.last_opened_at.is_none());
        assert!(
            active_book(&connection, "missing")
                .expect("query")
                .is_none()
        );
        connection
            .execute("UPDATE books SET lifecycle='removed' WHERE id='b1'", [])
            .expect("remove");
        assert!(active_book(&connection, "b1").expect("query").is_none());
    }

    #[test]
    fn reads_reading_state_settings_and_app_setting() {
        let connection = setup();
        connection
            .execute(
                "INSERT INTO reading_states VALUES ('b1','{\"version\":1}',1,0.25)",
                [],
            )
            .expect("state");
        connection
            .execute("INSERT INTO book_settings VALUES ('b1',NULL,1.5,NULL)", [])
            .expect("settings");
        connection
            .execute(
                "INSERT INTO app_settings VALUES ('reader_defaults','{\"mode\":\"scrolling\"}')",
                [],
            )
            .expect("global");

        let state = reading_state(&connection, "b1")
            .expect("query")
            .expect("state");
        assert_eq!(state.locator_version, 1);
        assert!((state.global_progression - 0.25).abs() < f64::EPSILON);
        let settings = book_settings(&connection, "b1")
            .expect("query")
            .expect("settings");
        assert!(settings.mode.is_none());
        assert_eq!(settings.font_scale, Some(1.5));
        assert!(settings.theme.is_none());
        assert_eq!(
            app_setting(&connection, "reader_defaults")
                .expect("query")
                .as_deref(),
            Some("{\"mode\":\"scrolling\"}")
        );
        assert!(
            reading_state(&connection, "missing")
                .expect("query")
                .is_none()
        );
        assert!(
            book_settings(&connection, "missing")
                .expect("query")
                .is_none()
        );
    }

    #[test]
    fn marks_opened_and_resets_missing_active() {
        let mut connection = setup();
        mark_opened(&mut connection, "b1", "2026-11-15T00:00:00Z").expect("mark");
        let record = active_book(&connection, "b1")
            .expect("query")
            .expect("record");
        assert_eq!(
            record.last_opened_at.as_deref(),
            Some("2026-11-15T00:00:00Z")
        );
        connection
            .execute("UPDATE books SET lifecycle='removed' WHERE id='b1'", [])
            .expect("remove");
        assert!(mark_opened(&mut connection, "b1", "2026-11-15T00:00:01Z").is_err());
    }
}
