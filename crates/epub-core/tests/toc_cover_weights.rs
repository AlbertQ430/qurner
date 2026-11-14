mod support;

use std::path::PathBuf;

use epub_core::model::EpubWarningCode;
use epub_core::{EpubErrorCode, parse_epub};
use support::fixtures::FixtureKind;
use support::zip_writer::{ZipEntry, build_zip};

const MIMETYPE: &str = "application/epub+zip";

fn chapter_html(body: &str) -> String {
    format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title></title></head><body>{body}</body></html>"
    )
}

fn write_epub(nav_body: &str, chapter: &str) -> (tempfile::TempDir, PathBuf) {
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
    <dc:title>测试书</dc:title>
    <dc:identifier id="id">urn:uuid:toc-test</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="c1"/></spine>
</package>
"#;
    let nav = format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><body><nav epub:type=\"toc\">{nav_body}</nav></body></html>"
    );

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("toc-test.epub");
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
    .expect("write epub");
    (dir, path)
}

#[test]
fn external_toc_items_are_skipped_with_warning() {
    let (_dir, path) = write_epub(
        r#"<ol>
  <li><a href="https://example.com/outside">外部链接</a></li>
  <li><a href="mailto:a@example.com">邮件</a></li>
  <li><a href="chapter1.xhtml">本地章节</a></li>
</ol>"#,
        &chapter_html("<p>正文</p>"),
    );

    let book = parse_epub(&path).expect("应解析");
    assert_eq!(book.toc.len(), 1);
    assert_eq!(book.toc[0].label, "本地章节");
    assert_eq!(book.toc[0].href, "OEBPS/chapter1.xhtml");
    let degraded = book
        .warnings
        .iter()
        .filter(|warning| warning.code == EpubWarningCode::DegradedMarkup)
        .count();
    assert_eq!(degraded, 2, "两个外部目录项各产生一条 warning");
}

#[test]
fn empty_toc_label_uses_deterministic_fallback() {
    let (_dir, path) = write_epub(
        r#"<ol><li><a href="chapter1.xhtml">   </a></li></ol>"#,
        &chapter_html("<p>正文</p>"),
    );

    let book = parse_epub(&path).expect("应解析");
    assert_eq!(book.toc.len(), 1);
    assert_eq!(book.toc[0].label, "未命名目录项");
}

#[test]
fn nested_nav_keeps_two_levels_without_synthetic_root() {
    let (_dir, path) = write_epub(
        r#"<ol>
  <li><a href="chapter1.xhtml">第一部分</a>
    <ol>
      <li><a href="chapter1.xhtml#s1">第一节</a></li>
    </ol>
  </li>
  <li><a href="chapter1.xhtml#s2">第二部分</a></li>
</ol>"#,
        &chapter_html("<p>正文</p>"),
    );

    let book = parse_epub(&path).expect("应解析");
    assert_eq!(book.toc.len(), 2);
    assert_eq!(book.toc[0].label, "第一部分");
    assert_eq!(book.toc[0].children.len(), 1);
    assert_eq!(book.toc[0].children[0].label, "第一节");
    assert_eq!(book.toc[0].children[0].href, "OEBPS/chapter1.xhtml#s1");
    assert_eq!(book.toc[1].label, "第二部分");
    assert_eq!(book.toc[1].href, "OEBPS/chapter1.xhtml#s2");
}

#[test]
fn chapter_weights_count_unicode_characters() {
    let chapter = chapter_html("中文正文");
    let (_dir, path) = write_epub(
        r#"<ol><li><a href="chapter1.xhtml">第一章</a></li></ol>"#,
        &chapter,
    );

    let book = parse_epub(&path).expect("应解析");
    assert_eq!(book.chapter_weights.len(), book.spine.len());
    assert_eq!(book.chapter_weights[0], 4, "应为 4 个 Unicode 字符");
    assert_ne!(
        book.chapter_weights[0],
        chapter.len() as u32,
        "不应按 UTF-8 字节"
    );
}

#[test]
fn chapter_weights_skip_style_noscript_and_template() {
    let chapter = chapter_html(
        "<p>正文</p><style>.a{color:red}</style><noscript>备用</noscript><template>模板</template>",
    );
    let (_dir, path) = write_epub(
        r#"<ol><li><a href="chapter1.xhtml">第一章</a></li></ol>"#,
        &chapter,
    );

    let book = parse_epub(&path).expect("应解析");
    assert_eq!(book.chapter_weights[0], 2);
}

#[test]
fn empty_chapter_uses_byte_length_fallback() {
    let chapter = chapter_html("");
    let expected = chapter.len() as u32;
    let (_dir, path) = write_epub(
        r#"<ol><li><a href="chapter1.xhtml">第一章</a></li></ol>"#,
        &chapter,
    );

    let book = parse_epub(&path).expect("应解析");
    assert_eq!(book.chapter_weights[0], expected);
}

#[test]
fn unsafe_toc_href_is_rejected() {
    let (_dir, path) = write_epub(
        r#"<ol><li><a href="C:\evil.xhtml">非法</a></li></ol>"#,
        &chapter_html("<p>正文</p>"),
    );

    let error = parse_epub(&path).expect_err("非法路径应拒绝");
    assert_eq!(error.code(), EpubErrorCode::UnsafeResourcePath);
}

#[test]
fn cover_round_trips_through_json() {
    let fixture = FixtureKind::CssAndImage.build();
    let book = parse_epub(fixture.path()).expect("应解析");
    assert!(book.cover.is_some());
    assert_eq!(book.chapter_weights.len(), book.spine.len());

    let json = serde_json::to_string(&book).expect("序列化");
    let restored: epub_core::model::ParsedBook = serde_json::from_str(&json).expect("反序列化");
    assert_eq!(restored, book);
}

