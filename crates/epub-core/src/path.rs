//! EPUB 归档内的安全相对资源路径。

use std::fmt;
use std::str::FromStr;

use serde::de::{Error as DeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// EPUB 归档内的规范化相对 POSIX 路径。
///
/// 只接受归档内的相对路径：拒绝绝对路径、Windows 盘符、反斜杠、`..` 段、NUL
/// 和空路径；删除重复 `/` 与 `.` 段。路径段原样保留，不做 URL 解码。
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourcePath(String);

/// 资源路径被拒绝的原因。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResourcePathError {
    /// 规范化后为空。
    #[error("资源路径不能为空")]
    Empty,
    /// 以 `/` 开头。
    #[error("资源路径不能是绝对路径")]
    Absolute,
    /// 包含反斜杠。
    #[error("资源路径不能包含反斜杠")]
    Backslash,
    /// 首个路径段是 Windows 盘符。
    #[error("资源路径不能包含 Windows 盘符")]
    WindowsDrivePrefix,
    /// 包含 `..` 段。
    #[error("资源路径不能包含 .. 路径段")]
    ParentSegment,
    /// 包含 NUL 字符。
    #[error("资源路径不能包含 NUL 字符")]
    NulByte,
}

impl ResourcePath {
    /// 校验并规范化路径。
    pub fn new(path: impl AsRef<str>) -> Result<Self, ResourcePathError> {
        let raw = path.as_ref();

        if raw.contains('\0') {
            return Err(ResourcePathError::NulByte);
        }
        if raw.contains('\\') {
            return Err(ResourcePathError::Backslash);
        }
        if raw.starts_with('/') {
            return Err(ResourcePathError::Absolute);
        }

        let mut normalized = String::with_capacity(raw.len());
        let mut first_segment = true;
        for segment in raw.split('/') {
            if segment.is_empty() || segment == "." {
                continue;
            }
            if segment == ".." {
                return Err(ResourcePathError::ParentSegment);
            }
            if first_segment && is_windows_drive_prefix(segment) {
                return Err(ResourcePathError::WindowsDrivePrefix);
            }
            if !first_segment {
                normalized.push('/');
            }
            normalized.push_str(segment);
            first_segment = false;
        }

        if normalized.is_empty() {
            return Err(ResourcePathError::Empty);
        }
        Ok(Self(normalized))
    }

    /// 返回规范化后的路径文本。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourcePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Debug for ResourcePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ResourcePath")
            .field(&self.0)
            .finish()
    }
}

impl FromStr for ResourcePath {
    type Err = ResourcePathError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ResourcePath {
    type Error = ResourcePathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for ResourcePath {
    type Error = ResourcePathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl AsRef<str> for ResourcePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Serialize for ResourcePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ResourcePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ResourcePathVisitor;

        impl Visitor<'_> for ResourcePathVisitor {
            type Value = ResourcePath;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("一个 EPUB 归档内的相对资源路径")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                ResourcePath::new(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(ResourcePathVisitor)
    }
}

fn is_windows_drive_prefix(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_normalizes_relative_paths() {
        assert_eq!(
            ResourcePath::new("OEBPS/content.opf")
                .expect("普通相对路径应被接受")
                .as_str(),
            "OEBPS/content.opf"
        );
        assert_eq!(
            ResourcePath::new("OEBPS//images/./cover.jpg")
                .expect("重复斜杠和当前目录段应被规范化")
                .as_str(),
            "OEBPS/images/cover.jpg"
        );
        assert_eq!(
            ResourcePath::new("中文/章节 1.xhtml")
                .expect("Unicode 与空格文件名应保留")
                .as_str(),
            "中文/章节 1.xhtml"
        );
    }

    #[test]
    fn rejects_unsafe_paths() {
        assert_eq!(ResourcePath::new(""), Err(ResourcePathError::Empty));
        assert_eq!(
            ResourcePath::new("/etc/passwd"),
            Err(ResourcePathError::Absolute)
        );
        assert_eq!(
            ResourcePath::new("OEBPS\\chapter.xhtml"),
            Err(ResourcePathError::Backslash)
        );
        assert_eq!(
            ResourcePath::new("C:/temp/evil.xhtml"),
            Err(ResourcePathError::WindowsDrivePrefix)
        );
        assert_eq!(
            ResourcePath::new("../outside.xhtml"),
            Err(ResourcePathError::ParentSegment)
        );
        assert_eq!(
            ResourcePath::new("OEBPS/../outside.xhtml"),
            Err(ResourcePathError::ParentSegment)
        );
        assert_eq!(
            ResourcePath::new("OEBPS/chapter\0.xhtml"),
            Err(ResourcePathError::NulByte)
        );
    }

    #[test]
    fn does_not_url_decode_segments() {
        assert_eq!(
            ResourcePath::new("images/%2e%2e/cover.png")
                .expect("百分号编码保持原样")
                .as_str(),
            "images/%2e%2e/cover.png"
        );
    }

    #[test]
    fn serializes_as_string() {
        let path = ResourcePath::new("OEBPS/content.opf").expect("路径应有效");
        let json = serde_json::to_string(&path).expect("序列化应成功");
        assert_eq!(json, "\"OEBPS/content.opf\"");

        let restored: ResourcePath = serde_json::from_str(&json).expect("反序列化应成功");
        assert_eq!(restored, path);
        assert!(serde_json::from_str::<ResourcePath>("\"../escape.xhtml\"").is_err());
    }
}
