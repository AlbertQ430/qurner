//! 公开 Tauri 命令：startImport、listBooks、softRemoveBook。

use std::path::Path;

use reader_model::{BookSort, ListBooksQuery, SortDirection};
use serde_json::{Value, json};
use tauri::Emitter;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, State};

use crate::dto::{BookSummaryDto, ImportCompletedEvent, ImportItemResultDto, ImportProgressEvent};
use crate::error::{AppError, AppErrorCode};
use crate::state::AppState;

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

#[cfg(test)]
mod tests {
    use super::*;

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
