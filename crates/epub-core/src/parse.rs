//! EPUB 打开入口与 rbook 适配层。
//!
//! 预检通过后才调用 `rbook 0.7.13`；所有 `rbook` 类型都限制在本模块内部，
//! 公开 API 只返回 M1A 的领域模型和 [`EpubError`]。

use std::cell::Cell;
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::Read as _;
use std::path::Path;
use std::rc::Rc;

use lol_html::{HtmlRewriter, Settings, element, end_tag, text};
use rbook::Epub;
use rbook::ebook::element::Href;
use rbook::ebook::errors::{ArchiveError, EbookError};
use rbook::epub::toc::EpubTocEntry;

use crate::content;
use crate::encryption;
use crate::model::{
    BookMetadata, CoverReference, EpubWarning, EpubWarningCode, ManifestItem, ParsedBook,
    SpineItem, TocNode,
};
use crate::precheck::{self, MAX_ENCRYPTION_XML_BYTES};
use crate::{EpubError, EpubErrorCode, ResourcePath};

/// 安全打开 EPUB，返回 M1A 领域模型。
///
/// 顺序固定为：流式 ZIP 安全预检 → 加密资源检查 → `rbook` 解析 → 映射。
/// 本函数会按需读取 TOC/导航、封面引用和每个 spine 章节的文本以计算章节权重；
/// 不读取封面图片字节，也不执行书内脚本或访问远程资源。
pub fn parse_epub(path: &Path) -> Result<ParsedBook, EpubError> {
    let report = precheck::precheck_epub(path)?;
    if !report.has_container {
        return Err(EpubError::new(EpubErrorCode::MissingContainer));
    }

    let encrypted_fonts = if report.has_encryption {
        inspect_encryption(path)?
    } else {
        Vec::new()
    };

    let epub = Epub::open(path).map_err(map_rbook_error)?;

    if epub.spine().page_direction().is_rtl() {
        return Err(EpubError::new(EpubErrorCode::VerticalWriting));
    }

    let package_path = resource_path(epub.package().location())?;
    let mut book = ParsedBook::new(package_path);
    book.metadata = map_metadata(&epub);
    book.manifest = map_manifest(&epub)?;
    book.spine = map_spine(&epub)?;
    book.warnings = encrypted_fonts
        .into_iter()
        .map(|path| EpubWarning {
            code: EpubWarningCode::EncryptedFontFallback,
            resource_href: Some(path),
        })
        .collect();
    content::check_content(path, &book.package_path, &book.manifest, &mut book.warnings)?;
    book.toc = map_toc(&epub, &mut book.warnings)?;
    book.cover = map_cover(&epub, &mut book.warnings)?;
    book.chapter_weights = map_chapter_weights(&epub)?;
    Ok(book)
}

fn map_metadata(epub: &Epub) -> BookMetadata {
    let metadata = epub.metadata();
    BookMetadata {
        title: metadata.title().map(|title| title.value().to_string()),
        authors: metadata
            .creators()
            .map(|creator| creator.value().to_string())
            .collect(),
        language: metadata
            .language()
            .map(|language| language.value().to_string()),
        publisher: metadata
            .publishers()
            .next()
            .map(|publisher| publisher.value().to_string()),
    }
}

fn map_manifest(epub: &Epub) -> Result<BTreeMap<String, ManifestItem>, EpubError> {
    let mut manifest = BTreeMap::new();
    for entry in epub.manifest().iter() {
        let id = entry.id().to_string();
        let item = ManifestItem {
            id: id.clone(),
            href: resource_path(entry.href())?,
            media_type: entry.media_type().to_string(),
            properties: entry.properties().iter().map(str::to_string).collect(),
        };
        manifest.insert(id, item);
    }
    Ok(manifest)
}

fn map_spine(epub: &Epub) -> Result<Vec<SpineItem>, EpubError> {
    let mut spine = Vec::new();
    for (index, entry) in epub.spine().iter().enumerate() {
        let manifest_entry = entry
            .manifest_entry()
            .ok_or_else(|| EpubError::new(EpubErrorCode::InvalidOpf))?;
        spine.push(SpineItem {
            index,
            idref: manifest_entry.id().to_string(),
            href: resource_path(manifest_entry.href())?,
            linear: entry.is_linear(),
        });
    }
    Ok(spine)
}

