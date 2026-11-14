mod support;

use std::path::PathBuf;

use epub_core::model::EpubWarningCode;
use epub_core::{EpubErrorCode, fingerprint_file, parse_epub};
use support::fixtures::{self, EncryptionStyle, FixtureKind, MIMETYPE};
use support::zip_writer::{ZipEntry, build_zip};

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
    <dc:title>验收测试</dc:title>
    <dc:identifier id="id">urn:uuid:acceptance</dc:identifier>
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

fn chapter_html(body: &str) -> String {
    format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title></title></head><body>{body}</body></html>"
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
    let path = dir.path().join("acceptance.epub");
    std::fs::write(&path, build_zip(&entries)).expect("write epub");
    (dir, path)
}

fn write_zip64_marker() -> (tempfile::TempDir, PathBuf) {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&0xffff_u16.to_le_bytes());
    bytes.extend_from_slice(&0xffff_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("zip64.epub");
    std::fs::write(&path, bytes).expect("write zip64");
    (dir, path)
}

#[test]
fn six_successful_fixtures_parse_completely() {
    let kinds = [
        FixtureKind::MinimalEpub2,
        FixtureKind::MinimalEpub3,
        FixtureKind::NestedToc,
        FixtureKind::MissingMetadata,
        FixtureKind::CssAndImage,
        FixtureKind::Bilingual,
    ];

    for kind in kinds {
        let fixture = kind.build();
        let book = parse_epub(fixture.path())
            .unwrap_or_else(|error| panic!("{}: {error}", kind.filename()));
        assert!(!book.manifest.is_empty());
        assert!(!book.spine.is_empty());
        assert!(!book.toc.is_empty());
        assert_eq!(book.chapter_weights.len(), book.spine.len());
        assert!(book.chapter_weights.iter().all(|weight| *weight > 0));
        assert_eq!(book.schema_version, 1);
    }
}

#[test]
fn error_scenarios_are_stable() {
    let missing_opf = FixtureKind::MissingOpf.build();
    assert_eq!(
        parse_epub(missing_opf.path()).expect_err("缺 OPF").code(),
        EpubErrorCode::MissingOpf
    );

    let missing_container_dir = tempfile::tempdir().expect("temp dir");
    let missing_container = missing_container_dir.path().join("no-container.epub");
    std::fs::write(
        &missing_container,
        build_zip(&[
            ZipEntry::new("mimetype", MIMETYPE),
            ZipEntry::new("OEBPS/chapter1.xhtml", chapter_html("<p>正文</p>")),
        ]),
    )
    .expect("write");
    assert_eq!(
        parse_epub(&missing_container)
            .expect_err("缺 container")
            .code(),
        EpubErrorCode::MissingContainer
    );

    let unsafe_paths = FixtureKind::UnsafePaths.build();
    assert_eq!(
        parse_epub(unsafe_paths.path())
            .expect_err("危险路径")
            .code(),
        EpubErrorCode::UnsafeResourcePath
    );

    let duplicates = FixtureKind::DuplicateEntries.build();
    assert_eq!(
        parse_epub(duplicates.path()).expect_err("重复条目").code(),
        EpubErrorCode::InvalidZip
    );

    let (_zip64_dir, zip64) = write_zip64_marker();
    assert_eq!(
        parse_epub(&zip64).expect_err("ZIP64").code(),
        EpubErrorCode::LimitExceeded
    );

    let encrypted = fixtures::encrypted_epub("OEBPS/chapter1.xhtml", EncryptionStyle::DoubleQuoted);
    assert_eq!(
        parse_epub(encrypted.path()).expect_err("加密正文").code(),
        EpubErrorCode::EncryptedContent
    );

    let (fixed_dir, fixed) = write_epub(
        "",
        r#"<meta property="rendition:layout">pre-paginated</meta>"#,
        &chapter_html("<p>正文</p>"),
        &[],
    );
    let _ = fixed_dir;
    assert_eq!(
        parse_epub(&fixed).expect_err("固定版式").code(),
        EpubErrorCode::FixedLayout
    );

    let (vertical_dir, vertical) = write_epub(
        "",
        "",
        &chapter_html(r#"<p style="writing-mode: vertical-rl">正文</p>"#),
        &[],
    );
    let _ = vertical_dir;
    assert_eq!(
        parse_epub(&vertical).expect_err("竖排").code(),
        EpubErrorCode::VerticalWriting
    );

    let (script_dir, script) = write_epub(
        "",
        "",
        &chapter_html("<script>alert(1)</script><p>正文</p>"),
        &[],
    );
    let _ = script_dir;
    assert_eq!(
        parse_epub(&script).expect_err("脚本").code(),
        EpubErrorCode::ScriptDependency
    );

    let (missing_css_dir, missing_css) = write_epub(
        r#"<item id="css" href="missing.css" media-type="text/css"/>"#,
        "",
        &chapter_html("<p>正文</p>"),
        &[],
    );
    let _ = missing_css_dir;
    assert_eq!(
        parse_epub(&missing_css).expect_err("缺失内容资源").code(),
        EpubErrorCode::InvalidOpf
    );
}

#[test]
fn remote_resources_only_warn() {
    let (dir, path) = write_epub(
        "",
        "",
        &chapter_html(r#"<img src="https://example.com/a.png"/><p>正文</p>"#),
        &[],
    );
    let _ = dir;

    let book = parse_epub(&path).expect("远程资源不应拒绝整本书");
    assert!(
        book.warnings
            .iter()
            .any(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
    );
}

#[test]
fn public_json_has_no_paths_urls_or_third_party_types() {
    let fixture = FixtureKind::CssAndImage.build();
    let book = parse_epub(fixture.path()).expect("应解析");
    let json = serde_json::to_string(&book).expect("序列化");

    for needle in [
        "/Users/",
        "http://",
        "https://",
        "rbook",
        "ZipArchive",
        "lol_html",
        "quick_xml",
    ] {
        assert!(!json.contains(needle), "JSON 不应包含 {needle}");
    }
}

#[test]
fn source_file_is_unchanged_after_parse() {
    let fixture = FixtureKind::CssAndImage.build();
    let path = fixture.path();

    let before_fingerprint = fingerprint_file(path).expect("解析前指纹");
    let before_bytes = std::fs::read(path).expect("解析前读取");

    parse_epub(path).expect("应解析");

    let after_fingerprint = fingerprint_file(path).expect("解析后指纹");
    let after_bytes = std::fs::read(path).expect("解析后读取");

    assert_eq!(before_fingerprint, after_fingerprint);
    assert_eq!(before_bytes, after_bytes);
}
