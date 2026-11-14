mod support;

use epub_core::model::{EpubWarningCode, ParsedBook};
use epub_core::{EpubErrorCode, parse_epub};
use support::fixtures::{self, EncryptionStyle, FixtureKind, MIMETYPE};
use support::zip_writer::{ZipEntry, build_zip};

const SUCCESSFUL: [FixtureKind; 6] = [
    FixtureKind::MinimalEpub2,
    FixtureKind::MinimalEpub3,
    FixtureKind::NestedToc,
    FixtureKind::MissingMetadata,
    FixtureKind::CssAndImage,
    FixtureKind::Bilingual,
];

#[test]
fn maps_metadata_manifest_and_spine_for_six_fixtures() {
    for kind in SUCCESSFUL {
        let fixture = kind.build();
        let book = parse_epub(fixture.path())
            .unwrap_or_else(|error| panic!("{}: {error}", kind.filename()));

        assert_eq!(book.schema_version, 1, "{}", kind.filename());
        assert!(!book.manifest.is_empty(), "{}", kind.filename());
        assert!(!book.spine.is_empty(), "{}", kind.filename());
        assert_eq!(book.chapter_weights.len(), book.spine.len());
        assert!(book.chapter_weights.iter().all(|weight| *weight > 0));
        assert!(!book.toc.is_empty(), "{}", kind.filename());
        assert_eq!(book.package_path.as_str(), "OEBPS/content.opf");

        for (index, item) in book.spine.iter().enumerate() {
            assert_eq!(item.index, index);
            assert!(!item.href.as_str().starts_with('/'));
        }
        for item in book.manifest.values() {
            assert!(!item.href.as_str().starts_with('/'));
        }

        match kind {
            FixtureKind::MinimalEpub2 | FixtureKind::MinimalEpub3 => {
                assert_eq!(book.metadata.title.as_deref(), Some("示例书籍"));
                assert_eq!(book.metadata.authors, vec!["示例作者"]);
                assert_eq!(book.metadata.language.as_deref(), Some("zh-CN"));
                assert_eq!(book.manifest.len(), 2);
                assert_eq!(book.spine.len(), 1);
                assert_eq!(book.spine[0].idref, "chapter1");
                assert_eq!(book.spine[0].href.as_str(), "OEBPS/chapter1.xhtml");
                assert!(book.spine[0].linear);
                assert_eq!(book.toc.len(), 1);
                assert_eq!(book.toc[0].label, "第一章");
                assert_eq!(book.toc[0].href, "OEBPS/chapter1.xhtml");
                assert!(book.cover.is_none());
                assert!(
                    book.warnings
                        .iter()
                        .any(|warning| warning.code == EpubWarningCode::MissingCover)
                );
            }
            FixtureKind::NestedToc => {
                assert_eq!(book.metadata.title.as_deref(), Some("嵌套目录示例"));
                assert_eq!(book.spine.len(), 2);
                assert_eq!(book.manifest.len(), 3);
                assert_eq!(book.toc.len(), 2);
                assert_eq!(book.toc[0].label, "第一部分");
                assert_eq!(book.toc[0].children.len(), 2);
                assert_eq!(book.toc[0].children[0].label, "第一节");
                assert_eq!(book.toc[0].children[0].href, "OEBPS/chapter1.xhtml#s1");
                assert_eq!(book.toc[1].label, "第二部分");
                assert_eq!(book.toc[1].href, "OEBPS/chapter2.xhtml");
            }
            FixtureKind::MissingMetadata => {
                assert!(book.metadata.title.is_none());
                assert!(book.metadata.authors.is_empty());
                assert!(book.metadata.language.is_none());
                assert!(book.metadata.publisher.is_none());
                assert_eq!(book.spine.len(), 1);
            }
            FixtureKind::CssAndImage => {
                assert_eq!(book.metadata.title.as_deref(), Some("图文混排示例"));
                assert_eq!(book.manifest.len(), 4);
                assert!(
                    book.manifest
                        .values()
                        .any(|item| item.media_type == "image/png")
                );
            }
            FixtureKind::Bilingual => {
                assert_eq!(book.metadata.title.as_deref(), Some("Bilingual Sample"));
                assert_eq!(book.spine.len(), 2);
            }
            _ => unreachable!("不在成功夹具集合中"),
        }
    }
}

#[test]
fn missing_opf_maps_to_stable_error() {
    let fixture = FixtureKind::MissingOpf.build();
    let error = parse_epub(fixture.path()).expect_err("缺 OPF 应失败");
    assert_eq!(error.code(), EpubErrorCode::MissingOpf);
    assert!(!error.to_string().contains("/Users/"));
}