fn resource_path(href: Href<'_>) -> Result<ResourcePath, EpubError> {
    let raw = href.as_str();
    let relative = raw.strip_prefix('/').unwrap_or(raw);
    ResourcePath::new(relative).map_err(|_| EpubError::new(EpubErrorCode::UnsafeResourcePath))
}

fn inspect_encryption(path: &Path) -> Result<Vec<ResourcePath>, EpubError> {
    let file = File::open(path).map_err(|_| EpubError::io())?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|_| EpubError::new(EpubErrorCode::InvalidZip))?;
    let entry = archive
        .by_name("META-INF/encryption.xml")
        .map_err(|_| EpubError::new(EpubErrorCode::InvalidZip))?;
    if entry.size() > MAX_ENCRYPTION_XML_BYTES {
        return Err(EpubError::new(EpubErrorCode::LimitExceeded));
    }

    let mut xml = String::new();
    entry
        .take(MAX_ENCRYPTION_XML_BYTES + 1)
        .read_to_string(&mut xml)
        .map_err(|_| EpubError::new(EpubErrorCode::XmlParseFailed))?;

    let mut encrypted_fonts = Vec::new();
    for uri in encryption::cipher_reference_uris(&xml)? {
        if encryption::is_font_path(uri.as_str()) {
            encrypted_fonts.push(uri);
        } else {
            return Err(EpubError::new(EpubErrorCode::EncryptedContent).with_resource_path(uri));
        }
    }
    Ok(encrypted_fonts)
}

fn map_toc(epub: &Epub, warnings: &mut Vec<EpubWarning>) -> Result<Vec<TocNode>, EpubError> {
    let Some(root) = epub.toc().contents() else {
        return Ok(Vec::new());
    };

    let mut allocator = TocIdAllocator::new();
    for entry in root.iter() {
        collect_toc_ids(entry, &mut allocator);
    }

    let mut nodes = Vec::new();
    for (index, entry) in root.iter().enumerate() {
        let generated_id = format!("toc-{index}");
        if let Some(node) = map_toc_entry(entry, &generated_id, &mut allocator, warnings)? {
            nodes.push(node);
        }
    }
    Ok(nodes)
}

struct TocIdAllocator {
    raw_id_counts: BTreeMap<String, usize>,
    used: HashSet<String>,
}

impl TocIdAllocator {
    fn new() -> Self {
        Self {
            raw_id_counts: BTreeMap::new(),
            used: HashSet::new(),
        }
    }

    fn record(&mut self, id: Option<&str>) {
        if let Some(id) = id {
            *self.raw_id_counts.entry(id.to_string()).or_default() += 1;
        }
    }

    fn allocate(&mut self, raw_id: Option<&str>, generated_id: &str) -> String {
        if let Some(raw) = raw_id {
            let unique = self.raw_id_counts.get(raw).copied().unwrap_or(0) == 1;
            if !raw.is_empty() && unique && self.used.insert(raw.to_string()) {
                return raw.to_string();
            }
        }

        let mut candidate = generated_id.to_string();
        let mut suffix = 1_usize;
        while !self.used.insert(candidate.clone()) {
            candidate = format!("{generated_id}-{suffix}");
            suffix += 1;
        }
        candidate
    }
}

fn collect_toc_ids(entry: EpubTocEntry<'_>, allocator: &mut TocIdAllocator) {
    allocator.record(entry.id());
    for child in entry.iter() {
        collect_toc_ids(child, allocator);
    }
}

fn map_toc_entry(
    entry: EpubTocEntry<'_>,
    generated_id: &str,
    allocator: &mut TocIdAllocator,
    warnings: &mut Vec<EpubWarning>,
) -> Result<Option<TocNode>, EpubError> {
    let raw_href = entry.href().map(|href| href.as_str().to_string());
    if let Some(raw) = &raw_href
        && is_external_toc_href(raw)
    {
        warnings.push(EpubWarning {
            code: EpubWarningCode::DegradedMarkup,
            resource_href: None,
        });
        return Ok(None);
    }

    let id = allocator.allocate(entry.id(), generated_id);
    let label = normalize_label(entry.label());

    let mut children = Vec::new();
    for (index, child) in entry.iter().enumerate() {
        let child_id = format!("{generated_id}-{index}");
        if let Some(node) = map_toc_entry(child, &child_id, allocator, warnings)? {
            children.push(node);
        }
    }

    let href = raw_href
        .as_deref()
        .map(normalize_toc_href)
        .transpose()?
        .unwrap_or_default();

    Ok(Some(TocNode {
        id,
        label,
        href,
        children,
    }))
}

