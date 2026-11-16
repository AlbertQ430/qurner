//! 公开 Tauri 命令：startImport、listBooks、softRemoveBook。

use std::path::Path;
use std::sync::Arc;

use reader_model::{BookSort, ListBooksQuery, SortDirection};
use serde_json::{Value, json};
use tauri::Emitter;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::dto::{
    BookSummaryDto, BookmarkDto, ImportCompletedEvent, ImportItemResultDto, ImportProgressEvent,
    SaveReadingStateResult,
};
use crate::error::{AppError, AppErrorCode};
use crate::state::AppState;
use library_core::{AppSettingsPatch, BookSettingsPatch};
use reader_model::{
    ReaderLocator, ReaderMode, ReaderSession, ReaderSettings, ReaderSpineItem, ReaderTheme,
};

const MAX_IMPORT_PATHS: usize = 1_000;

/// 异步导入：立即返回 jobId，阻塞线程逐本导入并发送进度/完成事件。
#[tauri::command(rename = "startImport")]
pub fn start_import<R: tauri::Runtime>(
    state: State<'_, AppState>,
    app: AppHandle<R>,
    request: Request<'_>,
) -> Result<Value, AppError> {
    let body = json_body(&request)?;
    let paths = parse_paths(&body)?;
    let library = state.library()?;
    let scheduler = state.scheduler();
    let job_id = uuid::Uuid::new_v4().to_string();
    let total = paths.len();
    let event_job_id = job_id.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let _slot = scheduler.acquire();
        let mut items = Vec::with_capacity(total);

        for (index, path) in paths.iter().enumerate() {
            let result = library.import(Path::new(path));
            let item = ImportItemResultDto::from_result(result, Path::new(path));
            let progress = ImportProgressEvent {
                job_id: event_job_id.clone(),
                completed: index + 1,
                total,
                item: item.clone(),
            };
            if app.emit("import-progress", &progress).is_err() {
                tracing::error!(
                    code = AppErrorCode::Internal.as_str(),
                    stage = "emit_import_progress",
                    "导入进度事件发送失败"
                );
            }
            items.push(item);
        }

        let completed = ImportCompletedEvent {
            job_id: event_job_id,
            items,
        };
        if app.emit("import-completed", &completed).is_err() {
            tracing::error!(
                code = AppErrorCode::Internal.as_str(),
                stage = "emit_import_completed",
                "导入完成事件发送失败"
            );
        }
    });

    Ok(json!({ "jobId": job_id }))
}

