//! 跨 IPC 的稳定错误模型（SDS 9.1）。

use std::collections::BTreeMap;

use epub_core::EpubErrorCode;
use library_core::BookError;
use serde::{Deserialize, Serialize};

/// 跨 IPC 的 14 个稳定错误 code。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppErrorCode {
    #[serde(rename = "E_INVALID_INPUT")]
    InvalidInput,
    #[serde(rename = "E_FILE_NOT_FOUND")]
    FileNotFound,
    #[serde(rename = "E_FILE_UNREADABLE")]
    FileUnreadable,
    #[serde(rename = "E_EPUB_INVALID")]
    EpubInvalid,
    #[serde(rename = "E_EPUB_ENCRYPTED")]
    EpubEncrypted,
    #[serde(rename = "E_EPUB_FIXED_LAYOUT")]
    EpubFixedLayout,
    #[serde(rename = "E_EPUB_VERTICAL")]
    EpubVertical,
    #[serde(rename = "E_EPUB_SCRIPT_REQUIRED")]
    EpubScriptRequired,
    #[serde(rename = "E_BOOK_NOT_FOUND")]
    BookNotFound,
    #[serde(rename = "E_BOOK_UNAVAILABLE")]
    BookUnavailable,
    #[serde(rename = "E_SOURCE_CHANGED")]
    SourceChanged,
    #[serde(rename = "E_SESSION_INVALID")]
    SessionInvalid,
    #[serde(rename = "E_STORAGE")]
    Storage,
    #[serde(rename = "E_INTERNAL")]
    Internal,
}

impl AppErrorCode {
    /// 稳定 code 字符串。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "E_INVALID_INPUT",
            Self::FileNotFound => "E_FILE_NOT_FOUND",
            Self::FileUnreadable => "E_FILE_UNREADABLE",
            Self::EpubInvalid => "E_EPUB_INVALID",
            Self::EpubEncrypted => "E_EPUB_ENCRYPTED",
            Self::EpubFixedLayout => "E_EPUB_FIXED_LAYOUT",
            Self::EpubVertical => "E_EPUB_VERTICAL",
            Self::EpubScriptRequired => "E_EPUB_SCRIPT_REQUIRED",
            Self::BookNotFound => "E_BOOK_NOT_FOUND",
            Self::BookUnavailable => "E_BOOK_UNAVAILABLE",
            Self::SourceChanged => "E_SOURCE_CHANGED",
            Self::SessionInvalid => "E_SESSION_INVALID",
            Self::Storage => "E_STORAGE",
            Self::Internal => "E_INTERNAL",
        }
    }

    const fn message(self) -> &'static str {
        match self {
            Self::InvalidInput => "请求参数无效，请检查后重试。",
            Self::FileNotFound => "书籍文件不存在，请重新选择文件。",
            Self::FileUnreadable => "书籍文件无法读取，请检查文件权限后重试。",
            Self::EpubInvalid => "EPUB 文件结构无效或已损坏，请更换文件。",
            Self::EpubEncrypted => "该书受 DRM 或加密保护，暂不支持。",
            Self::EpubFixedLayout => "该书为固定版式，暂不支持。",
            Self::EpubVertical => "该书为竖排或从右到左排版，暂不支持。",
            Self::EpubScriptRequired => "该书依赖脚本，暂不支持。",
            Self::BookNotFound => "未找到该书籍记录。",
            Self::BookUnavailable => "书籍当前不可用。",
            Self::SourceChanged => "源文件在导入过程中发生变化，请重试。",
            Self::SessionInvalid => "阅读会话已失效。",
            Self::Storage => "本地数据库不可用，请重启应用后重试。",
            Self::Internal => "发生内部错误，请重试。",
        }
    }
}

/// `details` 允许的标量值：字符串、数字或布尔。
///
/// 用 untagged 枚举限制跨 IPC 契约（SDS 9.1），对象、数组和 null 会被拒绝。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DetailValue {
    Bool(bool),
    Number(serde_json::Number),
    String(String),
}

