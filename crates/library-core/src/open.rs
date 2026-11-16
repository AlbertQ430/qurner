//! 打开书籍用例：读取记录、校验源文件与 manifest、合并设置、构建初始位置。

use std::path::PathBuf;

use epub_core::{PARSED_BOOK_SCHEMA_VERSION, ParsedBook, TocNode, fingerprint_file};
use reader_model::{ReaderLocator, ReaderSettings, ReaderTocNode};
use storage_core::{StorageError, StorageService};

use crate::error::{BookError, StorageErrorKind};

/// `app_settings` 中全局阅读设置的键。
pub const GLOBAL_SETTINGS_KEY: &str = "reader_defaults";

/// 打开书籍后、创建会话前的完整数据。
#[derive(Debug)]
pub struct OpenBook {
    pub book_id: String,
    pub title: String,
    pub source_path: PathBuf,
    pub book: ParsedBook,
    pub toc: Vec<ReaderTocNode>,
    pub chapter_weights: Vec<u32>,
    pub initial_locator: Option<ReaderLocator>,
    pub initial_progression: f64,
    pub settings: ReaderSettings,
}

/// 打开书籍；所有校验失败都不修改记录或个人数据。
pub(crate) fn open_book(storage: &StorageService, book_id: &str) -> Result<OpenBook, BookError> {
    let id = book_id.trim();
    if id.is_empty() {
        return Err(BookError::InvalidInput);
    }
    let record = storage
        .open_book_record(id)
        .map_err(map_storage)?
        .ok_or(BookError::BookNotFound)?;

    let source_path = PathBuf::from(&record.source_path);
    let metadata = std::fs::metadata(&source_path).map_err(|_| BookError::BookUnavailable)?;
    if !metadata.is_file() {
        return Err(BookError::BookUnavailable);
    }
    std::fs::File::open(&source_path).map_err(|_| BookError::BookUnavailable)?;

    let fingerprint = fingerprint_file(&source_path).map_err(|_| BookError::BookUnavailable)?;
    if fingerprint.to_string() != record.fingerprint {
        return Err(BookError::SourceChanged);
    }

    let book: ParsedBook =
        serde_json::from_str(&record.manifest_json).map_err(|_| storage_error())?;
    if book.schema_version != PARSED_BOOK_SCHEMA_VERSION || book.spine.is_empty() {
        return Err(storage_error());
    }
    if book.spine[0].href.as_str().is_empty() {
        return Err(storage_error());
    }
    if record.manifest_version != book.schema_version {
        return Err(storage_error());
    }
    if book.chapter_weights.len() != book.spine.len() {
        return Err(storage_error());
    }

    let (initial_locator, initial_progression) = load_reading_state(storage, id, &book)?;
    let settings = load_settings(storage, id)?;
    let toc = map_toc(&book.toc, &book);
    let chapter_weights = book.chapter_weights.clone();

    Ok(OpenBook {
        book_id: record.id,
        title: record.title,
        source_path,
        book,
        toc,
        chapter_weights,
        initial_locator,
        initial_progression,
        settings,
    })
}

/// 记录最近打开时间。
pub(crate) fn mark_opened(storage: &StorageService, book_id: &str) -> Result<(), BookError> {
    let timestamp = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| BookError::Internal)?;
    storage
        .mark_opened(book_id, &timestamp)
        .map_err(map_storage)
}

fn load_reading_state(
    storage: &StorageService,
    book_id: &str,
    book: &ParsedBook,
) -> Result<(Option<ReaderLocator>, f64), BookError> {
    let Some(state) = storage.reading_state(book_id).map_err(map_storage)? else {
        return Ok((None, 0.0));
    };
    if state.locator_version != 1 {
        return Err(storage_error());
    }
    let locator: ReaderLocator =
        serde_json::from_str(&state.locator_json).map_err(|_| storage_error())?;
    if locator.version != 1 {
        return Err(storage_error());
    }
    let Some(spine_index) = usize::try_from(locator.spine_index).ok() else {
        return Err(storage_error());
    };
    let found = book
        .spine
        .iter()
        .any(|item| item.index == spine_index && item.href.as_str() == locator.spine_href);
    if !found {
        return Err(storage_error());
    }
    if !state.global_progression.is_finite() || !(0.0..=1.0).contains(&state.global_progression) {
        return Err(storage_error());
    }
    Ok((Some(locator), state.global_progression))
}

fn load_settings(storage: &StorageService, book_id: &str) -> Result<ReaderSettings, BookError> {
    crate::settings::load_settings(storage, book_id)
}

fn map_toc(nodes: &[TocNode], book: &ParsedBook) -> Vec<ReaderTocNode> {
    let mut output = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let (path, fragment) = split_href(&node.href);
        let children = map_toc(&node.children, book);
        let Some(spine_index) = book
            .spine
            .iter()
            .position(|item| item.href.as_str() == path)
        else {
            // 无法映射到 spine 的父项：提升可映射的子项，保留顺序。
            output.extend(children);
            continue;
        };
        let id = if node.id.is_empty() {
            format!("toc-{index}-{path}")
        } else {
            node.id.clone()
        };
        output.push(ReaderTocNode {
            id,
            label: node.label.clone(),
            spine_index,
            fragment: fragment
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            children,
        });
    }
    output
}

fn split_href(href: &str) -> (&str, Option<&str>) {
    match href.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (href, None),
    }
}

fn map_storage(error: StorageError) -> BookError {
    BookError::Storage(StorageErrorKind::from(error))
}

fn storage_error() -> BookError {
    BookError::Storage(StorageErrorKind::QueryFailed)
}
