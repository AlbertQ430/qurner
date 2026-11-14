//! Qurner 阅读模型。
//!
//! 本 crate 不依赖 Tauri，保存可序列化的阅读领域类型。M1A 先提供内容指纹值类型；
//! Locator、阅读进度和设置模型在后续任务实现。

#![forbid(unsafe_code)]

pub mod fingerprint;
pub mod library;

pub use fingerprint::{BookFingerprint, FingerprintError, SHA256_BYTES, SHA256_HEX_LEN};
pub use library::{BookSort, BookSummary, ListBooksQuery, SortDirection};

/// crate 名称，用于冒烟测试确认 workspace 能编译并测试本包。
pub const CRATE_NAME: &str = "reader-model";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_is_wired_into_workspace() {
        assert_eq!(CRATE_NAME, "reader-model");
    }
}
