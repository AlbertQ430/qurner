mod support;

use std::path::PathBuf;

use epub_core::model::EpubWarningCode;
use epub_core::{EpubErrorCode, parse_epub};
use support::zip_writer::{ZipEntry, build_zip};

const MIMETYPE: &str = "application/epub+zip";

const CONTAINER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

fn opf(meta_extra: &str, manifest_extra: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>安全测试</dc:title>
    <dc:identifier id="id">urn:uuid:content-safety</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
    {meta_extra}
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    {manifest_extra}
  </manifest>
  <spine><itemref idref="c1"/></spine>
</package>
"#
    )
}

fn write_epub(
    manifest_extra: &str,
    meta_extra: &str,
    chapter: &str,
    extra: &[(&str, &str)],
) -> (tempfile::TempDir, PathBuf) {
    let nav = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">第一章</a></li></ol></nav></body></html>"#;

    let mut entries = vec![
        ZipEntry::new("mimetype", MIMETYPE),
        ZipEntry::new("META-INF/container.xml", CONTAINER),
        ZipEntry::new("OEBPS/content.opf", opf(meta_extra, manifest_extra)),
        ZipEntry::new("OEBPS/nav.xhtml", nav),
        ZipEntry::new("OEBPS/chapter1.xhtml", chapter),
    ];
    for (name, content) in extra {
        entries.push(ZipEntry::new(*name, *content));
    }

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("content-safety.epub");
    std::fs::write(&path, build_zip(&entries)).expect("write epub");
    (dir, path)
}

fn chapter(body: &str) -> String {
    format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title></title></head><body>{body}</body></html>"
    )
}

#[test]
fn modern_fixed_layout_is_rejected() {
    let (_dir, path) = write_epub(
        "",
        r#"<meta property="rendition:layout">pre-paginated</meta>"#,
        &chapter("<p>正文</p>"),
        &[],
    );

    let error = parse_epub(&path).expect_err("固定版式应拒绝");
    assert_eq!(error.code(), EpubErrorCode::FixedLayout);
    assert_eq!(
        error.resource_path().map(epub_core::ResourcePath::as_str),
        Some("OEBPS/content.opf")
    );
    assert!(!error.to_string().contains("/Users/"));
}

#[test]
fn legacy_fixed_layout_meta_is_rejected() {
    let (_dir, path) = write_epub(
        "",
        r#"<meta content="TRUE" name="fixed-layout"/>"#,
        &chapter("<p>正文</p>"),
        &[],
    );

    let error = parse_epub(&path).expect_err("legacy 固定版式应拒绝");
    assert_eq!(error.code(), EpubErrorCode::FixedLayout);
}

#[test]
fn ibooks_fixed_layout_display_options_are_rejected() {
    let options = r#"<?xml version="1.0" encoding="UTF-8"?>
<display_options>
  <platform name="*">
    <option name="fixed-layout">true</option>
  </platform>
</display_options>
"#;
    let (_dir, path) = write_epub(
        "",
        "",
        &chapter("<p>正文</p>"),
        &[("META-INF/com.apple.ibooks.display-options.xml", options)],
    );

    let error = parse_epub(&path).expect_err("iBooks 固定版式应拒绝");
    assert_eq!(error.code(), EpubErrorCode::FixedLayout);
    assert_eq!(
        error.resource_path().map(epub_core::ResourcePath::as_str),
        Some("META-INF/com.apple.ibooks.display-options.xml")
    );
}

#[test]
fn vertical_writing_in_external_css_is_rejected() {
    let (_dir, path) = write_epub(
        r#"<item id="css" href="style.css" media-type="text/css"/>"#,
        "",
        &chapter("<p>正文</p>"),
        &[("OEBPS/style.css", "p { WRITING-MODE :  Vertical-RL; }")],
    );

    let error = parse_epub(&path).expect_err("竖排 CSS 应拒绝");
    assert_eq!(error.code(), EpubErrorCode::VerticalWriting);
    assert_eq!(
        error.resource_path().map(epub_core::ResourcePath::as_str),
        Some("OEBPS/style.css")
    );
}

