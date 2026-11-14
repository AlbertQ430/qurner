mod support;

use std::collections::BTreeSet;
use std::io::Read;

use support::fixtures::{Fixture, FixtureKind};

/// 结构上可被 ZIP 读取器打开的有效夹具（缺失 OPF 仍是合法 ZIP）。
const PARSABLE: [FixtureKind; 7] = [
    FixtureKind::MinimalEpub2,
    FixtureKind::MinimalEpub3,
    FixtureKind::NestedToc,
    FixtureKind::MissingMetadata,
    FixtureKind::CssAndImage,
    FixtureKind::Bilingual,
    FixtureKind::MissingOpf,
];

fn archive(fixture: &Fixture) -> zip::ZipArchive<std::fs::File> {
    let file = std::fs::File::open(fixture.path()).expect("夹具文件应存在");
    zip::ZipArchive::new(file).expect("夹具应是合法 ZIP")
}

fn read_entry(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> String {
    let mut entry = archive.by_name(name).expect("条目应存在");
    let mut content = String::new();
    entry.read_to_string(&mut content).expect("条目应可读取");
    content
}

/// 直接检查第一个 local file header 的条目名和压缩方式。
fn assert_mimetype_first(bytes: &[u8]) {
    assert_eq!(
        &bytes[0..4],
        b"PK\x03\x04",
        "第一个 local file header 应在文件开头"
    );
    let method = u16::from_le_bytes([bytes[8], bytes[9]]);
    assert_eq!(method, 0, "mimetype 必须使用 Stored");
    let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
    let extra_len = u16::from_le_bytes([bytes[28], bytes[29]]) as usize;
    assert_eq!(&bytes[30..30 + name_len], b"mimetype");
    let data_start = 30 + name_len + extra_len;
    assert_eq!(&bytes[data_start..data_start + 20], b"application/epub+zip");
}

#[test]
fn all_fixture_kinds_have_unique_names_and_build_files() {
    let mut seen = BTreeSet::new();
    for kind in FixtureKind::ALL {
        assert!(
            seen.insert(kind.filename()),
            "夹具文件名重复：{}",
            kind.filename()
        );

        let fixture = kind.build();
        assert!(
            fixture.path().exists(),
            "{} 应存在于临时目录",
            kind.filename()
        );
        assert!(fixture.path().starts_with(fixture.dir()));
        assert_eq!(
            fixture.path().file_name().and_then(|name| name.to_str()),
            Some(kind.filename())
        );
        assert!(!fixture.read_bytes().is_empty());
    }
}

#[test]
fn every_fixture_puts_stored_mimetype_first() {
    for kind in FixtureKind::ALL {
        let fixture = kind.build();
        assert_mimetype_first(&fixture.read_bytes());
    }
}

#[test]
fn parsable_fixtures_open_and_expose_stored_mimetype() {
    for kind in PARSABLE {
        let fixture = kind.build();
        let mut archive = archive(&fixture);
        assert!(archive.len() >= 3, "{} 条目过少", kind.filename());

        {
            let first = archive.by_index(0).expect("应存在首个条目");
            assert_eq!(first.name(), "mimetype");
            assert_eq!(first.compression(), zip::CompressionMethod::Stored);
        }

        let mut mimetype = String::new();
        {
            let mut first = archive.by_index(0).expect("应存在首个条目");
            first
                .read_to_string(&mut mimetype)
                .expect("mimetype 应可读取");
        }
        assert_eq!(mimetype, "application/epub+zip");
    }
}

#[test]
fn minimal_epub2_has_ncx_and_opf_v2() {
    let fixture = FixtureKind::MinimalEpub2.build();
    let mut archive = archive(&fixture);
    let opf = read_entry(&mut archive, "OEBPS/content.opf");
    let ncx = read_entry(&mut archive, "OEBPS/toc.ncx");

    assert!(opf.contains("version=\"2.0\""));
    assert!(opf.contains("application/x-dtbncx+xml"));
    assert!(ncx.contains("navMap"));
    assert!(read_entry(&mut archive, "OEBPS/chapter1.xhtml").contains("这是第一章。"));
}

#[test]
fn minimal_epub3_has_nav_property() {
    let fixture = FixtureKind::MinimalEpub3.build();
    let mut archive = archive(&fixture);
    let opf = read_entry(&mut archive, "OEBPS/content.opf");
    let nav = read_entry(&mut archive, "OEBPS/nav.xhtml");

    assert!(opf.contains("version=\"3.0\""));
    assert!(opf.contains("properties=\"nav\""));
    assert!(nav.contains("epub:type=\"toc\""));
}

#[test]
fn nested_toc_fixture_has_at_least_two_levels() {
    let fixture = FixtureKind::NestedToc.build();
    let mut archive = archive(&fixture);
    let nav = read_entry(&mut archive, "OEBPS/nav.xhtml");

    assert!(nav.contains("第一部分"));
    assert!(nav.contains("第一节"));
    assert!(nav.contains("第二部分"));
    assert!(nav.matches("<ol>").count() >= 2, "应存在嵌套列表");
}

#[test]
fn missing_metadata_fixture_omits_title_author_and_cover() {
    let fixture = FixtureKind::MissingMetadata.build();
    let mut archive = archive(&fixture);
    let opf = read_entry(&mut archive, "OEBPS/content.opf");

    assert!(!opf.contains("dc:title"));
    assert!(!opf.contains("dc:creator"));
    assert!(!opf.contains("cover"));
}

#[test]
fn css_and_image_fixture_contains_stylesheet_and_png() {
    let fixture = FixtureKind::CssAndImage.build();
    let mut archive = archive(&fixture);

    let css = read_entry(&mut archive, "OEBPS/style.css");
    assert!(css.contains("font-family"));

    let mut png = Vec::new();
    archive
        .by_name("OEBPS/cover.png")
        .expect("封面图片应存在")
        .read_to_end(&mut png)
        .expect("封面图片应可读取");
    assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn bilingual_fixture_contains_chinese_and_english_text() {
    let fixture = FixtureKind::Bilingual.build();
    let mut archive = archive(&fixture);

    assert!(read_entry(&mut archive, "OEBPS/chapter1.xhtml").contains("这是中文第一章。"));
    assert!(read_entry(&mut archive, "OEBPS/chapter2.xhtml").contains("This is chapter two."));
}

#[test]
fn missing_opf_fixture_omits_referenced_package_document() {
    let fixture = FixtureKind::MissingOpf.build();
    let mut archive = archive(&fixture);

    assert!(archive.by_name("META-INF/container.xml").is_ok());
    assert!(archive.by_name("OEBPS/content.opf").is_err());
}

#[test]
fn unsafe_paths_fixture_contains_traversal_and_absolute_names() {
    let fixture = FixtureKind::UnsafePaths.build();
    let bytes = fixture.read_bytes();
    let text = String::from_utf8_lossy(&bytes);

    assert!(text.contains("../evil.txt"));
    assert!(text.contains("/absolute.txt"));
    assert!(text.contains("C:\\windows\\evil.txt"));
    assert!(text.contains("OEBPS/../../escape.txt"));
}

#[test]
fn duplicate_entries_fixture_contains_two_same_named_entries() {
    let fixture = FixtureKind::DuplicateEntries.build();
    let bytes = fixture.read_bytes();
    let text = String::from_utf8_lossy(&bytes);

    // zip crate 的读取器会按名称去重，因此直接检查 ZIP 字节：
    // 两条同名条目各出现一次 local header 和一次 central directory header。
    assert_eq!(text.matches("OEBPS/chapter1.xhtml").count(), 4);

    let end_of_central_directory = bytes.len() - 22;
    assert_eq!(
        &bytes[end_of_central_directory..end_of_central_directory + 4],
        b"PK\x05\x06"
    );
    let total_entries = u16::from_le_bytes([
        bytes[end_of_central_directory + 10],
        bytes[end_of_central_directory + 11],
    ]);
    assert_eq!(total_entries, 6);
}
