//! 版本化阅读 Locator 与阅读模式模型。
//!
//! 字段使用 camelCase；构造与反序列化都执行同一套校验，保证 JSON round-trip 安全。

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 当前 Locator schema 版本。
pub const READER_LOCATOR_VERSION: u32 = 1;
/// JavaScript 安全整数上限（2^53 - 1）。
pub const MAX_TEXT_OFFSET: u64 = 9_007_199_254_740_991;
/// quote.exact 的 UTF-16 code unit 上限。
pub const MAX_QUOTE_EXACT_UTF16: usize = 128;
/// quote.prefix / quote.suffix 的 UTF-16 code unit 上限。
pub const MAX_QUOTE_CONTEXT_UTF16: usize = 32;

/// 阅读模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReaderMode {
    Paged,
    Scrolling,
}

/// 文本锚点上下文；exact 为锚点文本，prefix/suffix 为前后文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderQuoteContext {
    pub exact: String,
    pub prefix: String,
    pub suffix: String,
}

/// Locator 校验错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReaderLocatorError {
    #[error("不支持的 Locator 版本")]
    UnsupportedVersion,
    #[error("spineHref 必须是 EPUB 根内的相对路径")]
    InvalidSpineHref,
    #[error("textOffset 超出安全整数上限")]
    TextOffsetTooLarge,
    #[error("intraSpineProgression 必须是 0..=1 的有限数")]
    InvalidProgression,
    #[error("quote 字段超过 UTF-16 长度上限")]
    QuoteTooLong,
}

/// 版本化阅读位置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "RawReaderLocator")]
pub struct ReaderLocator {
    pub version: u32,
    pub spine_index: u32,
    pub spine_href: String,
    pub text_offset: u64,
    pub intra_spine_progression: f64,
    pub quote: ReaderQuoteContext,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawReaderLocator {
    version: u32,
    spine_index: u32,
    spine_href: String,
    text_offset: u64,
    intra_spine_progression: f64,
    quote: ReaderQuoteContext,
}

impl ReaderLocator {
    /// 构造并校验 Locator。
    pub fn new(
        spine_index: u32,
        spine_href: impl Into<String>,
        text_offset: u64,
        intra_spine_progression: f64,
        quote: ReaderQuoteContext,
    ) -> Result<Self, ReaderLocatorError> {
        let href = spine_href.into();
        if text_offset > MAX_TEXT_OFFSET {
            return Err(ReaderLocatorError::TextOffsetTooLarge);
        }
        if !intra_spine_progression.is_finite() || !(0.0..=1.0).contains(&intra_spine_progression) {
            return Err(ReaderLocatorError::InvalidProgression);
        }
        validate_spine_href(&href)?;
        if quote.exact.encode_utf16().count() > MAX_QUOTE_EXACT_UTF16
            || quote.prefix.encode_utf16().count() > MAX_QUOTE_CONTEXT_UTF16
            || quote.suffix.encode_utf16().count() > MAX_QUOTE_CONTEXT_UTF16
        {
            return Err(ReaderLocatorError::QuoteTooLong);
        }
        Ok(Self {
            version: READER_LOCATOR_VERSION,
            spine_index,
            spine_href: href,
            text_offset,
            intra_spine_progression,
            quote,
        })
    }
}

impl TryFrom<RawReaderLocator> for ReaderLocator {
    type Error = ReaderLocatorError;

