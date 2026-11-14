//! 版本化 EPUB 领域模型。
//!
//! 模型只使用 `serde` 与领域类型，不依赖 Tauri/UI。字段在 JSON 中为 camelCase，
//! 以便后续直接作为 Tauri DTO 的输入。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::path::ResourcePath;

/// 当前 `ParsedBook` schema 版本。持久化或缓存前必须写入并校验。
pub const PARSED_BOOK_SCHEMA_VERSION: u32 = 1;

/// 书籍元数据。缺失字段由调用方按 SRS 规则回退，不在模型中写死占位值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BookMetadata {
    /// 书名；缺失时为 `None`。
    pub title: Option<String>,
    /// 作者列表，保持 OPF 中的顺序。
    pub authors: Vec<String>,
    /// 语言标签。
    pub language: Option<String>,
    /// 出版者。
    pub publisher: Option<String>,
}

/// OPF manifest 中的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestItem {
    /// OPF manifest 的 `id`。
    pub id: String,
    /// 相对 OPF 所在目录、已规范化的归档内路径。
    pub href: ResourcePath,
    /// MIME 类型。
    pub media_type: String,
    /// manifest `properties` 拆分的值，例如 `nav`。
    pub properties: Vec<String>,
}

/// 阅读顺序中的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpineItem {
    /// 从 0 开始的阅读顺序索引。
    pub index: usize,
    /// 指向 manifest 的 `idref`。
    pub idref: String,
    /// 解析后的资源路径。
    pub href: ResourcePath,
    /// `linear` 属性；缺省为 true。
    pub linear: bool,
}

/// 目录节点，允许任意层级嵌套。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TocNode {
    /// 目录项标识。
    pub id: String,
    /// 显示标签。
    pub label: String,
    /// 原始 `href` 文本；URL 解码与定位在 M1C 处理。
    pub href: String,
    /// 子目录项。
    pub children: Vec<TocNode>,
}

/// 封面引用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverReference {
    /// 封面图片在归档内的路径。
    pub href: ResourcePath,
    /// 封面图片 MIME 类型。
    pub media_type: String,
}

/// 非致命解析警告类别。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EpubWarningCode {
    /// 书名、作者等元数据缺失，已使用回退值。
    MissingMetadata,
    /// 未找到可用封面。
    MissingCover,
    /// 已阻断远程资源引用。
    RemoteResourceBlocked,
    /// 字体被加密，已回退到系统字体。
    EncryptedFontFallback,
    /// 标记结构部分降级。
    DegradedMarkup,
}

impl EpubWarningCode {
    /// 稳定的 camelCase code 字符串。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingMetadata => "missingMetadata",
            Self::MissingCover => "missingCover",
            Self::RemoteResourceBlocked => "remoteResourceBlocked",
            Self::EncryptedFontFallback => "encryptedFontFallback",
            Self::DegradedMarkup => "degradedMarkup",
        }
    }
}

/// 解析警告；资源路径受 [`ResourcePath`] 约束，只允许 EPUB 内的相对路径。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpubWarning {
    /// 警告类别。
    pub code: EpubWarningCode,
    /// 相关资源路径；无特定资源时为 `None`。
    pub resource_href: Option<ResourcePath>,
}

/// 版本化解析结果。
///
/// 构造必须经过 [`ParsedBook::new`]，以写入当前 schema 版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedBook {
    /// schema 版本，当前为 [`PARSED_BOOK_SCHEMA_VERSION`]。
    pub schema_version: u32,
    /// OPF 包文档在归档内的路径。
    pub package_path: ResourcePath,
    /// 元数据。
    pub metadata: BookMetadata,
    /// manifest，按 id 排序以保证序列化稳定。
    pub manifest: BTreeMap<String, ManifestItem>,
    /// 阅读顺序。
    pub spine: Vec<SpineItem>,
    /// 目录。
    pub toc: Vec<TocNode>,
    /// 封面引用。
    pub cover: Option<CoverReference>,
    /// 章节权重；解析完成后与 `spine` 等长，由 M1C 填充。
    pub chapter_weights: Vec<u32>,
    /// 非致命警告。
    pub warnings: Vec<EpubWarning>,
}

impl ParsedBook {
    /// 创建带当前 schema 版本的空解析结果。
    #[must_use]
    pub fn new(package_path: ResourcePath) -> Self {
        Self {
            schema_version: PARSED_BOOK_SCHEMA_VERSION,
            package_path,
            metadata: BookMetadata::default(),
            manifest: BTreeMap::new(),
            spine: Vec::new(),
            toc: Vec::new(),
            cover: None,
            chapter_weights: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// 当前 schema 版本是否受支持。
    #[must_use]
    pub fn has_supported_schema(&self) -> bool {
        self.schema_version == PARSED_BOOK_SCHEMA_VERSION
    }
}
