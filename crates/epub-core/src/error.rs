//! EPUB 解析的稳定错误类别。

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ResourcePath;

/// 解析器内部错误类别，序列化为 camelCase。
///
/// 后续 Tauri 命令层据此完整映射 SDS 的 `AppError`；本 crate 不定义跨 IPC 错误模型。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EpubErrorCode {
    /// ZIP 归档无法读取或已损坏。
    InvalidZip,
    /// 缺少 `META-INF/container.xml`。
    MissingContainer,
    /// 缺少 OPF 包文档。
    MissingOpf,
    /// OPF 包文档结构无效。
    InvalidOpf,
    /// XML 解析失败。
    XmlParseFailed,
    /// 正文、图片、样式或目录资源被加密，或书籍带 DRM。
    EncryptedContent,
    /// 书籍声明固定版式。
    FixedLayout,
    /// 书籍声明竖排或从右到左阅读顺序。
    VerticalWriting,
    /// 书籍依赖原书脚本才能正确显示。
    ScriptDependency,
    /// 资源路径逃出归档根目录或格式不安全。
    UnsafeResourcePath,
    /// 超过条目数量、单条目大小或总解压大小限制。
    LimitExceeded,
    /// 文件不存在。
    FileNotFound,
    /// 读取文件时发生 I/O 错误。
    IoError,
}

impl EpubErrorCode {
    /// 稳定的 camelCase code 字符串。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidZip => "invalidZip",
            Self::MissingContainer => "missingContainer",
            Self::MissingOpf => "missingOpf",
            Self::InvalidOpf => "invalidOpf",
            Self::XmlParseFailed => "xmlParseFailed",
            Self::EncryptedContent => "encryptedContent",
            Self::FixedLayout => "fixedLayout",
            Self::VerticalWriting => "verticalWriting",
            Self::ScriptDependency => "scriptDependency",
            Self::UnsafeResourcePath => "unsafeResourcePath",
            Self::LimitExceeded => "limitExceeded",
            Self::FileNotFound => "fileNotFound",
            Self::IoError => "ioError",
        }
    }

    /// 面向用户的中文诊断说明。
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidZip => "ZIP 归档无效或已损坏",
            Self::MissingContainer => "缺少 META-INF/container.xml",
            Self::MissingOpf => "缺少 OPF 包文档",
            Self::InvalidOpf => "OPF 包文档无效",
            Self::XmlParseFailed => "XML 解析失败",
            Self::EncryptedContent => "书籍包含加密正文或 DRM",
            Self::FixedLayout => "不支持固定版式 EPUB",
            Self::VerticalWriting => "不支持竖排或从右到左排版",
            Self::ScriptDependency => "书籍依赖脚本才能显示",
            Self::UnsafeResourcePath => "资源路径不安全",
            Self::LimitExceeded => "超过 ZIP 条目或解压大小限制",
            Self::FileNotFound => "书籍文件不存在",
            Self::IoError => "读取书籍文件失败",
        }
    }
}

impl fmt::Display for EpubErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// 不泄漏 ZIP/XML 库错误类型的稳定错误。
///
/// 错误只携带稳定 code 和可选的归档内规范化 [`ResourcePath`]；结构位置
/// （如 `META-INF/container.xml`）也用 `ResourcePath` 表示，不保存任意字符串或
/// 用户绝对路径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpubError {
    code: EpubErrorCode,
    resource_path: Option<ResourcePath>,
}

impl EpubError {
    /// 由错误类别创建错误。
    #[must_use]
    pub const fn new(code: EpubErrorCode) -> Self {
        Self {
            code,
            resource_path: None,
        }
    }

    /// 读取文件时的 I/O 错误；不保留 `std::io::Error` 以免泄漏绝对路径。
    #[must_use]
    pub const fn io() -> Self {
        Self::new(EpubErrorCode::IoError)
    }

    /// 附加 EPUB 内的相对资源路径或结构位置。
    #[must_use]
    pub fn with_resource_path(mut self, path: ResourcePath) -> Self {
        self.resource_path = Some(path);
        self
    }

    /// 返回稳定的错误类别。
    #[must_use]
    pub const fn code(&self) -> EpubErrorCode {
        self.code
    }

    /// 返回关联的 EPUB 内资源路径或结构位置。
    #[must_use]
    pub fn resource_path(&self) -> Option<&ResourcePath> {
        self.resource_path.as_ref()
    }
}

impl fmt::Display for EpubError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "EPUB 错误（{}）：{}",
            self.code.as_str(),
            self.code.message()
        )?;
        if let Some(path) = &self.resource_path {
            write!(formatter, "；资源 {path}")?;
        }
        Ok(())
    }
}

impl std::error::Error for EpubError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_have_stable_strings() {
        assert_eq!(EpubErrorCode::InvalidZip.as_str(), "invalidZip");
        assert_eq!(EpubErrorCode::MissingContainer.as_str(), "missingContainer");
        assert_eq!(EpubErrorCode::MissingOpf.as_str(), "missingOpf");
        assert_eq!(EpubErrorCode::InvalidOpf.as_str(), "invalidOpf");
        assert_eq!(EpubErrorCode::XmlParseFailed.as_str(), "xmlParseFailed");
        assert_eq!(EpubErrorCode::ScriptDependency.as_str(), "scriptDependency");
        assert_eq!(EpubErrorCode::LimitExceeded.as_str(), "limitExceeded");
        assert_eq!(EpubErrorCode::IoError.as_str(), "ioError");
    }

    #[test]
    fn display_uses_relative_structure_path_only() {
        let error = EpubError::new(EpubErrorCode::MissingContainer)
            .with_resource_path(ResourcePath::new("META-INF/container.xml").expect("路径有效"));

        assert_eq!(error.code(), EpubErrorCode::MissingContainer);
        assert_eq!(
            error.resource_path().map(ResourcePath::as_str),
            Some("META-INF/container.xml")
        );
        let message = error.to_string();
        assert!(message.contains("missingContainer"));
        assert!(message.contains("META-INF/container.xml"));
        assert!(!message.contains("/Users/"));
    }

    #[test]
    fn io_error_has_no_resource() {
        let error = EpubError::io();
        assert_eq!(error.code(), EpubErrorCode::IoError);
        assert!(error.resource_path().is_none());
        assert!(error.to_string().contains("读取书籍文件失败"));
    }
}