    fn try_from(raw: RawReaderLocator) -> Result<Self, Self::Error> {
        if raw.version != READER_LOCATOR_VERSION {
            return Err(ReaderLocatorError::UnsupportedVersion);
        }
        ReaderLocator::new(
            raw.spine_index,
            raw.spine_href,
            raw.text_offset,
            raw.intra_spine_progression,
            raw.quote,
        )
    }
}

fn validate_spine_href(href: &str) -> Result<(), ReaderLocatorError> {
    if href.is_empty() || href.contains('\0') || href.contains('\\') || href.starts_with('/') {
        return Err(ReaderLocatorError::InvalidSpineHref);
    }
    for segment in href.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(ReaderLocatorError::InvalidSpineHref);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quote(exact: &str, prefix: &str, suffix: &str) -> ReaderQuoteContext {
        ReaderQuoteContext {
            exact: exact.to_string(),
            prefix: prefix.to_string(),
            suffix: suffix.to_string(),
        }
    }

    fn locator() -> ReaderLocator {
        ReaderLocator::new(
            3,
            "OEBPS/chapter1.xhtml",
            42,
            0.25,
            quote("锚点", "前", "后"),
        )
        .expect("valid locator")
    }

    #[test]
    fn serializes_camel_case_and_round_trips() {
        let value = serde_json::to_value(locator()).expect("serialize");
        assert_eq!(value["version"], 1);
        assert_eq!(value["spineIndex"], 3);
        assert_eq!(value["spineHref"], "OEBPS/chapter1.xhtml");
        assert_eq!(value["textOffset"], 42);
        assert_eq!(value["intraSpineProgression"], 0.25);
        assert_eq!(value["quote"]["exact"], "锚点");
        assert_eq!(value["quote"]["prefix"], "前");
        assert_eq!(value["quote"]["suffix"], "后");

        let restored: ReaderLocator = serde_json::from_value(value).expect("deserialize");
        assert_eq!(restored, locator());
    }

    #[test]
    fn rejects_unsafe_spine_href() {
        for href in [
            "",
            "/abs/chapter.xhtml",
            "../chapter.xhtml",
            "OEBPS/../chapter.xhtml",
            "OEBPS\\chapter.xhtml",
            "OEBPS//chapter.xhtml",
            "OEBPS/\0chapter.xhtml",
        ] {
            assert!(
                ReaderLocator::new(0, href, 0, 0.0, quote("", "", "")).is_err(),
                "{href:?}"
            );
        }
    }

    #[test]
    fn rejects_unknown_version_and_invalid_numbers() {
        let json = serde_json::json!({
            "version": 2,
            "spineIndex": 0,
            "spineHref": "chapter.xhtml",
            "textOffset": 0,
            "intraSpineProgression": 0.0,
            "quote": { "exact": "", "prefix": "", "suffix": "" }
        });
        let error = serde_json::from_value::<ReaderLocator>(json).expect_err("version");
        assert_eq!(error.to_string(), "不支持的 Locator 版本");

        assert_eq!(
            ReaderLocator::new(
                0,
                "chapter.xhtml",
                MAX_TEXT_OFFSET + 1,
                0.0,
                quote("", "", "")
            )
            .expect_err("offset"),
            ReaderLocatorError::TextOffsetTooLarge
        );

        for progression in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            assert_eq!(
                ReaderLocator::new(0, "chapter.xhtml", 0, progression, quote("", "", ""))
                    .expect_err("progression"),
                ReaderLocatorError::InvalidProgression
            );
        }
    }

    #[test]
    fn enforces_utf16_quote_limits() {
        // 64 个 emoji = 128 个 UTF-16 code unit，正好允许。
        let emoji = "😀".repeat(64);
        assert!(ReaderLocator::new(0, "chapter.xhtml", 0, 0.0, quote(&emoji, "", "")).is_ok());
        let too_long = "😀".repeat(65);
        assert_eq!(
            ReaderLocator::new(0, "chapter.xhtml", 0, 0.0, quote(&too_long, "", ""))
                .expect_err("emoji"),
            ReaderLocatorError::QuoteTooLong
        );

        let context = "a".repeat(MAX_QUOTE_CONTEXT_UTF16 + 1);
        assert_eq!(
            ReaderLocator::new(0, "chapter.xhtml", 0, 0.0, quote("", &context, ""))
                .expect_err("prefix"),
            ReaderLocatorError::QuoteTooLong
        );

        // 中文每个字符一个 UTF-16 code unit。
        let chinese = "中文".repeat(64);
        assert!(ReaderLocator::new(0, "chapter.xhtml", 0, 0.0, quote(&chinese, "", "")).is_ok());
    }

    #[test]
    fn short_and_empty_quotes_are_allowed() {
        assert!(ReaderLocator::new(0, "短.xhtml", 0, 1.0, quote("", "", "")).is_ok());
        assert!(ReaderLocator::new(0, "短.xhtml", 0, 0.0, quote("短", "", "")).is_ok());
    }
}
