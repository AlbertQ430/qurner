//! 书签读写（`bookmarks` 表，单条事务）。

use rusqlite::Connection;

use crate::StorageError;

/// 数据库书签记录。
#[derive(Debug, Clone, PartialEq)]
pub struct BookmarkRecord {
    pub id: String,
    pub book_id: String,
    pub locator_json: String,
    pub locator_version: u32,
    pub excerpt: Option<String>,
    pub created_at: String,
}

/// 列出书签：`created_at DESC, id ASC`。
pub(crate) fn list_bookmarks(
    connection: &Connection,
    book_id: &str,
) -> Result<Vec<BookmarkRecord>, StorageError> {
    let mut statement = connection
        .prepare(
            "SELECT id, book_id, locator_json, locator_version, excerpt, created_at
             FROM bookmarks WHERE book_id = ?1
             ORDER BY created_at DESC, id ASC",
        )
        .map_err(map_query_error)?;
    let rows = statement
        .query_map([book_id], |row| {
            let locator_version: i64 = row.get(3)?;
            Ok(BookmarkRecord {
                id: row.get(0)?,
                book_id: row.get(1)?,
                locator_json: row.get(2)?,
                locator_version: u32::try_from(locator_version)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(3, locator_version))?,
                excerpt: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(map_query_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(map_query_error)
}

/// 插入书签；重复 id 返回 `Conflict`。
pub(crate) fn insert_bookmark(
    connection: &Connection,
    record: &BookmarkRecord,
) -> Result<(), StorageError> {
    connection
        .execute(
            "INSERT INTO bookmarks (id, book_id, locator_json, locator_version, excerpt, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                record.id,
                record.book_id,
                record.locator_json,
                record.locator_version,
                record.excerpt,
                record.created_at
            ],
        )
        .map_err(map_insert_error)?;
    Ok(())
}

/// 删除属于指定书籍的书签；返回是否删除。
pub(crate) fn delete_bookmark(
    connection: &Connection,
    book_id: &str,
    bookmark_id: &str,
) -> Result<bool, StorageError> {
    let deleted = connection
        .execute(
            "DELETE FROM bookmarks WHERE id = ?1 AND book_id = ?2",
            rusqlite::params![bookmark_id, book_id],
        )
        .map_err(map_query_error)?;
    Ok(deleted > 0)
}

fn map_query_error(_error: rusqlite::Error) -> StorageError {
    StorageError::QueryFailed
}

fn map_insert_error(error: rusqlite::Error) -> StorageError {
    if let rusqlite::Error::SqliteFailure(inner, _) = &error
        && inner.code == rusqlite::ErrorCode::ConstraintViolation
    {
        return StorageError::Conflict;
    }
    StorageError::QueryFailed
}