/// 查询 active 书库列表。
#[tauri::command(rename = "listBooks")]
pub async fn list_books(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<Vec<BookSummaryDto>, AppError> {
    let body = json_body(&request)?;
    let query = parse_list_books_query(&body)?;
    let library = state.library()?;
    tauri::async_runtime::spawn_blocking(move || {
        library
            .list_books(query)
            .map_err(AppError::from_book_error)
            .map(|summaries| summaries.into_iter().map(BookSummaryDto::from).collect())
    })
    .await
    .map_err(|_| AppError::new(AppErrorCode::Internal))?
}

/// 软移除书籍，保留个人阅读数据。
#[tauri::command(rename = "softRemoveBook")]
pub async fn soft_remove_book(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<(), AppError> {
    let body = json_body(&request)?;
    let book_id = parse_book_id(&body)?.to_string();
    let library = state.library()?;
    tauri::async_runtime::spawn_blocking(move || {
        library
            .soft_remove(&book_id)
            .map_err(AppError::from_book_error)
    })
    .await
    .map_err(|_| AppError::new(AppErrorCode::Internal))?
}

fn json_body(request: &Request<'_>) -> Result<Value, AppError> {
    match request.body() {
        InvokeBody::Json(value) => Ok(value.clone()),
        _ => Err(AppError::invalid_input()),
    }
}

fn parse_paths(body: &Value) -> Result<Vec<String>, AppError> {
    let paths = body
        .get("paths")
        .and_then(Value::as_array)
        .ok_or_else(AppError::invalid_input)?;
    if paths.is_empty() || paths.len() > MAX_IMPORT_PATHS {
        return Err(AppError::invalid_input());
    }

    let mut parsed = Vec::with_capacity(paths.len());
    for path in paths {
        let value = path.as_str().ok_or_else(AppError::invalid_input)?;
        if value.trim().is_empty() {
            return Err(AppError::invalid_input());
        }
        // 只对 trim 后的临时值做空白判断；入队与导入使用调用方原始字符串。
        parsed.push(value.to_string());
    }
    Ok(parsed)
}

fn parse_list_books_query(body: &Value) -> Result<ListBooksQuery, AppError> {
    let input = body
        .get("input")
        .and_then(Value::as_object)
        .ok_or_else(AppError::invalid_input)?;

    let query = match input.get("query") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => return Err(AppError::invalid_input()),
    };

    let sort = match input.get("sort").and_then(Value::as_str) {
        Some("title") => BookSort::Title,
        Some("author") => BookSort::Author,
        Some("importedAt") => BookSort::ImportedAt,
        _ => return Err(AppError::invalid_input()),
    };

    let direction = match input.get("direction").and_then(Value::as_str) {
        Some("asc") => SortDirection::Asc,
        Some("desc") => SortDirection::Desc,
        _ => return Err(AppError::invalid_input()),
    };

    Ok(ListBooksQuery {
        query,
        sort,
        direction,
    })
}

fn parse_book_id(body: &Value) -> Result<&str, AppError> {
    body.get("bookId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(AppError::invalid_input)
}

/// 打开书籍：校验记录与源文件、创建 M3 会话并返回首章与初始位置。
#[tauri::command(rename = "openBook")]
pub async fn open_book(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<ReaderSession, AppError> {
    let body = json_body(&request)?;
    let book_id = parse_book_id(&body)?.to_string();
    let library = state.library()?;
    let sessions = state.sessions();
    let worker_library = Arc::clone(&library);
    let opened = tauri::async_runtime::spawn_blocking(move || worker_library.open_book(&book_id))
        .await
        .map_err(|_| AppError::new(AppErrorCode::Internal))?
        .map_err(AppError::from_book_error)?;

    finalize_open(opened, &sessions, |book_id| library.mark_opened(book_id))
}

/// 创建会话并记录 last_opened；更新失败时清理会话，避免半开放状态。
fn finalize_open(
    opened: library_core::OpenBook,
    sessions: &crate::session::SessionRegistry,
    mark_opened: impl FnOnce(&str) -> Result<(), library_core::BookError>,
) -> Result<ReaderSession, AppError> {
    let frame_href = opened
        .book
        .spine
        .first()
        .map(|item| item.href.as_str().to_string())
        .ok_or_else(|| AppError::new(AppErrorCode::Storage))?;
    let handle = sessions
        .create(&opened.book_id, &opened.source_path, opened.book.clone())
        .map_err(|_| AppError::new(AppErrorCode::BookUnavailable))?;
    let frame_url = build_frame_url(&handle.token, &frame_href);

    if let Err(error) = mark_opened(&opened.book_id) {
        sessions.close(&handle.session_id);
        return Err(AppError::from_book_error(error));
    }

    let spine = opened
        .book
        .spine
        .iter()
        .map(|item| ReaderSpineItem {
            index: item.index,
            href: item.href.as_str().to_string(),
        })
        .collect();
    Ok(ReaderSession {
        session_id: handle.session_id,
        book_id: opened.book_id,
        title: opened.title,
        frame_url,
        spine,
        chapter_weights: opened.chapter_weights,
        toc: opened.toc,
        initial_locator: opened.initial_locator,
        initial_progression: opened.initial_progression,
        settings: opened.settings,
    })
}

/// 关闭阅读会话；未知/已关闭返回 E_SESSION_INVALID。
#[tauri::command(rename = "closeBook")]
pub fn close_book(state: State<'_, AppState>, request: Request<'_>) -> Result<(), AppError> {
    let body = json_body(&request)?;
    let session_id = parse_session_id(&body)?;
    if !state.sessions().close(session_id) {
        return Err(AppError::new(AppErrorCode::SessionInvalid));
    }
    Ok(())
}

fn parse_session_id(body: &Value) -> Result<&str, AppError> {
    let value = body
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(AppError::invalid_input)?;
    let parsed = uuid::Uuid::parse_str(value).map_err(|_| AppError::invalid_input())?;
    if parsed.get_version() != Some(uuid::Version::Random) {
        return Err(AppError::invalid_input());
    }
    Ok(value)
}

fn build_frame_url(token: &str, href: &str) -> String {
    format!("epub://{token}/{}", encode_path(href))
}

fn encode_path(path: &str) -> String {
    let mut output = String::new();
    for byte in path.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                output.push(*byte as char);
            }
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

/// 读取单书 effective settings。
#[tauri::command(rename = "getEffectiveSettings")]
pub async fn get_effective_settings(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<ReaderSettings, AppError> {
    let body = json_body(&request)?;
    let book_id = parse_book_id(&body)?.to_string();
    let library = state.library()?;
    tauri::async_runtime::spawn_blocking(move || library.get_effective_settings(&book_id))
        .await
        .map_err(|_| AppError::new(AppErrorCode::Internal))?
        .map_err(AppError::from_book_error)
}

/// 更新全局默认设置并返回新的全局设置。
#[tauri::command(rename = "updateAppSettings")]
pub async fn update_app_settings(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<ReaderSettings, AppError> {
    let body = json_body(&request)?;
    let patch = parse_app_patch(body.get("patch").ok_or_else(AppError::invalid_input)?)?;
    let library = state.library()?;
    tauri::async_runtime::spawn_blocking(move || library.update_app_settings(patch))
        .await
        .map_err(|_| AppError::new(AppErrorCode::Internal))?
        .map_err(AppError::from_book_error)
}

/// 更新单书覆盖并返回合并后的 effective settings。
#[tauri::command(rename = "updateBookSettings")]
pub async fn update_book_settings(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<ReaderSettings, AppError> {
    let body = json_body(&request)?;
    let input = body.get("input").ok_or_else(AppError::invalid_input)?;
    let book_id = parse_book_id(input)?.to_string();
    let patch = parse_book_patch(input.get("patch").ok_or_else(AppError::invalid_input)?)?;
    let library = state.library()?;
    tauri::async_runtime::spawn_blocking(move || library.update_book_settings(&book_id, patch))
        .await
        .map_err(|_| AppError::new(AppErrorCode::Internal))?
        .map_err(AppError::from_book_error)
}

/// 清除单书覆盖并返回全局设置。
#[tauri::command(rename = "resetBookSettings")]
pub async fn reset_book_settings(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<ReaderSettings, AppError> {
    let body = json_body(&request)?;
    let book_id = parse_book_id(&body)?.to_string();
    let library = state.library()?;
    tauri::async_runtime::spawn_blocking(move || library.reset_book_settings(&book_id))
        .await
        .map_err(|_| AppError::new(AppErrorCode::Internal))?
        .map_err(AppError::from_book_error)
}

fn parse_app_patch(value: &Value) -> Result<AppSettingsPatch, AppError> {
    let object = value.as_object().ok_or_else(AppError::invalid_input)?;
    let mut patch = AppSettingsPatch::default();
    for key in object.keys() {
        match key.as_str() {
            "mode" | "fontScale" | "theme" => {}
            _ => return Err(AppError::invalid_input()),
        }
    }
    if let Some(value) = object.get("mode") {
        patch.mode = Some(parse_mode(value)?);
    }
    if let Some(value) = object.get("fontScale") {
        patch.font_scale = Some(parse_font_scale(value)?);
    }
    if let Some(value) = object.get("theme") {
        patch.theme = Some(parse_theme(value)?);
    }
    if patch.is_empty() {
        return Err(AppError::invalid_input());
    }
    Ok(patch)
}

fn parse_book_patch(value: &Value) -> Result<BookSettingsPatch, AppError> {
    let object = value.as_object().ok_or_else(AppError::invalid_input)?;
    let mut patch = BookSettingsPatch::default();
    for key in object.keys() {
        match key.as_str() {
            "mode" | "fontScale" | "theme" => {}
            _ => return Err(AppError::invalid_input()),
        }
    }
    if let Some(value) = object.get("mode") {
        patch.mode = Some(if value.is_null() {
            None
        } else {
            Some(parse_mode(value)?)
        });
    }
    if let Some(value) = object.get("fontScale") {
        patch.font_scale = Some(if value.is_null() {
            None
        } else {
            Some(parse_font_scale(value)?)
        });
    }
    if let Some(value) = object.get("theme") {
        patch.theme = Some(if value.is_null() {
            None
        } else {
            Some(parse_theme(value)?)
        });
    }
    if patch.is_empty() {
        return Err(AppError::invalid_input());
    }
    Ok(patch)
}

fn parse_mode(value: &Value) -> Result<ReaderMode, AppError> {
    match value.as_str() {
        Some("paged") => Ok(ReaderMode::Paged),
        Some("scrolling") => Ok(ReaderMode::Scrolling),
        _ => Err(AppError::invalid_input()),
    }
}

fn parse_theme(value: &Value) -> Result<ReaderTheme, AppError> {
    match value.as_str() {
        Some("day") => Ok(ReaderTheme::Day),
        Some("night") => Ok(ReaderTheme::Night),
        _ => Err(AppError::invalid_input()),
    }
}

fn parse_font_scale(value: &Value) -> Result<f64, AppError> {
    let number = value.as_f64().ok_or_else(AppError::invalid_input)?;
    if number.is_finite()
        && (ReaderSettings::MIN_FONT_SCALE..=ReaderSettings::MAX_FONT_SCALE).contains(&number)
    {
        Ok(number)
    } else {
        Err(AppError::invalid_input())
    }
}

/// 保存阅读位置；bookId 只来自有效会话。
#[tauri::command(rename = "saveReadingState")]
pub async fn save_reading_state(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<SaveReadingStateResult, AppError> {
    let body = json_body(&request)?;
    let session_id = parse_session_id(&body)?.to_string();
    let locator_value = body
        .get("locator")
        .cloned()
        .ok_or_else(AppError::invalid_input)?;
    let session = state
        .sessions()
        .find(&session_id)
        .ok_or_else(|| AppError::new(AppErrorCode::SessionInvalid))?;
    let locator = extract_session_locator(&session, &locator_value)?;
    let spine_index =
        usize::try_from(locator.spine_index).map_err(|_| AppError::invalid_input())?;
    if session.book.chapter_weights.len() != session.book.spine.len() {
        return Err(AppError::new(AppErrorCode::Storage));
    }
    let progression = library_core::compute_global_progression(
        &session.book.chapter_weights,
        spine_index,
        locator.intra_spine_progression,
    );
    let locator_json =
        serde_json::to_string(&locator).map_err(|_| AppError::new(AppErrorCode::Internal))?;
    let library = state.library()?;
    let book_id = session.book_id.clone();
    let saved_at = tauri::async_runtime::spawn_blocking(move || {
        library.save_reading_state(&book_id, &locator_json, progression)
    })
    .await
    .map_err(|_| AppError::new(AppErrorCode::Internal))?
    .map_err(AppError::from_book_error)?;
    Ok(SaveReadingStateResult {
        global_progression: progression,
        saved_at,
    })
}

/// 列出当前书籍书签。
#[tauri::command(rename = "listBookmarks")]
pub async fn list_bookmarks(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<Vec<BookmarkDto>, AppError> {
    let body = json_body(&request)?;
    let book_id = parse_book_id(&body)?.to_string();
    let library = state.library()?;
    tauri::async_runtime::spawn_blocking(move || library.list_bookmarks(&book_id))
        .await
        .map_err(|_| AppError::new(AppErrorCode::Internal))?
        .map_err(AppError::from_book_error)
        .map(|bookmarks| bookmarks.into_iter().map(bookmark_dto).collect())
}

/// 从有效 session 创建书签；bookId 只来自 session。
#[tauri::command(rename = "createBookmark")]
pub async fn create_bookmark(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<BookmarkDto, AppError> {
    let body = json_body(&request)?;
    let session_id = parse_session_id(&body)?.to_string();
    let locator_value = body
        .get("locator")
        .cloned()
        .ok_or_else(AppError::invalid_input)?;
    let excerpt = body
        .get("excerpt")
        .and_then(Value::as_str)
        .map(str::to_string);
    let session = state
        .sessions()
        .find(&session_id)
        .ok_or_else(|| AppError::new(AppErrorCode::SessionInvalid))?;
    let locator = extract_session_locator(&session, &locator_value)?;
    let library = state.library()?;
    let book_id = session.book_id.clone();
    let bookmark = tauri::async_runtime::spawn_blocking(move || {
        library.create_bookmark(&book_id, &locator, excerpt.as_deref())
    })
    .await
    .map_err(|_| AppError::new(AppErrorCode::Internal))?
    .map_err(AppError::from_book_error)?;
    Ok(bookmark_dto(bookmark))
}

/// 删除属于指定书籍的书签。
#[tauri::command(rename = "deleteBookmark")]
pub async fn delete_bookmark(
    state: State<'_, AppState>,
    request: Request<'_>,
) -> Result<(), AppError> {
    let body = json_body(&request)?;
    let book_id = parse_book_id(&body)?.to_string();
    let bookmark_id = body
        .get("bookmarkId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(AppError::invalid_input)?;
    let parsed = uuid::Uuid::parse_str(bookmark_id).map_err(|_| AppError::invalid_input())?;
    if parsed.get_version() != Some(uuid::Version::Random) {
        return Err(AppError::invalid_input());
    }
    let bookmark_id = bookmark_id.to_string();
    let library = state.library()?;
    tauri::async_runtime::spawn_blocking(move || library.delete_bookmark(&book_id, &bookmark_id))
        .await
        .map_err(|_| AppError::new(AppErrorCode::Internal))?
        .map_err(AppError::from_book_error)
}

fn extract_session_locator(
    session: &crate::session::Session,
    value: &Value,
) -> Result<ReaderLocator, AppError> {
    let locator: ReaderLocator =
        serde_json::from_value(value.clone()).map_err(|_| AppError::invalid_input())?;
    if locator.version != 1 {
        return Err(AppError::invalid_input());
    }
    let spine_index =
        usize::try_from(locator.spine_index).map_err(|_| AppError::invalid_input())?;
    let Some(item) = session.book.spine.get(spine_index) else {
        return Err(AppError::invalid_input());
    };
    if item.href.as_str() != locator.spine_href {
        return Err(AppError::invalid_input());
    }
    Ok(locator)
}

fn bookmark_dto(bookmark: reader_model::Bookmark) -> BookmarkDto {
    BookmarkDto {
        id: bookmark.id,
        book_id: bookmark.book_id,
        locator: bookmark.locator,
        excerpt: bookmark.excerpt,
        created_at: bookmark.created_at,
    }
}

/// 打开系统原生多选文件对话框，返回用户确认的原始路径；取消返回空数组。
#[tauri::command(rename = "selectImportFiles")]
pub async fn select_import_files<R: tauri::Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<String>, AppError> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || select_import_files_blocking(&app))
        .await
        .map_err(|_| AppError::new(AppErrorCode::Internal))?
}

fn select_import_files_blocking<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<Vec<String>, AppError> {
    let files = app
        .dialog()
        .file()
        .add_filter("EPUB", &["epub"])
        .blocking_pick_files();
    match files {
        None => Ok(Vec::new()),
        Some(files) => validate_selected_files(files),
    }
}

/// 校验选择结果：只接受 1–1,000 个路径，保持顺序，拒绝空白与非 UTF-8 路径。
fn validate_selected_files(
    files: Vec<tauri_plugin_dialog::FilePath>,
) -> Result<Vec<String>, AppError> {
    if files.len() > MAX_IMPORT_PATHS {
        return Err(AppError::invalid_input());
    }
    let mut paths = Vec::with_capacity(files.len());
    for file in files {
        let path = file.into_path().map_err(|_| AppError::invalid_input())?;
        let Some(value) = path.to_str() else {
            return Err(AppError::invalid_input());
        };
        if value.trim().is_empty() {
            return Err(AppError::invalid_input());
        }
        paths.push(value.to_string());
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_selected_dialog_files() {
        use std::os::unix::ffi::OsStringExt;
        use tauri_plugin_dialog::FilePath;

        fn path(value: &str) -> FilePath {
            FilePath::Path(std::path::PathBuf::from(value))
        }

        assert_eq!(
            validate_selected_files(vec![]).expect("cancel"),
            Vec::<String>::new()
        );
        assert_eq!(
            validate_selected_files(vec![path("/tmp/a.epub")]).expect("single"),
            vec!["/tmp/a.epub".to_string()]
        );

        let ordered: Vec<FilePath> = ["c", "a", "b"]
            .into_iter()
            .map(|name| path(&format!("/tmp/{name}.epub")))
            .collect();
        assert_eq!(
            validate_selected_files(ordered).expect("order"),
            vec![
                "/tmp/c.epub".to_string(),
                "/tmp/a.epub".to_string(),
                "/tmp/b.epub".to_string(),
            ]
        );

        let max: Vec<FilePath> = (0..MAX_IMPORT_PATHS)
            .map(|index| path(&format!("/tmp/{index}.epub")))
            .collect();
        assert_eq!(
            validate_selected_files(max).expect("max").len(),
            MAX_IMPORT_PATHS
        );

        let too_many: Vec<FilePath> = (0..=MAX_IMPORT_PATHS)
            .map(|index| path(&format!("/tmp/{index}.epub")))
            .collect();
        assert_eq!(
            validate_selected_files(too_many)
                .expect_err("too many")
                .code,
            AppErrorCode::InvalidInput
        );

        for invalid in [
            path("   "),
            path("\t"),
            FilePath::Path(std::ffi::OsString::from_vec(vec![0xFF, 0xFE]).into()),
        ] {
            assert_eq!(
                validate_selected_files(vec![invalid])
                    .expect_err("invalid path")
                    .code,
                AppErrorCode::InvalidInput
            );
        }
    }

    #[test]
    fn finalize_open_cleans_up_session_when_mark_opened_fails() {
        use epub_core::{ParsedBook, ResourcePath, SpineItem};
        use library_core::{BookError, OpenBook, StorageErrorKind};
        use reader_model::ReaderSettings;

        let path =
            std::env::temp_dir().join(format!("qurner-finalize-{}.epub", std::process::id()));
        std::fs::write(&path, b"x").expect("write temp");
        let mut book = ParsedBook::new(ResourcePath::new("OEBPS/content.opf").expect("opf"));
        book.spine.push(SpineItem {
            index: 0,
            idref: "c1".to_string(),
            href: ResourcePath::new("OEBPS/chapter1.xhtml").expect("href"),
            linear: true,
        });
        let opened = OpenBook {
            book_id: "book-1".to_string(),
            title: "书名".to_string(),
            source_path: path.clone(),
            book,
            toc: Vec::new(),
            chapter_weights: vec![1],
            initial_locator: None,
            initial_progression: 0.0,
            settings: ReaderSettings::default(),
        };
        let sessions = crate::session::SessionRegistry::new();
        let error = finalize_open(opened, &sessions, |_| {
            Err(BookError::Storage(StorageErrorKind::QueryFailed))
        })
        .expect_err("mark_opened failure");
        assert_eq!(error.code, AppErrorCode::Storage);
        assert!(sessions.is_empty(), "更新失败必须清理刚创建的会话");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn builds_encoded_frame_urls() {
        let url = build_frame_url("a".repeat(32).as_str(), "OEBPS/第一章 节.xhtml");
        assert!(url.starts_with("epub://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/OEBPS/"));
        assert!(url.contains("%E7%AC%AC%E4%B8%80%E7%AB%A0"));
        assert!(url.contains("%20"));
        assert!(!url.contains(' '));
        let plain = build_frame_url("b".repeat(64).as_str(), "OEBPS/chapter1.xhtml");
        assert_eq!(
            plain,
            format!("epub://{}/OEBPS/chapter1.xhtml", "b".repeat(64))
        );
    }

    #[test]
    fn validates_paths() {
        assert!(parse_paths(&json!({ "paths": [] })).is_err());
        assert!(parse_paths(&json!({ "paths": ["  "] })).is_err());
        assert!(parse_paths(&json!({ "paths": "book.epub" })).is_err());

        let too_many: Vec<String> = (0..=MAX_IMPORT_PATHS)
            .map(|index| format!("/tmp/{index}.epub"))
            .collect();
        assert!(parse_paths(&json!({ "paths": too_many })).is_err());

        let parsed = parse_paths(&json!({ "paths": ["/tmp/a.epub"] })).expect("valid");
        assert_eq!(parsed, vec!["/tmp/a.epub"]);
    }

    #[test]
    fn validates_list_books_input() {
        let query = parse_list_books_query(&json!({
            "input": { "query": "rust", "sort": "title", "direction": "asc" }
        }))
        .expect("valid");
        assert_eq!(query.sort, BookSort::Title);
        assert_eq!(query.direction, SortDirection::Asc);

        let all = parse_list_books_query(&json!({
            "input": { "sort": "importedAt", "direction": "desc" }
        }))
        .expect("valid");
        assert!(all.query.is_none());

        assert!(
            parse_list_books_query(&json!({
                "input": { "sort": "unknown", "direction": "asc" }
            }))
            .is_err()
        );
        assert!(
            parse_list_books_query(&json!({
                "input": { "sort": "title", "direction": "sideways" }
            }))
            .is_err()
        );
        assert!(parse_list_books_query(&json!({ "input": "title" })).is_err());
    }

    #[test]
    fn validates_book_id() {
        assert_eq!(
            parse_book_id(&json!({ "bookId": "book-1" })).expect("valid"),
            "book-1"
        );
        assert!(parse_book_id(&json!({ "bookId": "   " })).is_err());
        assert!(parse_book_id(&json!({ "bookId": 3 })).is_err());
        assert!(parse_book_id(&json!({})).is_err());
    }
}