fn normalize_label(label: &str) -> String {
    let normalized = label.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        "未命名目录项".to_string()
    } else {
        normalized
    }
}

fn is_external_toc_href(raw: &str) -> bool {
    let value = raw.trim();
    if value.starts_with("//") {
        return true;
    }

    let Some(colon) = value.find(':') else {
        return false;
    };
    let scheme = &value[..colon];
    let mut characters = scheme.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    if !characters
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.'))
    {
        return false;
    }

    // 单字母 scheme 视为 Windows 盘符，交给 ResourcePath 拒绝。
    scheme.len() > 1
}

fn normalize_toc_href(raw: &str) -> Result<String, EpubError> {
    let mut relative = raw.trim().trim_start_matches('/');
    while let Some(rest) = relative.strip_prefix("./") {
        relative = rest;
    }
    if relative.is_empty() {
        return Ok(String::new());
    }

    let (base, fragment) = match relative.split_once('#') {
        Some((base, fragment)) => (base, Some(fragment)),
        None => (relative, None),
    };
    if base.is_empty() {
        return Ok(fragment.map_or_else(String::new, |fragment| format!("#{fragment}")));
    }

    let path =
        ResourcePath::new(base).map_err(|_| EpubError::new(EpubErrorCode::UnsafeResourcePath))?;
    Ok(match fragment {
        Some(fragment) => format!("{path}#{fragment}"),
        None => path.to_string(),
    })
}

fn map_cover(
    epub: &Epub,
    warnings: &mut Vec<EpubWarning>,
) -> Result<Option<CoverReference>, EpubError> {
    let Some(entry) = epub.manifest().cover_image() else {
        warnings.push(EpubWarning {
            code: EpubWarningCode::MissingCover,
            resource_href: None,
        });
        return Ok(None);
    };

    let media_type = entry.media_type().to_string();
    if !is_image_media_type(&media_type) {
        warnings.push(EpubWarning {
            code: EpubWarningCode::MissingCover,
            resource_href: None,
        });
        return Ok(None);
    }

    Ok(Some(CoverReference {
        href: resource_path(entry.href())?,
        media_type,
    }))
}

fn is_image_media_type(media_type: &str) -> bool {
    media_type.to_ascii_lowercase().starts_with("image/")
}

fn map_chapter_weights(epub: &Epub) -> Result<Vec<u32>, EpubError> {
    let mut weights = Vec::new();
    for entry in epub.spine().iter() {
        let manifest_entry = entry
            .manifest_entry()
            .ok_or_else(|| EpubError::new(EpubErrorCode::InvalidOpf))?;
        let html = manifest_entry
            .read_str()
            .map_err(|_| EpubError::new(EpubErrorCode::InvalidOpf))?;

        let mut weight = count_text_characters(&html)?;
        if weight == 0 {
            weight = manifest_entry
                .read_bytes()
                .map_err(|_| EpubError::new(EpubErrorCode::InvalidOpf))?
                .len() as u64;
        }
        weights.push(u32::try_from(weight).unwrap_or(u32::MAX));
    }
    Ok(weights)
}

