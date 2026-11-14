//! Qurner 存储核心。
//!
//! 提供 SQLite schema v1、事务迁移和专用写线程；不包含 EPUB 导入、
//! 书库查询或 Tauri 命令。

#![forbid(unsafe_code)]

mod books;
mod error;
mod records;
mod schema;
mod service;

pub use error::StorageError;
pub use records::{ExistingBookRecord, NewBookRecord, ReactivateOutcome};
pub use service::StorageService;

/// crate 名称，用于冒烟测试确认 workspace 能编译并测试本包。
pub const CRATE_NAME: &str = "storage-core";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_is_wired_into_workspace() {
        assert_eq!(CRATE_NAME, "storage-core");
    }
}