/// 前端可见的稳定错误对象；`details` 缺省时从 JSON 中省略。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<BTreeMap<String, DetailValue>>,
}

impl AppError {
    #[must_use]
    pub fn new(code: AppErrorCode) -> Self {
        Self {
            code,
            message: code.message().to_string(),
            details: None,
        }
    }

    #[must_use]
    pub fn invalid_input() -> Self {
        Self::new(AppErrorCode::InvalidInput)
    }

    #[must_use]
    pub fn storage() -> Self {
        Self::new(AppErrorCode::Storage)
    }

    #[must_use]
    pub fn from_book_error(error: BookError) -> Self {
        match error {
            BookError::InvalidInput => Self::new(AppErrorCode::InvalidInput),
            BookError::FileNotFound => Self::new(AppErrorCode::FileNotFound),
            BookError::FileUnreadable => Self::new(AppErrorCode::FileUnreadable),
            BookError::Epub(code) => Self::from_epub_code(code),
            BookError::SourceChanged => Self::new(AppErrorCode::SourceChanged),
            BookError::Storage(_) => Self::new(AppErrorCode::Storage),
            BookError::Internal => Self::new(AppErrorCode::Internal),
        }
    }

    #[must_use]
    pub fn from_epub_code(code: EpubErrorCode) -> Self {
        let app_code = match code {
            EpubErrorCode::FileNotFound => AppErrorCode::FileNotFound,
            EpubErrorCode::IoError => AppErrorCode::FileUnreadable,
            EpubErrorCode::EncryptedContent => AppErrorCode::EpubEncrypted,
            EpubErrorCode::FixedLayout => AppErrorCode::EpubFixedLayout,
            EpubErrorCode::VerticalWriting => AppErrorCode::EpubVertical,
            EpubErrorCode::ScriptDependency => AppErrorCode::EpubScriptRequired,
            _ => AppErrorCode::EpubInvalid,
        };
        Self::new(app_code)
    }

    #[must_use]
    pub fn with_details(mut self, details: BTreeMap<String, DetailValue>) -> Self {
        self.details = Some(details);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_CODES: [AppErrorCode; 14] = [
        AppErrorCode::InvalidInput,
        AppErrorCode::FileNotFound,
        AppErrorCode::FileUnreadable,
        AppErrorCode::EpubInvalid,
        AppErrorCode::EpubEncrypted,
        AppErrorCode::EpubFixedLayout,
        AppErrorCode::EpubVertical,
        AppErrorCode::EpubScriptRequired,
        AppErrorCode::BookNotFound,
        AppErrorCode::BookUnavailable,
        AppErrorCode::SourceChanged,
        AppErrorCode::SessionInvalid,
        AppErrorCode::Storage,
        AppErrorCode::Internal,
    ];

    #[test]
    fn serializes_all_fourteen_codes() {
        let mut seen = Vec::new();
        for code in ALL_CODES {
            let json = serde_json::to_string(&code).expect("serialize");
            assert_eq!(json, format!("\"{}\"", code.as_str()));
            seen.push(json);
        }
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 14);
    }

    #[test]
    fn error_round_trips_and_omits_empty_details() {
        let error = AppError::new(AppErrorCode::FileNotFound);
        let value = serde_json::to_value(&error).expect("serialize");
        assert_eq!(value["code"], "E_FILE_NOT_FOUND");
        assert!(value.get("details").is_none());
        assert!(!error.message.contains('/'));

        let restored: AppError = serde_json::from_value(value).expect("deserialize");
        assert_eq!(restored, error);
    }

