//! Qurner EPUB 核心。
//!
//! 提供版本化领域模型、安全路径与错误类别、流式 ZIP 预检、rbook 解析适配、
//! 目录/封面/章节权重和流式内容指纹。

#![forbid(unsafe_code)]

mod content;
mod encryption;
mod fingerprint;
mod parse;
mod precheck;

pub mod error;
pub mod model;
pub mod path;

pub use error::{EpubError, EpubErrorCode};
pub use fingerprint::fingerprint_file;
pub use model::{
    BookMetadata, CoverReference, EpubWarning, EpubWarningCode, ManifestItem,
    PARSED_BOOK_SCHEMA_VERSION, ParsedBook, SpineItem, TocNode,
};
pub use parse::parse_epub;
pub use path::{ResourcePath, ResourcePathError};

/// crate 名称，用于冒烟测试确认 workspace 能编译并测试本包。
pub const CRATE_NAME: &str = "epub-core";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_is_wired_into_workspace() {
        assert_eq!(CRATE_NAME, "epub-core");
    }
}
