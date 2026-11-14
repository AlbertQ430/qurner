//! 书库列表查询与摘要 DTO（跨 crate 共享，camelCase JSON）。

use serde::{Deserialize, Serialize};

/// 书库排序字段。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BookSort {
    Title,
    Author,
    ImportedAt,
}

/// 排序方向；只影响主排序字段。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Asc,
    Desc,
}

/// 书库列表查询参数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListBooksQuery {
    /// 书名/作者包含匹配文本；`None` 或空白表示不过滤。
    pub query: Option<String>,
    pub sort: BookSort,
    pub direction: SortDirection,
}

/// 书库列表摘要；不包含原始文件路径或正文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookSummary {
    pub id: String,
    pub title: String,
    pub author: String,
    /// 封面缓存键；封面协议 URL 由 Tauri 层生成。
    pub cover_cache_key: Option<String>,
    pub imported_at: String,
    pub last_opened_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_query_with_camel_case() {
        let query = ListBooksQuery {
            query: Some("title".to_string()),
            sort: BookSort::ImportedAt,
            direction: SortDirection::Desc,
        };
        let value = serde_json::to_value(&query).expect("serialize");
        assert_eq!(value["query"], "title");
        assert_eq!(value["sort"], "importedAt");
        assert_eq!(value["direction"], "desc");
    }

    #[test]
    fn serializes_summary_with_camel_case() {
        let summary = BookSummary {
            id: "book-1".to_string(),
            title: "书名".to_string(),
            author: "作者".to_string(),
            cover_cache_key: Some("cover-key".to_string()),
            imported_at: "2026-11-14T00:00:00Z".to_string(),
            last_opened_at: None,
        };
        let value = serde_json::to_value(&summary).expect("serialize");
        assert_eq!(value["coverCacheKey"], "cover-key");
        assert_eq!(value["importedAt"], "2026-11-14T00:00:00Z");
        assert!(value["lastOpenedAt"].is_null());

        let restored: BookSummary = serde_json::from_value(value).expect("deserialize");
        assert_eq!(restored, summary);
    }

    #[test]
    fn sort_enums_round_trip() {
        for sort in [BookSort::Title, BookSort::Author, BookSort::ImportedAt] {
            let json = serde_json::to_string(&sort).expect("serialize");
            let restored: BookSort = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(restored, sort);
        }
        for direction in [SortDirection::Asc, SortDirection::Desc] {
            let json = serde_json::to_string(&direction).expect("serialize");
            let restored: SortDirection = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(restored, direction);
        }
    }
}