fn count_text_characters(html: &str) -> Result<u64, EpubError> {
    let body_depth = Rc::new(Cell::new(0_usize));
    let skip_depth = Rc::new(Cell::new(0_usize));
    let total = Rc::new(Cell::new(0_u64));

    let body_for_end = Rc::clone(&body_depth);
    let skip_for_end = Rc::clone(&skip_depth);
    let body_for_text = Rc::clone(&body_depth);
    let skip_for_text = Rc::clone(&skip_depth);
    let total_for_text = Rc::clone(&total);

    let mut rewriter = HtmlRewriter::new(
        Settings::new()
            .append_element_content_handler(element!("body", move |element| {
                let body_end = Rc::clone(&body_for_end);
                body_end.set(body_end.get() + 1);
                element.on_end_tag(end_tag!(move |_| {
                    let value = body_end.get();
                    body_end.set(value.saturating_sub(1));
                    Ok(())
                }))?;
                Ok(())
            }))
            .append_element_content_handler(element!(
                "script, style, noscript, template",
                move |element| {
                    let skip_end = Rc::clone(&skip_for_end);
                    skip_end.set(skip_end.get() + 1);
                    element.on_end_tag(end_tag!(move |_| {
                        let value = skip_end.get();
                        skip_end.set(value.saturating_sub(1));
                        Ok(())
                    }))?;
                    Ok(())
                }
            ))
            .append_element_content_handler(text!("*", move |chunk| {
                if body_for_text.get() > 0 && skip_for_text.get() == 0 {
                    let count = chunk.as_str().chars().count();
                    let count = u64::try_from(count).unwrap_or(u64::MAX);
                    total_for_text.set(total_for_text.get().saturating_add(count));
                }
                Ok(())
            })),
        |_: &[u8]| {},
    );

    rewriter
        .write(html.as_bytes())
        .map_err(|_| EpubError::new(EpubErrorCode::InvalidOpf))?;
    rewriter
        .end()
        .map_err(|_| EpubError::new(EpubErrorCode::InvalidOpf))?;
    Ok(total.get())
}

fn map_rbook_error(error: EbookError) -> EpubError {
    match error {
        EbookError::Archive(ArchiveError::InvalidResource { .. }) => {
            EpubError::new(EpubErrorCode::MissingOpf)
        }
        EbookError::Format(_) => EpubError::new(EpubErrorCode::InvalidOpf),
        _ => EpubError::new(EpubErrorCode::InvalidZip),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_toc_hrefs() {
        assert_eq!(
            normalize_toc_href("/OEBPS/chapter1.xhtml").expect("应有效"),
            "OEBPS/chapter1.xhtml"
        );
        assert_eq!(
            normalize_toc_href("./OEBPS/./chapter1.xhtml#s1").expect("应有效"),
            "OEBPS/chapter1.xhtml#s1"
        );
        assert_eq!(
            normalize_toc_href("#fragment").expect("应有效"),
            "#fragment"
        );
        assert_eq!(normalize_toc_href("").expect("空 href"), "");
    }

    #[test]
    fn rejects_unsafe_toc_hrefs() {
        for raw in ["../escape.xhtml", "/../escape.xhtml", "C:\\evil.xhtml"] {
            assert_eq!(
                normalize_toc_href(raw).expect_err("应拒绝").code(),
                EpubErrorCode::UnsafeResourcePath,
                "{raw}"
            );
        }
    }

    #[test]
    fn classifies_external_toc_hrefs() {
        assert!(is_external_toc_href("https://example.com/x"));
        assert!(is_external_toc_href("mailto:a@example.com"));
        assert!(is_external_toc_href("javascript:alert(1)"));
        assert!(is_external_toc_href("foo:bar"));
        assert!(is_external_toc_href("//example.com/x"));
        assert!(!is_external_toc_href("OEBPS/chapter1.xhtml"));
        assert!(!is_external_toc_href("/OEBPS/chapter1.xhtml#s1"));
        assert!(!is_external_toc_href("C:\\evil.xhtml"));
    }

    #[test]
    fn text_count_skips_script_and_style_and_limits_to_body() {
        let html = "<html><head><title>标题</title></head><body>正文<script>var x = 1;</script><style>.a{}</style></body></html>";
        assert_eq!(count_text_characters(html).expect("应解析"), 2);
    }

    #[test]
    fn classifies_image_media_types_case_insensitively() {
        assert!(is_image_media_type("image/png"));
        assert!(is_image_media_type("IMAGE/PNG"));
        assert!(is_image_media_type("Image/Jpeg"));
        assert!(!is_image_media_type("text/plain"));
        assert!(!is_image_media_type(""));
    }

    #[test]
    fn normalizes_labels() {
        assert_eq!(normalize_label("  第一  章  "), "第一 章");
        assert_eq!(normalize_label("\n\t"), "未命名目录项");
    }
}