#[test]
fn non_image_cover_is_treated_as_missing() {
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
    <dc:title>非图片封面</dc:title>
    <dc:identifier id="id">urn:uuid:cover-test</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="cover" href="cover.bin" media-type="text/plain" properties="cover-image"/>
  </manifest>
  <spine><itemref idref="c1"/></spine>
</package>
"#;
    let nav = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">第一章</a></li></ol></nav></body></html>"#;
    let chapter = chapter_html("<p>正文</p>");

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("cover-test.epub");
    std::fs::write(
        &path,
        build_zip(&[
            ZipEntry::new("mimetype", MIMETYPE),
            ZipEntry::new("META-INF/container.xml", container),
            ZipEntry::new("OEBPS/content.opf", opf),
            ZipEntry::new("OEBPS/nav.xhtml", nav),
            ZipEntry::new("OEBPS/chapter1.xhtml", chapter),
            ZipEntry::new("OEBPS/cover.bin", "not an image"),
        ]),
    )
    .expect("write epub");

    let book = parse_epub(&path).expect("应解析");
    assert!(book.cover.is_none());
    assert!(
        book.warnings
            .iter()
            .any(|warning| warning.code == EpubWarningCode::MissingCover)
    );
}

fn write_epub2(ncx: &str) -> (tempfile::TempDir, PathBuf) {
    let container = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;
    let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>NCX 测试</dc:title>
    <dc:identifier id="id">urn:uuid:ncx-test</dc:identifier>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine toc="ncx"><itemref idref="c1"/></spine>
</package>
"#;
    let chapter = chapter_html("<p>正文</p>");

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ncx-test.epub");
    std::fs::write(
        &path,
        build_zip(&[
            ZipEntry::new("mimetype", MIMETYPE),
            ZipEntry::new("META-INF/container.xml", container),
            ZipEntry::new("OEBPS/content.opf", opf),
            ZipEntry::new("OEBPS/toc.ncx", ncx),
            ZipEntry::new("OEBPS/chapter1.xhtml", chapter),
        ]),
    )
    .expect("write epub");
    (dir, path)
}

#[test]
fn chapter_title_does_not_contribute_to_weight() {
    let chapter = "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>不应计入的标题文字</title></head><body><p>正文</p></body></html>";
    let (_dir, path) = write_epub(
        r#"<ol><li><a href="chapter1.xhtml">第一章</a></li></ol>"#,
        chapter,
    );

    let book = parse_epub(&path).expect("应解析");
    assert_eq!(book.chapter_weights[0], 2, "只统计 body 内正文");
}

#[test]
fn custom_schemes_and_protocol_relative_toc_items_are_external() {
    let (_dir, path) = write_epub(
        r#"<ol>
  <li><a href="foo:bar">自定义</a></li>
  <li><a href="//example.com/chapter">协议相对</a></li>
  <li><a href="chapter1.xhtml">本地</a></li>
</ol>"#,
        &chapter_html("<p>正文</p>"),
    );

    let book = parse_epub(&path).expect("应解析");
    assert_eq!(book.toc.len(), 1);
    assert_eq!(book.toc[0].href, "OEBPS/chapter1.xhtml");
    assert_eq!(
        book.warnings
            .iter()
            .filter(|warning| warning.code == EpubWarningCode::DegradedMarkup)
            .count(),
        2
    );
}

#[test]
fn duplicate_ncx_ids_are_regenerated_and_unique() {
    let ncx = r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head/>
  <docTitle><text>NCX 测试</text></docTitle>
  <navMap>
    <navPoint id="dup" playOrder="1">
      <navLabel><text>第一章</text></navLabel>
      <content src="chapter1.xhtml"/>
    </navPoint>
    <navPoint id="dup" playOrder="2">
      <navLabel><text>第二章</text></navLabel>
      <content src="chapter1.xhtml#s2"/>
    </navPoint>
  </navMap>
</ncx>
"#;
    let (_dir, path) = write_epub2(ncx);
    let book = parse_epub(&path).expect("应解析");

    assert_eq!(book.toc.len(), 2);
    assert_eq!(book.toc[0].id, "toc-0");
    assert_eq!(book.toc[1].id, "toc-1");
    assert_ne!(book.toc[0].id, "dup");
}

#[test]
fn uppercase_cover_mime_is_recognized() {
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
    <dc:title>大写封面</dc:title>
    <dc:identifier id="id">urn:uuid:cover-upper</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="cover" href="cover.png" media-type="IMAGE/PNG" properties="cover-image"/>
  </manifest>
  <spine><itemref idref="c1"/></spine>
</package>
"#;
    let nav = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">第一章</a></li></ol></nav></body></html>"#;
    let chapter = chapter_html("<p>正文</p>");

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("cover-upper.epub");
    std::fs::write(
        &path,
        build_zip(&[
            ZipEntry::new("mimetype", MIMETYPE),
            ZipEntry::new("META-INF/container.xml", container),
            ZipEntry::new("OEBPS/content.opf", opf),
            ZipEntry::new("OEBPS/nav.xhtml", nav),
            ZipEntry::new("OEBPS/chapter1.xhtml", chapter),
            ZipEntry::new("OEBPS/cover.png", b"\x89PNG\r\n\x1a\n".to_vec()),
        ]),
    )
    .expect("write epub");

    let book = parse_epub(&path).expect("应解析");
    let cover = book.cover.as_ref().expect("大写 MIME 仍应识别");
    assert_eq!(cover.href.as_str(), "OEBPS/cover.png");
    // rbook 会将 manifest media-type 规范化为小写；大小写判断由单元测试覆盖。
    assert_eq!(cover.media_type, "image/png");
}
