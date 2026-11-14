use std::collections::BTreeMap;

use epub_core::{
    BookMetadata, CoverReference, EpubWarning, EpubWarningCode, ManifestItem,
    PARSED_BOOK_SCHEMA_VERSION, ParsedBook, ResourcePath, SpineItem, TocNode,
};

fn path(value: &str) -> ResourcePath {
    ResourcePath::new(value).expect("测试资源路径应有效")
}

fn sample_book() -> ParsedBook {
    let mut book = ParsedBook::new(path("OEBPS/content.opf"));
    book.metadata = BookMetadata {
        title: Some("示例书籍".to_string()),
        authors: vec!["示例作者".to_string()],
        language: Some("zh-CN".to_string()),
        publisher: Some("示例出版社".to_string()),
    };
    book.manifest.insert(
        "chapter1".to_string(),
        ManifestItem {
            id: "chapter1".to_string(),
            href: path("OEBPS/chapter1.xhtml"),
            media_type: "application/xhtml+xml".to_string(),
            properties: vec!["nav".to_string()],
        },
    );
    book.spine.push(SpineItem {
        index: 0,
        idref: "chapter1".to_string(),
        href: path("OEBPS/chapter1.xhtml"),
        linear: true,
    });
    book.toc.push(TocNode {
        id: "toc-1".to_string(),
        label: "第一章".to_string(),
        href: "chapter1.xhtml".to_string(),
        children: vec![TocNode {
            id: "toc-1-1".to_string(),
            label: "第一节".to_string(),
            href: "chapter1.xhtml#s1".to_string(),
            children: Vec::new(),
        }],
    });
    book.cover = Some(CoverReference {
        href: path("OEBPS/cover.png"),
        media_type: "image/png".to_string(),
    });
    book.chapter_weights = vec![1_000];
    book.warnings.push(EpubWarning {
        code: EpubWarningCode::MissingCover,
        resource_href: Some(path("OEBPS/cover.png")),
    });
    book
}

#[test]
fn new_sets_current_schema_version() {
    let book = ParsedBook::new(path("OEBPS/content.opf"));
    assert_eq!(book.schema_version, PARSED_BOOK_SCHEMA_VERSION);
    assert!(book.has_supported_schema());
    assert!(book.manifest.is_empty());
    assert!(book.spine.is_empty());
    assert!(book.chapter_weights.is_empty());
}

#[test]
fn serializes_public_fields_in_camel_case() {
    let book = sample_book();
    let value = serde_json::to_value(&book).expect("序列化应成功");
    let object = value.as_object().expect("顶层应为 JSON 对象");

    for key in [
        "schemaVersion",
        "packagePath",
        "metadata",
        "manifest",
        "spine",
        "toc",
        "cover",
        "chapterWeights",
        "warnings",
    ] {
        assert!(object.contains_key(key), "缺少字段 {key}");
    }

    assert_eq!(value["schemaVersion"], PARSED_BOOK_SCHEMA_VERSION);
    assert_eq!(value["packagePath"], "OEBPS/content.opf");
    assert_eq!(value["metadata"]["authors"][0], "示例作者");
    assert_eq!(
        value["manifest"]["chapter1"]["mediaType"],
        "application/xhtml+xml"
    );
    assert_eq!(
        value["manifest"]["chapter1"]["href"],
        "OEBPS/chapter1.xhtml"
    );
    assert_eq!(value["spine"][0]["idref"], "chapter1");
    assert_eq!(value["spine"][0]["linear"], true);
    assert_eq!(value["toc"][0]["children"][0]["label"], "第一节");
    assert_eq!(value["warnings"][0]["code"], "missingCover");
    assert_eq!(value["warnings"][0]["resourceHref"], "OEBPS/cover.png");
}

#[test]
fn round_trips_through_json() {
    let book = sample_book();
    let json = serde_json::to_string(&book).expect("序列化应成功");
    let restored: ParsedBook = serde_json::from_str(&json).expect("反序列化应成功");
    assert_eq!(restored, book);
}

#[test]
fn manifest_serialization_order_is_stable() {
    let mut book = ParsedBook::new(path("OEBPS/content.opf"));
    for id in ["zeta", "alpha", "middle"] {
        book.manifest.insert(
            id.to_string(),
            ManifestItem {
                id: id.to_string(),
                href: path("OEBPS/chapter.xhtml"),
                media_type: "application/xhtml+xml".to_string(),
                properties: Vec::new(),
            },
        );
    }

    let json = serde_json::to_string(&book).expect("序列化应成功");
    let alpha = json.find("\"alpha\"").expect("应包含 alpha");
    let middle = json.find("\"middle\"").expect("应包含 middle");
    let zeta = json.find("\"zeta\"").expect("应包含 zeta");
    assert!(alpha < middle && middle < zeta, "manifest 应按 id 排序");
}

#[test]
fn manifest_map_type_is_btree_map() {
    let book = ParsedBook::new(path("OEBPS/content.opf"));
    let _: &BTreeMap<String, ManifestItem> = &book.manifest;
}

#[test]
fn warning_resource_href_rejects_unsafe_paths() {
    let absolute = r#"{"code":"missingCover","resourceHref":"/absolute.png"}"#;
    assert!(serde_json::from_str::<EpubWarning>(absolute).is_err());

    let traversal = r#"{"code":"missingCover","resourceHref":"../escape.png"}"#;
    assert!(serde_json::from_str::<EpubWarning>(traversal).is_err());
}

#[test]
fn warning_resource_href_accepts_relative_path() {
    let warning: EpubWarning =
        serde_json::from_str(r#"{"code":"missingCover","resourceHref":"OEBPS/cover.png"}"#)
            .expect("相对路径应可反序列化");
    assert_eq!(
        warning.resource_href.as_ref().map(ResourcePath::as_str),
        Some("OEBPS/cover.png")
    );
}