    #[test]
    fn serializes_details_when_present() {
        let mut details = BTreeMap::new();
        details.insert(
            "stage".to_string(),
            DetailValue::String("import".to_string()),
        );
        let error = AppError::new(AppErrorCode::Internal).with_details(details);
        let value = serde_json::to_value(&error).expect("serialize");
        assert_eq!(value["details"]["stage"], "import");
        assert!(value.get("details").is_some());
    }

    #[test]
    fn details_accept_only_scalar_values() {
        let valid = serde_json::json!({
            "code": "E_INTERNAL",
            "message": "内部错误",
            "details": { "text": "value", "number": 1.5, "flag": true }
        });
        let error: AppError = serde_json::from_value(valid).expect("标量 details");
        let details = error.details.as_ref().expect("details");
        assert_eq!(details.len(), 3);

        let value = serde_json::to_value(&error).expect("serialize");
        assert_eq!(value["details"]["text"], "value");
        assert_eq!(value["details"]["number"], 1.5);
        assert_eq!(value["details"]["flag"], true);

        for invalid in [
            serde_json::json!({
                "code": "E_INTERNAL",
                "message": "内部错误",
                "details": { "bad": { "nested": 1 } }
            }),
            serde_json::json!({
                "code": "E_INTERNAL",
                "message": "内部错误",
                "details": { "bad": [1, 2] }
            }),
            serde_json::json!({
                "code": "E_INTERNAL",
                "message": "内部错误",
                "details": { "bad": null }
            }),
        ] {
            assert!(
                serde_json::from_value::<AppError>(invalid).is_err(),
                "details 不应接受对象、数组或 null"
            );
        }
    }

    #[test]
    fn maps_book_errors() {
        assert_eq!(
            AppError::from_book_error(BookError::InvalidInput).code,
            AppErrorCode::InvalidInput
        );
        assert_eq!(
            AppError::from_book_error(BookError::FileNotFound).code,
            AppErrorCode::FileNotFound
        );
        assert_eq!(
            AppError::from_book_error(BookError::FileUnreadable).code,
            AppErrorCode::FileUnreadable
        );
        assert_eq!(
            AppError::from_book_error(BookError::SourceChanged).code,
            AppErrorCode::SourceChanged
        );
        assert_eq!(
            AppError::from_book_error(BookError::Storage(
                library_core::StorageErrorKind::QueryFailed
            ))
            .code,
            AppErrorCode::Storage
        );
        assert_eq!(
            AppError::from_book_error(BookError::Internal).code,
            AppErrorCode::Internal
        );
    }

    #[test]
    fn maps_every_epub_error_code() {
        let cases = [
            (EpubErrorCode::FileNotFound, AppErrorCode::FileNotFound),
            (EpubErrorCode::IoError, AppErrorCode::FileUnreadable),
            (EpubErrorCode::EncryptedContent, AppErrorCode::EpubEncrypted),
            (EpubErrorCode::FixedLayout, AppErrorCode::EpubFixedLayout),
            (EpubErrorCode::VerticalWriting, AppErrorCode::EpubVertical),
            (
                EpubErrorCode::ScriptDependency,
                AppErrorCode::EpubScriptRequired,
            ),
            (EpubErrorCode::InvalidZip, AppErrorCode::EpubInvalid),
            (EpubErrorCode::MissingContainer, AppErrorCode::EpubInvalid),
            (EpubErrorCode::MissingOpf, AppErrorCode::EpubInvalid),
            (EpubErrorCode::InvalidOpf, AppErrorCode::EpubInvalid),
            (EpubErrorCode::XmlParseFailed, AppErrorCode::EpubInvalid),
            (EpubErrorCode::UnsafeResourcePath, AppErrorCode::EpubInvalid),
            (EpubErrorCode::LimitExceeded, AppErrorCode::EpubInvalid),
        ];
        for (epub_code, app_code) in cases {
            let error = AppError::from_epub_code(epub_code);
            assert_eq!(error.code, app_code, "{epub_code:?}");
            assert!(!error.message.contains('/'));
        }
    }
}
