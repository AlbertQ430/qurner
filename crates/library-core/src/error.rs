//! 书库用例错误模型。
//!
//! 不包含绝对路径、rbook 或 SQLite 类型；只保留稳定 code 与可枚举原因。

use std::fmt;

use epub_core::EpubErrorCode;

/// 存储错误的稳定类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageErrorKind {
    OpenFailed,
    MigrationFailed,
    QueryFailed,
    TransactionFailed,
    Busy,
    Conflict,
    UnsupportedSchemaVersion,
    ThreadUnavailable,
}

impl From<storage_core::StorageError> for StorageErrorKind {
    fn from(error: storage_core::StorageError) -> Self {
        match error {
            storage_core::StorageError::OpenFailed => Self::OpenFailed,
            storage_core::StorageError::MigrationFailed => Self::MigrationFailed,
            storage_core::StorageError::QueryFailed => Self::QueryFailed,
            storage_core::StorageError::TransactionFailed => Self::TransactionFailed,
            storage_core::StorageError::Busy => Self::Busy,
            storage_core::StorageError::Conflict => Self::Conflict,
            storage_core::StorageError::UnsupportedSchemaVersion => Self::UnsupportedSchemaVersion,
            storage_core::StorageError::ThreadUnavailable => Self::ThreadUnavailable,
        }
    }
}

impl StorageErrorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenFailed => "openFailed",
            Self::MigrationFailed => "migrationFailed",
            Self::QueryFailed => "queryFailed",
            Self::TransactionFailed => "transactionFailed",
            Self::Busy => "busy",
            Self::Conflict => "conflict",
            Self::UnsupportedSchemaVersion => "unsupportedSchemaVersion",
            Self::ThreadUnavailable => "threadUnavailable",
        }
    }
}

/// 导入与软移除恢复的错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookError {
    FileNotFound,
    FileUnreadable,
    InvalidInput,
    BookNotFound,
    BookUnavailable,
    Epub(EpubErrorCode),
    SourceChanged,
    Storage(StorageErrorKind),
    Internal,
}

impl BookError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FileNotFound => "fileNotFound",
            Self::FileUnreadable => "fileUnreadable",
            Self::InvalidInput => "invalidInput",
            Self::BookNotFound => "bookNotFound",
            Self::BookUnavailable => "bookUnavailable",
            Self::Epub(_) => "epub",
            Self::SourceChanged => "sourceChanged",
            Self::Storage(_) => "storage",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for BookError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileNotFound => formatter.write_str("书籍文件不存在"),
            Self::FileUnreadable => formatter.write_str("书籍文件无法读取"),
            Self::InvalidInput => formatter.write_str("导入参数无效"),
            Self::BookNotFound => formatter.write_str("未找到该书籍"),
            Self::BookUnavailable => formatter.write_str("书籍文件当前不可用"),
            Self::Epub(code) => write!(formatter, "EPUB 解析失败（{}）", code.as_str()),
            Self::SourceChanged => formatter.write_str("源文件在解析期间发生变化"),
            Self::Storage(kind) => write!(formatter, "存储错误（{}）", kind.as_str()),
            Self::Internal => formatter.write_str("内部错误"),
        }
    }
}

impl std::error::Error for BookError {}
