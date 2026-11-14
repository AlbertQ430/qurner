//! 书籍 ID、时间、搜索键与排序键生成。

use std::path::Path;

use icu_collator::Collator;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::BookError;

/// 小写标准 UUID v4。
pub(crate) fn new_book_id() -> String {
    Uuid::new_v4().to_string()
}

/// UTC RFC 3339，精确到秒。
pub(crate) fn now_rfc3339() -> String {
    let now = OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .unwrap_or_else(|_| OffsetDateTime::now_utc());
    now.format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

/// 生成大小写不敏感的搜索键。
pub(crate) fn search_key(text: &str) -> String {
    text.to_lowercase()
}

/// 目录/标题展示字段：元数据 → 文件 stem → 固定回退。
pub(crate) fn display_title(metadata_title: Option<&str>, path: &Path) -> String {
    if let Some(title) = metadata_title
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return title.to_string();
    }

    if let Some(stem) = path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return stem.to_string();
    }

    "未命名书籍".to_string()
}

/// 作者展示字段：按顺序用 `, ` 连接；无作者时固定回退。
pub(crate) fn display_author(authors: &[String]) -> String {
    let authors: Vec<&str> = authors
        .iter()
        .map(|author| author.trim())
        .filter(|author| !author.is_empty())
        .collect();
    if authors.is_empty() {
        "未知作者".to_string()
    } else {
        authors.join(", ")
    }
}

/// 使用 ICU 默认排序配置生成标题/作者排序键。
pub(crate) fn sort_keys(title: &str, author: &str) -> Result<(Vec<u8>, Vec<u8>), BookError> {
    let collator = Collator::try_new(Default::default(), Default::default())
        .map_err(|_| BookError::Internal)?;

    let mut title_key = Vec::new();
    collator
        .write_sort_key_to(title, &mut title_key)
        .map_err(|_| BookError::Internal)?;

    let mut author_key = Vec::new();
    collator
        .write_sort_key_to(author, &mut author_key)
        .map_err(|_| BookError::Internal)?;

    Ok((title_key, author_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_rfc3339_with_second_precision() {
        let value = now_rfc3339();
        assert!(value.ends_with('Z'), "{value}");
        assert_eq!(value.len(), 20, "{value}");
        assert_eq!(&value[4..5], "-");
        assert_eq!(&value[10..11], "T");
    }

    #[test]
    fn falls_back_for_missing_metadata() {
        assert_eq!(
            display_title(None, Path::new("/tmp/我的 书.epub")),
            "我的 书"
        );
        assert_eq!(display_title(Some("   "), Path::new("/")), "未命名书籍");
        assert_eq!(display_author(&[]), "未知作者");
        assert_eq!(
            display_author(&["甲".to_string(), "乙".to_string()]),
            "甲, 乙"
        );
    }

    #[test]
    fn search_keys_are_lowercase() {
        assert_eq!(search_key("Hello 世界"), "hello 世界");
    }

    #[test]
    fn sort_keys_are_deterministic() {
        let (title_a, author_a) = sort_keys("中文标题", "作者").expect("key");
        let (title_b, author_b) = sort_keys("中文标题", "作者").expect("key");
        assert_eq!(title_a, title_b);
        assert_eq!(author_a, author_b);
        assert!(!title_a.is_empty());

        let (title_c, _) = sort_keys("English Title", "作者").expect("key");
        assert_ne!(title_a, title_c);
    }
}