#[test]
fn vertical_writing_in_inline_style_is_rejected() {
    let (_dir, path) = write_epub(
        "",
        "",
        &chapter(r#"<p style="writing-mode: sideways-lr">正文</p>"#),
        &[],
    );

    let error = parse_epub(&path).expect_err("内联竖排应拒绝");
    assert_eq!(error.code(), EpubErrorCode::VerticalWriting);
    assert_eq!(
        error.resource_path().map(epub_core::ResourcePath::as_str),
        Some("OEBPS/chapter1.xhtml")
    );
}

#[test]
fn vertical_writing_in_style_element_is_rejected() {
    let (_dir, path) = write_epub(
        "",
        "",
        &chapter("<style>p { -webkit-writing-mode: vertical-rl; }</style><p>正文</p>"),
        &[],
    );

    let error = parse_epub(&path).expect_err("style 标签竖排应拒绝");
    assert_eq!(error.code(), EpubErrorCode::VerticalWriting);
}

#[test]
fn script_element_is_rejected() {
    let (_dir, path) = write_epub(
        "",
        "",
        &chapter("<script>alert(1)</script><p>正文</p>"),
        &[],
    );

    let error = parse_epub(&path).expect_err("脚本应拒绝");
    assert_eq!(error.code(), EpubErrorCode::ScriptDependency);
}

#[test]
fn event_handler_attributes_are_rejected() {
    let (_dir, path) = write_epub(
        "",
        "",
        &chapter("<body onload=\"boot()\"><p onerror=\"x()\">正文</p></body>"),
        &[],
    );

    let error = parse_epub(&path).expect_err("事件属性应拒绝");
    assert_eq!(error.code(), EpubErrorCode::ScriptDependency);
}

#[test]
fn javascript_urls_are_rejected() {
    let (_dir, path) = write_epub(
        "",
        "",
        &chapter(r#"<a href="  JavaScript:alert(1)">链接</a>"#),
        &[],
    );

    let error = parse_epub(&path).expect_err("javascript: 应拒绝");
    assert_eq!(error.code(), EpubErrorCode::ScriptDependency);
}

#[test]
fn iframe_and_object_are_rejected() {
    for body in [
        r#"<iframe src="chapter1.xhtml"></iframe>"#,
        r#"<object data="chapter1.xhtml"></object>"#,
    ] {
        let (_dir, path) = write_epub("", "", &chapter(body), &[]);
        let error = parse_epub(&path).expect_err("嵌入内容应拒绝");
        assert_eq!(error.code(), EpubErrorCode::ScriptDependency);
    }
}

#[test]
fn remote_images_produce_one_deduplicated_warning() {
    let (_dir, path) = write_epub(
        "",
        "",
        &chapter(
            r#"<img src="https://example.com/a.png"/><img src="http://example.com/b.png"/><p>正文</p>"#,
        ),
        &[],
    );

    let book = parse_epub(&path).expect("远程图片不应拒绝整本书");
    let warnings: Vec<_> = book
        .warnings
        .iter()
        .filter(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
        .collect();
    assert_eq!(warnings.len(), 1, "同一文件只报告一次");
    assert_eq!(
        warnings[0]
            .resource_href
            .as_ref()
            .map(epub_core::ResourcePath::as_str),
        Some("OEBPS/chapter1.xhtml")
    );
    assert!(
        !warnings[0]
            .resource_href
            .as_ref()
            .unwrap()
            .as_str()
            .contains("example.com")
    );
}

#[test]
fn srcset_and_remote_css_import_produce_warnings() {
    let (_dir, path) = write_epub(
        r#"<item id="css" href="style.css" media-type="text/css"/>"#,
        "",
        &chapter(r#"<img srcset="images/a.png 1x, https://example.com/a.png 2x"/>"#),
        &[(
            "OEBPS/style.css",
            "@import url(\"https://example.com/theme.css\");",
        )],
    );

    let book = parse_epub(&path).expect("应解析");
    let warnings: Vec<_> = book
        .warnings
        .iter()
        .filter(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
        .collect();
    assert_eq!(warnings.len(), 2, "HTML 与 CSS 各一条");
}

#[test]
fn external_links_are_not_remote_resources() {
    let (_dir, path) = write_epub(
        "",
        "",
        &chapter(r#"<a href="https://example.com/page">外部链接</a><p>正文</p>"#),
        &[],
    );

    let book = parse_epub(&path).expect("外部链接不应拒绝");
    assert!(
        !book
            .warnings
            .iter()
            .any(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
    );
}

#[test]
fn local_and_data_uris_are_not_warnings() {
    let (_dir, path) = write_epub(
        r#"<item id="css" href="style.css" media-type="text/css"/>"#,
        "",
        &chapter(r#"<img src="images/a.png"/><img src="data:image/png;base64,AAAA"/><p>正文</p>"#),
        &[(
            "OEBPS/style.css",
            "body { background: url(images/bg.png); }",
        )],
    );

    let book = parse_epub(&path).expect("本地资源应通过");
    assert!(
        !book
            .warnings
            .iter()
            .any(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
    );
}

#[test]
fn errors_do_not_expose_remote_urls_or_absolute_paths() {
    let (_dir, path) = write_epub(
        "",
        "",
        &chapter(r#"<script src="https://example.com/a.js"></script>"#),
        &[],
    );

    let error = parse_epub(&path).expect_err("脚本应拒绝");
    let message = error.to_string();
    assert!(!message.contains("example.com"));
    assert!(!message.contains("https://"));
    assert!(!message.contains("/Users/"));
}

#[test]
fn remote_font_in_style_attribute_produces_warning() {
    let (_dir, path) = write_epub(
        "",
        "",
        &chapter(r#"<p style="background-image: url(https://example.com/font.woff2)">正文</p>"#),
        &[],
    );

    let book = parse_epub(&path).expect("应解析");
    let warnings: Vec<_> = book
        .warnings
        .iter()
        .filter(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
        .collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0]
            .resource_href
            .as_ref()
            .map(epub_core::ResourcePath::as_str),
        Some("OEBPS/chapter1.xhtml")
    );
}

#[test]
fn remote_import_in_style_element_produces_warning() {
    let (_dir, path) = write_epub(
        "",
        "",
        &chapter(
            r#"<style>@import url(https://example.com/theme.css); p { color: black; }</style><p>正文</p>"#,
        ),
        &[],
    );

    let book = parse_epub(&path).expect("应解析");
    let warnings: Vec<_> = book
        .warnings
        .iter()
        .filter(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
        .collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0]
            .resource_href
            .as_ref()
            .map(epub_core::ResourcePath::as_str),
        Some("OEBPS/chapter1.xhtml")
    );
}

#[test]
fn remote_import_after_local_import_is_detected() {
    let css = r#"@import "images/base.css";
@import "https://example.com/theme.css";
"#;
    let (_dir, path) = write_epub(
        r#"<item id="css" href="style.css" media-type="text/css"/>"#,
        "",
        &chapter("<p>正文</p>"),
        &[("OEBPS/style.css", css)],
    );

    let book = parse_epub(&path).expect("应解析");
    let warnings: Vec<_> = book
        .warnings
        .iter()
        .filter(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
        .collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0]
            .resource_href
            .as_ref()
            .map(epub_core::ResourcePath::as_str),
        Some("OEBPS/style.css")
    );
}

#[test]
fn missing_manifest_css_is_invalid_opf() {
    let (_dir, path) = write_epub(
        r#"<item id="css" href="missing.css" media-type="text/css"/>"#,
        "",
        &chapter("<p>正文</p>"),
        &[],
    );

    let error = parse_epub(&path).expect_err("缺失 CSS 应失败");
    assert_eq!(error.code(), EpubErrorCode::InvalidOpf);
    assert_eq!(
        error.resource_path().map(epub_core::ResourcePath::as_str),
        Some("OEBPS/missing.css")
    );
}

#[test]
fn missing_manifest_xhtml_is_invalid_opf() {
    let (_dir, path) = write_epub(
        r#"<item id="extra" href="missing.xhtml" media-type="application/xhtml+xml"/>"#,
        "",
        &chapter("<p>正文</p>"),
        &[],
    );

    let error = parse_epub(&path).expect_err("缺失 XHTML 应失败");
    assert_eq!(error.code(), EpubErrorCode::InvalidOpf);
    assert_eq!(
        error.resource_path().map(epub_core::ResourcePath::as_str),
        Some("OEBPS/missing.xhtml")
    );
}
