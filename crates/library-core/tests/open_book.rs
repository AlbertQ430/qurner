//! 打开书籍用例：源文件校验、TOC 映射、初始位置与设置合并。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use library_core::{BookError, LibraryService, OpenBook, StorageErrorKind};
use storage_core::StorageService;

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "qurner-open-book-{}-{name}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("temp");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

const CONTAINER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

const NAV: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">第一章</a><ol><li><a href="chapter1.xhtml#s1">第一节</a></li></ol></li><li><a href="chapter2.xhtml">第二章</a></li><li><a href="chapter2.xhtml#">第二章（空片段）</a></li></ol></nav></body></html>"#;

fn build_epub() -> Vec<u8> {
    let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>打开测试书</dc:title>
    <dc:creator>作者</dc:creator>
    <dc:identifier id="id">urn:uuid:open-test</dc:identifier>
    <meta property="dcterms:modified">2026-11-15T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="c1"/><itemref idref="c2"/></spine>
</package>
"#;
    build_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", CONTAINER.as_bytes()),
        ("OEBPS/content.opf", opf.as_bytes()),
        ("OEBPS/nav.xhtml", NAV.as_bytes()),
        (
            "OEBPS/chapter1.xhtml",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><h1 id=\"s1\">第一节</h1><p>正文一</p></body></html>".as_bytes(),
        ),
        (
            "OEBPS/chapter2.xhtml",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>正文二</p></body></html>".as_bytes(),
        ),
    ])
}

fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut central = Vec::new();
    let mut offset = 0_u32;
    for (name, data) in entries {
        let name = name.as_bytes();
        let size = data.len() as u32;
        let crc = crc32(data);
        output.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
        output.extend_from_slice(&20_u16.to_le_bytes());
        output.extend_from_slice(&0x0800_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0x0021_u16.to_le_bytes());
        output.extend_from_slice(&crc.to_le_bytes());
        output.extend_from_slice(&size.to_le_bytes());
        output.extend_from_slice(&size.to_le_bytes());
        output.extend_from_slice(&(name.len() as u16).to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(name);
        output.extend_from_slice(data);
        central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
        central.extend_from_slice(&20_u16.to_le_bytes());
        central.extend_from_slice(&20_u16.to_le_bytes());
        central.extend_from_slice(&0x0800_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0x0021_u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
        offset += 30 + name.len() as u32 + size;
    }
    let central_offset = offset;
    output.extend_from_slice(&central);
    output.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    output.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    output.extend_from_slice(&(central.len() as u32).to_le_bytes());
    output.extend_from_slice(&central_offset.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

struct Fixture {
    directory: TempDir,
    epub_path: PathBuf,
    db_path: PathBuf,
    book_id: String,
}

fn import_fixture(name: &str) -> Fixture {
    let directory = TempDir::new(name);
    let epub_path = directory.path.join("book.epub");
    std::fs::write(&epub_path, build_epub()).expect("epub");
    let db_path = directory.path.join("reader.sqlite3");
    let storage = StorageService::open(&db_path).expect("db");
    let service = LibraryService::new(storage);
    let result = service.import(&epub_path);
    let book_id = result.book_id.expect("book id");
    drop(service);
    Fixture {
        directory,
        epub_path,
        db_path,
        book_id,
    }
}

fn open(fixture: &Fixture) -> Result<OpenBook, BookError> {
    let storage = StorageService::open(&fixture.db_path).expect("open db");
    let service = LibraryService::new(storage);
    service.open_book(&fixture.book_id)
}

fn sql(fixture: &Fixture, statement: &str) {
    let connection = rusqlite::Connection::open(&fixture.db_path).expect("sqlite");
    connection
        .execute_batch("PRAGMA busy_timeout = 5000;")
        .expect("busy");
    connection.execute_batch(statement).expect("sql");
}

#[test]
fn opens_book_with_toc_defaults_and_initial_position() {
    let fixture = import_fixture("success");
    let before = std::fs::read(&fixture.epub_path).expect("read before");
    let opened = open(&fixture).expect("open");
    assert_eq!(opened.title, "打开测试书");
    assert_eq!(opened.toc.len(), 3);
    assert_eq!(opened.toc[0].label, "第一章");
    assert_eq!(opened.toc[0].spine_index, 0);
    assert_eq!(opened.toc[0].children[0].fragment.as_deref(), Some("s1"));
    assert_eq!(opened.toc[1].spine_index, 1);
    assert_eq!(opened.toc[2].label, "第二章（空片段）");
    assert!(opened.toc[2].fragment.is_none(), "空 fragment 必须省略");
    let toc_json = serde_json::to_string(&opened.toc).expect("toc json");
    assert!(
        !toc_json.contains("\"fragment\":\"\""),
        "不得序列化空 fragment"
    );
    assert!(opened.initial_locator.is_none());
    assert_eq!(opened.initial_progression, 0.0);
    assert_eq!(opened.settings.mode, reader_model::ReaderMode::Paged);
    assert_eq!(opened.settings.font_scale, 1.0);
    assert_eq!(opened.settings.theme, reader_model::ReaderTheme::Day);
    let after = std::fs::read(&fixture.epub_path).expect("read after");
    assert_eq!(before, after, "打开不得修改原文件");
}

#[test]
fn loads_valid_locator_and_progression() {
    let fixture = import_fixture("locator");
    sql(
        &fixture,
        "INSERT INTO reading_states (book_id, locator_json, locator_version, global_progression, updated_at) VALUES ('BOOK_ID', '{\"version\":1,\"spineIndex\":0,\"spineHref\":\"OEBPS/chapter1.xhtml\",\"textOffset\":4,\"intraSpineProgression\":0.2,\"quote\":{\"exact\":\"正文\",\"prefix\":\"\",\"suffix\":\"\"}}', 1, 0.4, '2026-11-15T00:00:00Z');"
            .replace("BOOK_ID", &fixture.book_id)
            .as_str(),
    );
    let opened = open(&fixture).expect("open");
    let locator = opened.initial_locator.expect("locator");
    assert_eq!(locator.spine_index, 0);
    assert_eq!(locator.spine_href, "OEBPS/chapter1.xhtml");
    assert!((opened.initial_progression - 0.4).abs() < f64::EPSILON);
}

#[test]
fn rejects_invalid_locator_and_progression() {
    for (name, statement) in [
        (
            "locator-version",
            "INSERT INTO reading_states (book_id, locator_json, locator_version, global_progression, updated_at) VALUES ('BOOK_ID', '{\"version\":2,\"spineIndex\":0,\"spineHref\":\"OEBPS/chapter1.xhtml\",\"textOffset\":0,\"intraSpineProgression\":0,\"quote\":{\"exact\":\"\",\"prefix\":\"\",\"suffix\":\"\"}}', 1, 0.1, '2026-11-15T00:00:00Z');",
        ),
        (
            "locator-spine",
            "INSERT INTO reading_states (book_id, locator_json, locator_version, global_progression, updated_at) VALUES ('BOOK_ID', '{\"version\":1,\"spineIndex\":9,\"spineHref\":\"OEBPS/missing.xhtml\",\"textOffset\":0,\"intraSpineProgression\":0,\"quote\":{\"exact\":\"\",\"prefix\":\"\",\"suffix\":\"\"}}', 1, 0.1, '2026-11-15T00:00:00Z');",
        ),
        (
            "progression",
            "INSERT INTO reading_states (book_id, locator_json, locator_version, global_progression, updated_at) VALUES ('BOOK_ID', '{\"version\":1,\"spineIndex\":0,\"spineHref\":\"OEBPS/chapter1.xhtml\",\"textOffset\":0,\"intraSpineProgression\":0,\"quote\":{\"exact\":\"\",\"prefix\":\"\",\"suffix\":\"\"}}', 1, 1.5, '2026-11-15T00:00:00Z');",
        ),
    ] {
        let fixture = import_fixture(name);
        sql(
            &fixture,
            statement.replace("BOOK_ID", &fixture.book_id).as_str(),
        );
        match open(&fixture) {
            Err(BookError::Storage(StorageErrorKind::QueryFailed)) => {}
            other => panic!("{name} 应返回 storage 错误，实际 {other:?}"),
        }
    }
}

#[test]
fn merges_global_and_book_settings() {
    let fixture = import_fixture("settings");
    sql(
        &fixture,
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES ('reader_defaults', '{\"mode\":\"scrolling\",\"fontScale\":1.25,\"theme\":\"night\"}', '2026-11-15T00:00:00Z');",
    );
    sql(
        &fixture,
        &format!(
            "INSERT INTO book_settings (book_id, mode, font_scale, theme, updated_at) VALUES ('{}', 'paged', NULL, NULL, '2026-11-15T00:00:00Z');",
            fixture.book_id
        ),
    );
    let opened = open(&fixture).expect("open");
    assert_eq!(opened.settings.mode, reader_model::ReaderMode::Paged);
    assert!((opened.settings.font_scale - 1.25).abs() < f64::EPSILON);
    assert_eq!(opened.settings.theme, reader_model::ReaderTheme::Night);
}

#[test]
fn rejects_invalid_persisted_settings() {
    for (name, statement) in [
        (
            "font-global",
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES ('reader_defaults', '{\"fontScale\":4.0}', '2026-11-15T00:00:00Z');"
                .to_string(),
        ),
        (
            "theme-book",
            "INSERT INTO book_settings (book_id, mode, font_scale, theme, updated_at) VALUES ('BOOK_ID', NULL, NULL, 'sepia', '2026-11-15T00:00:00Z');"
                .to_string(),
        ),
    ] {
        let fixture = import_fixture(name);
        let statement = statement.replace("BOOK_ID", &fixture.book_id);
        sql(&fixture, &statement);
        match open(&fixture) {
            Err(BookError::Storage(StorageErrorKind::QueryFailed)) => {}
            other => panic!("{name} 应返回 storage 错误，实际 {other:?}"),
        }
    }
}

#[test]
fn maps_missing_removed_and_changed_books() {
    let missing = import_fixture("missing");
    std::fs::remove_file(&missing.epub_path).expect("remove");
    assert_eq!(
        open(&missing).expect_err("missing"),
        BookError::BookUnavailable
    );

    let removed = import_fixture("removed");
    sql(
        &removed,
        &format!(
            "UPDATE books SET lifecycle='removed' WHERE id='{}';",
            removed.book_id
        ),
    );
    assert_eq!(
        open(&removed).expect_err("removed"),
        BookError::BookNotFound
    );

    let unreadable = import_fixture("unreadable");
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&unreadable.epub_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o000);
        std::fs::set_permissions(&unreadable.epub_path, permissions).expect("chmod");
    }
    assert_eq!(
        open(&unreadable).expect_err("unreadable"),
        BookError::BookUnavailable
    );
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&unreadable.epub_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o644);
        std::fs::set_permissions(&unreadable.epub_path, permissions).expect("restore permissions");
    }

    let changed = import_fixture("changed");
    let mut bytes = std::fs::read(&changed.epub_path).expect("read");
    bytes.extend_from_slice(b"changed");
    std::fs::write(&changed.epub_path, bytes).expect("write");
    assert_eq!(
        open(&changed).expect_err("changed"),
        BookError::SourceChanged
    );

    let directory = import_fixture("directory");
    let directory_path = directory.directory.path.to_string_lossy().into_owned();
    sql(
        &directory,
        &format!(
            "UPDATE books SET source_path='{}' WHERE id='{}';",
            directory_path, directory.book_id
        ),
    );
    assert_eq!(
        open(&directory).expect_err("directory"),
        BookError::BookUnavailable
    );

    let corrupt = import_fixture("corrupt");
    sql(
        &corrupt,
        &format!(
            "UPDATE books SET manifest_json='{{bad}}' WHERE id='{}';",
            corrupt.book_id
        ),
    );
    assert_eq!(
        open(&corrupt).expect_err("corrupt"),
        BookError::Storage(StorageErrorKind::QueryFailed)
    );

    let version = import_fixture("version");
    sql(
        &version,
        &format!(
            "UPDATE books SET manifest_version=999 WHERE id='{}';",
            version.book_id
        ),
    );
    assert_eq!(
        open(&version).expect_err("version"),
        BookError::Storage(StorageErrorKind::QueryFailed)
    );
}

#[test]
fn rejects_blank_book_id() {
    let fixture = import_fixture("blank");
    let storage = StorageService::open(&fixture.db_path).expect("db");
    let service = LibraryService::new(storage);
    assert_eq!(
        service.open_book("   ").expect_err("blank"),
        BookError::InvalidInput
    );
}
