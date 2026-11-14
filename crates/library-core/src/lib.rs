//! Qurner 书库用例层（M2B 实现导入与软移除恢复）。

#![forbid(unsafe_code)]

mod cover_cache;
mod error;
mod keys;
mod service;

pub use cover_cache::CoverCache;
pub use error::{BookError, StorageErrorKind};
pub use service::{ImportItemResult, ImportStatus, LibraryService};

/// crate 名称，用于冒烟测试确认 workspace 能编译并测试本包。
pub const CRATE_NAME: &str = "library-core";
