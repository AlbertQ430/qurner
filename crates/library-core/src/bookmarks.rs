//! 书签用例：会话/书籍绑定、excerpt 截断、CRUD 与错误映射。

use reader_model::{Bookmark, ReaderLocator, excerpt_from_quote};
use storage_core::{BookmarkRecord, StorageError, StorageService};

use crate::error::{BookError, StorageErrorKind};
use crate::settings::ensure_active;

pub(crate) fn list_bookmarks(
    storage: &StorageService,
    book_id: &str,
) -> Result<Vec<Bookmark>, BookError> {
    let id = book_id.trim();
    if id.is_empty() {
        return Err(BookError::InvalidInput);
    }
    ensure_active(storage, id)?;
    let records = storage.list_bookmarks(id).map_err(map_storage)?;
    records
        .into_iter()
        .map(|record| parse_record(storage, record))
        .collect()
}

pub(crate) fn create_bookmark(
    storage: &StorageService,
    book_id: &str,
    locator: &ReaderLocator,
    excerpt: Option<&str>,
) -> Result<Bookmark, BookError> {
    let id = book_id.trim();
    if id.is_empty() {
        return Err(BookError::InvalidInput);
    }
    ensure_active(storage, id)?;
    let locator_json = serde_json::to_string(locator).map_err(|_| BookError::Internal)?;
    let excerpt = excerpt
        .and_then(excerpt_from_quote)
        .or_else(|| excerpt_from_quote(&locator.quote.exact));
    let created_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| BookError::Internal)?;
    let bookmark = Bookmark {
        id: uuid::Uuid::new_v4().to_string(),
        book_id: id.to_string(),
        locator: locator.clone(),
        excerpt,
        created_at,
    };
    storage
        .insert_bookmark(BookmarkRecord {
            id: bookmark.id.clone(),
            book_id: bookmark.book_id.clone(),
            locator_json,
            locator_version: 1,
            excerpt: bookmark.excerpt.clone(),
            created_at: bookmark.created_at.clone(),
        })
        .map_err(map_storage)?;
    Ok(bookmark)
}

pub(crate) fn delete_bookmark(
    storage: &StorageService,
    book_id: &str,
    bookmark_id: &str,
) -> Result<(), BookError> {
    let id = book_id.trim();
    if id.is_empty() || bookmark_id.trim().is_empty() {
        return Err(BookError::InvalidInput);
    }
    ensure_active(storage, id)?;
    let deleted = storage
        .delete_bookmark(id, bookmark_id.trim())
        .map_err(map_storage)?;
    if !deleted {
        return Err(BookError::InvalidInput);
    }
    Ok(())
}

fn parse_record(_storage: &StorageService, record: BookmarkRecord) -> Result<Bookmark, BookError> {
    if record.locator_version != 1 {
        return Err(storage_error());
    }
    let locator: ReaderLocator =
        serde_json::from_str(&record.locator_json).map_err(|_| storage_error())?;
    if locator.version != 1 {
        return Err(storage_error());
    }
    if let Some(value) = &record.excerpt {
        let length = value
            .chars()
            .map(|character| character.len_utf16())
            .sum::<usize>();
        if length > reader_model::MAX_BOOKMARK_EXCERPT_UTF16 {
            return Err(storage_error());
        }
    }
    Ok(Bookmark {
        id: record.id,
        book_id: record.book_id,
        locator,
        excerpt: record.excerpt,
        created_at: record.created_at,
    })
}

fn map_storage(error: StorageError) -> BookError {
    BookError::Storage(StorageErrorKind::from(error))
}

fn storage_error() -> BookError {
    BookError::Storage(StorageErrorKind::QueryFailed)
}