#[test]
fn unsafe_paths_and_duplicates_are_rejected_before_rbook() {
    let unsafe_fixture = FixtureKind::UnsafePaths.build();
    assert_eq!(
        parse_epub(unsafe_fixture.path())
            .expect_err("不安全路径应被拒绝")
            .code(),
        EpubErrorCode::UnsafeResourcePath
    );

    let duplicate_fixture = FixtureKind::DuplicateEntries.build();
    assert_eq!(
        parse_epub(duplicate_fixture.path())
            .expect_err("重复条目应被拒绝")
            .code(),
        EpubErrorCode::InvalidZip
    );
}

#[test]
fn missing_container_is_rejected_before_rbook() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("no-container.epub");
    std::fs::write(
        &path,
        build_zip(&[
            ZipEntry::new("mimetype", MIMETYPE),
            ZipEntry::new("OEBPS/chapter1.xhtml", "<html/>"),
        ]),
    )
    .expect("write fixture");

    assert_eq!(
        parse_epub(&path).expect_err("缺 container 应失败").code(),
        EpubErrorCode::MissingContainer
    );
}

#[test]
fn encrypted_fonts_produce_warning_for_all_xml_styles() {
    for style in [
        EncryptionStyle::DoubleQuoted,
        EncryptionStyle::SingleQuoted,
        EncryptionStyle::Prefixed,
    ] {
        let fixture = fixtures::encrypted_epub("OEBPS/fonts/sample.woff2", style);
        let book = parse_epub(fixture.path())
            .unwrap_or_else(|error| panic!("字体加密应允许：{style:?}: {error}"));

        let font_warnings: Vec<_> = book
            .warnings
            .iter()
            .filter(|warning| warning.code == EpubWarningCode::EncryptedFontFallback)
            .collect();
        assert_eq!(font_warnings.len(), 1);
        assert_eq!(
            font_warnings[0]
                .resource_href
                .as_ref()
                .map(epub_core::ResourcePath::as_str),
            Some("OEBPS/fonts/sample.woff2")
        );
    }
}

#[test]
fn encrypted_content_is_rejected_with_relative_path() {
    let fixture = fixtures::encrypted_epub("OEBPS/chapter1.xhtml", EncryptionStyle::DoubleQuoted);
    let error = parse_epub(fixture.path()).expect_err("正文加密应拒绝");
    assert_eq!(error.code(), EpubErrorCode::EncryptedContent);
    assert_eq!(
        error.resource_path().map(epub_core::ResourcePath::as_str),
        Some("OEBPS/chapter1.xhtml")
    );
    assert!(!error.to_string().contains("/Users/"));
}

#[test]
fn parses_large_epub_with_streaming_precheck() {
    let fixture = fixtures::large_epub();
    let book = parse_epub(fixture.path()).expect("100 MB 夹具应可解析");
    assert_eq!(book.spine.len(), 1);
    assert_eq!(book.metadata.title.as_deref(), Some("大文件样本"));
}

#[test]
fn parsed_book_round_trips_through_json() {
    let fixture = FixtureKind::MinimalEpub3.build();
    let book = parse_epub(fixture.path()).expect("应解析");
    let json = serde_json::to_string(&book).expect("序列化");
    let restored: ParsedBook = serde_json::from_str(&json).expect("反序列化");
    assert_eq!(restored, book);
}

#[test]
fn rtl_spine_is_rejected() {
    let container = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;
    let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>RTL 样本</dc:title>
    <dc:identifier id="id">urn:uuid:rtl</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine page-progression-direction="rtl">
    <itemref idref="c1"/>
  </spine>
</package>
"#;
    let nav = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">第一章</a></li></ol></nav></body></html>"#;
    let chapter = r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>正文。</p></body></html>"#;

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("rtl.epub");
    std::fs::write(
        &path,
        build_zip(&[
            ZipEntry::new("mimetype", MIMETYPE),
            ZipEntry::new("META-INF/container.xml", container),
            ZipEntry::new("OEBPS/content.opf", opf),
            ZipEntry::new("OEBPS/nav.xhtml", nav),
            ZipEntry::new("OEBPS/chapter1.xhtml", chapter),
        ]),
    )
    .expect("write fixture");

    let error = parse_epub(&path).expect_err("RTL 应被拒绝");
    assert_eq!(error.code(), EpubErrorCode::VerticalWriting);
}
