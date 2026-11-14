//! 存储错误模型。
//!
//! 错误只保留稳定 code，不包含数据库绝对路径或 SQLite 原始错误文本。

use std::fmt;

/// 稳定的存储错误类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    /// 打开数据库失败。
    OpenFailed,
    /// schema 迁移失败。
    MigrationFailed,
    /// 查询执行失败。
    QueryFailed,
    /// 事务开始/提交失败。
    TransactionFailed,
    /// SQLite 忙。
    Busy,
    /// 唯一约束冲突（例如重复 fingerprint）。
    Conflict,
    /// 数据库 schema 版本高于当前支持版本。
    UnsupportedSchemaVersion,
    /// 写线程已关闭或不可用。
    ThreadUnavailable,
}

impl StorageError {
    /// 稳定的 camelCase code。
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

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OpenFailed => "打开数据库失败",
            Self::MigrationFailed => "数据库迁移失败",
            Self::QueryFailed => "数据库查询失败",
            Self::TransactionFailed => "数据库事务失败",
            Self::Busy => "数据库忙",
            Self::Conflict => "记录冲突",
            Self::UnsupportedSchemaVersion => "数据库版本不受支持",
            Self::ThreadUnavailable => "存储线程不可用",
        })
    }
}

impl std::error::Error for StorageError {}
