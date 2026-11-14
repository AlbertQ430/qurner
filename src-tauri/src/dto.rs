//! IPC 层 DTO 与事件载荷（camelCase）。

use std::path::Path;

use library_core::{ImportItemResult, ImportStatus};
use reader_model::BookSummary;
use serde::Serialize;

use crate::error::AppError;

/// 书库摘要；同名字段与 SDS 9.1/前端类型一致。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookSummaryDto {
    pub id: String,
    pub title: String,
    pub author: String,
    pub cover_url: Option<String>,
    pub imported_at: String,
    pub last_opened_at: Option<String>,
}

impl From<BookSummary> for BookSummaryDto {
    fn from(summary: BookSummary) -> Self {
        Self {
            id: summary.id,
            title: summary.title,
            author: summary.author,
            cover_url: summary.cover_cache_key.map(|key| format!("cover://{key}")),
            imported_at: summary.imported_at,
            last_opened_at: summary.last_opened_at,
        }
    }
}

/// 单本导入状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportStatusDto {
    Imported,
    Reactivated,
    Duplicate,
    Failed,
}

impl From<ImportStatus> for ImportStatusDto {
    fn from(status: ImportStatus) -> Self {
        match status {
            ImportStatus::Imported => Self::Imported,
            ImportStatus::Reactivated => Self::Reactivated,
            ImportStatus::Duplicate => Self::Duplicate,
            ImportStatus::Failed => Self::Failed,
        }
    }
}

/// 单本导入结果；`sourceName` 只包含文件 basename。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItemResultDto {
    pub source_name: String,
    pub status: ImportStatusDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
}

impl ImportItemResultDto {
    #[must_use]
    pub fn from_result(result: ImportItemResult, path: &Path) -> Self {
        Self {
            source_name: source_name(path),
            status: result.status.into(),
            book_id: result.book_id,
            error: result.error.map(AppError::from_book_error),
        }
    }
}

fn source_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// 每个输入路径产生一次。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgressEvent {
    pub job_id: String,
    pub completed: usize,
    pub total: usize,
    pub item: ImportItemResultDto,
}

/// 任务结束（成功、部分失败或全部失败）时产生一次。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCompletedEvent {
    pub job_id: String,
    pub items: Vec<ImportItemResultDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_cover_cache_key_to_cover_url() {
        let base = BookSummary {
            id: "book-1".to_string(),
            title: "标题".to_string(),
            author: "作者".to_string(),
            cover_cache_key: None,
            imported_at: "2026-11-14T00:00:00Z".to_string(),
            last_opened_at: None,
        };
        let dto = BookSummaryDto::from(base.clone());
        assert!(dto.cover_url.is_none());

        let with_cover = BookSummary {
            cover_cache_key: Some("sha256-cover".to_string()),
            ..base
        };
        let dto = BookSummaryDto::from(with_cover);
        assert_eq!(dto.cover_url.as_deref(), Some("cover://sha256-cover"));
    }

    #[test]
    fn summary_serializes_camel_case() {
        let dto = BookSummaryDto {
            id: "book-1".to_string(),
            title: "Title".to_string(),
            author: "Author".to_string(),
            cover_url: None,
            imported_at: "2026-11-14T00:00:00Z".to_string(),
            last_opened_at: None,
        };
        let value = serde_json::to_value(&dto).expect("serialize");
        assert_eq!(value["coverUrl"], serde_json::Value::Null);
        assert_eq!(value["importedAt"], "2026-11-14T00:00:00Z");
        assert_eq!(value["lastOpenedAt"], serde_json::Value::Null);
    }

    #[test]
    fn import_result_uses_basename_only() {
        let result = ImportItemResult {
            status: ImportStatus::Imported,
            book_id: Some("book-1".to_string()),
            error: None,
        };
        let dto =
            ImportItemResultDto::from_result(result, Path::new("/Users/example/Books/我的书.epub"));
        assert_eq!(dto.source_name, "我的书.epub");
        let value = serde_json::to_value(&dto).expect("serialize");
        assert_eq!(value["status"], "imported");
        assert!(!value.to_string().contains("/Users/"));
        assert!(value.get("error").is_none());
    }

    #[test]
    fn failed_import_serializes_stable_error() {
        let result = ImportItemResult {
            status: ImportStatus::Failed,
            book_id: None,
            error: Some(library_core::BookError::FileNotFound),
        };
        let dto = ImportItemResultDto::from_result(result, Path::new("/tmp/missing.epub"));
        let value = serde_json::to_value(&dto).expect("serialize");
        assert_eq!(value["status"], "failed");
        assert_eq!(value["error"]["code"], "E_FILE_NOT_FOUND");
    }
}
