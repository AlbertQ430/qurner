//! 书签用例：稳定排序、excerpt 边界、损坏记录与不变量。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use library_core::{BookError, LibraryService, StorageErrorKind};
use reader_model::ReaderLocator;
use storage_core::{BookmarkRecord, StorageError, StorageService};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "qurner-bookmarks-{}-{name}-{}",
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

fn build_epub() -> Vec<u8> {
    let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>书签测试书</dc:title>
    <dc:creator>作者</dc:creator>
    <dc:identifier id="id">urn:uuid:bookmark-test</dc:identifier>
    <meta property="dcterms:modified">2026-11-18T00:00:00Z</meta>
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
        (
            "OEBPS/nav.xhtml",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><nav epub:type=\"toc\"><ol><li><a href=\"chapter1.xhtml\">第一章</a></li><li><a href=\"chapter2.xhtml\">第二章</a></li></ol></nav></body></html>".as_bytes(),
        ),
        (
            "OEBPS/chapter1.xhtml",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>正文一</p></body></html>".as_bytes(),
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
    _directory: TempDir,
    db_path: PathBuf,
    epub_path: PathBuf,
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
        _directory: directory,
        db_path,
        epub_path,
        book_id,
    }
}

fn locator(spine_index: u32, href: &str, offset: u64, exact: &str) -> ReaderLocator {
    ReaderLocator {
        version: 1,
        spine_index,
        spine_href: href.to_string(),
        text_offset: offset,
        intra_spine_progression: 0.2,
        quote: reader_model::ReaderQuoteContext {
            exact: exact.to_string(),
            prefix: String::new(),
            suffix: String::new(),
        },
    }
}

fn with_service<T>(fixture: &Fixture, action: impl FnOnce(&LibraryService) -> T) -> T {
    let storage = StorageService::open(&fixture.db_path).expect("open db");
    let service = LibraryService::new(storage);
    action(&service)
}

#[test]
fn stable_order_and_excerpt_truncation() {
    let fixture = import_fixture("order");
    let first = with_service(&fixture, |service| {
        service
            .create_bookmark(
                &fixture.book_id,
                &locator(0, "OEBPS/chapter1.xhtml", 1, "第一"),
                None,
            )
            .expect("first")
    });
    let second = with_service(&fixture, |service| {
        service
            .create_bookmark(
                &fixture.book_id,
                &locator(1, "OEBPS/chapter2.xhtml", 2, ""),
                Some(&"字".repeat(200)),
            )
            .expect("second")
    });
    assert_eq!(second.excerpt.as_deref(), Some("字".repeat(128).as_str()));
    assert!(first.excerpt.is_some());

    // 直接用不同 created_at 验证排序：created_at DESC, id ASC。
    let connection = rusqlite::Connection::open(&fixture.db_path).expect("sqlite");
    connection
        .execute_batch("PRAGMA busy_timeout = 5000;")
        .expect("busy");
    connection
        .execute(
            "UPDATE bookmarks SET created_at='2026-11-18T00:00:02Z' WHERE id=?1",
            [&first.id],
        )
        .expect("update first");
    connection
        .execute(
            "UPDATE bookmarks SET created_at='2026-11-18T00:00:01Z' WHERE id=?1",
            [&second.id],
        )
        .expect("update second");
    let listed = with_service(&fixture, |service| {
        service.list_bookmarks(&fixture.book_id).expect("list")
    });
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, first.id);
    assert_eq!(listed[1].id, second.id);
}

#[test]
fn duplicate_ids_and_transaction_failures_leave_no_partial_rows() {
    let fixture = import_fixture("failures");
    let storage = StorageService::open(&fixture.db_path).expect("open db");
    let connection = rusqlite::Connection::open(&fixture.db_path).expect("sqlite");
    connection
        .execute_batch("PRAGMA busy_timeout = 5000;")
        .expect("busy");
    let locator_json =
        serde_json::to_string(&locator(0, "OEBPS/chapter1.xhtml", 1, "重复")).expect("json");
    let record = BookmarkRecord {
        id: "123e4567-e89b-42d3-a456-426614174000".to_string(),
        book_id: fixture.book_id.clone(),
        locator_json: locator_json.clone(),
        locator_version: 1,
        excerpt: Some("重复".to_string()),
        created_at: "2026-11-18T00:00:00Z".to_string(),
    };
    storage
        .insert_bookmark(record.clone())
        .expect("first insert");
    assert_eq!(
        storage.insert_bookmark(record).expect_err("duplicate id"),
        StorageError::Conflict
    );

    let missing_book = BookmarkRecord {
        id: "223e4567-e89b-42d3-a456-426614174000".to_string(),
        book_id: "323e4567-e89b-42d3-a456-426614174000".to_string(),
        locator_json,
        locator_version: 1,
        excerpt: None,
        created_at: "2026-11-18T00:00:01Z".to_string(),
    };
    assert_eq!(
        storage
            .insert_bookmark(missing_book)
            .expect_err("foreign key"),
        StorageError::Conflict
    );

    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 1, "失败插入不得留下部分行");
    assert!(
        !storage
            .delete_bookmark(&fixture.book_id, "323e4567-e89b-42d3-a456-426614174000")
            .expect("delete missing")
    );
}

#[test]
fn corrupt_records_and_removed_books_error_without_writes() {
    let fixture = import_fixture("corrupt");
    let before = std::fs::read(&fixture.epub_path).expect("epub");
    with_service(&fixture, |service| {
        service
            .create_bookmark(
                &fixture.book_id,
                &locator(0, "OEBPS/chapter1.xhtml", 1, "摘录"),
                None,
            )
            .expect("create");
    });
    let connection = rusqlite::Connection::open(&fixture.db_path).expect("sqlite");
    connection
        .execute_batch("PRAGMA busy_timeout = 5000;")
        .expect("busy");
    connection
        .execute(
            "INSERT INTO bookmarks (id, book_id, locator_json, locator_version, excerpt, created_at)
             VALUES ('123e4567-e89b-42d3-a456-426614174000', ?1, '{bad}', 1, NULL, '2026-11-18T00:00:00Z')",
            [&fixture.book_id],
        )
        .expect("corrupt");
    with_service(&fixture, |service| {
        assert_eq!(
            service
                .list_bookmarks(&fixture.book_id)
                .expect_err("corrupt locator"),
            BookError::Storage(StorageErrorKind::QueryFailed)
        );
        service.soft_remove(&fixture.book_id).expect("soft remove");
        assert_eq!(
            service
                .list_bookmarks(&fixture.book_id)
                .expect_err("removed"),
            BookError::BookNotFound
        );
        assert_eq!(
            service
                .create_bookmark(
                    &fixture.book_id,
                    &locator(0, "OEBPS/chapter1.xhtml", 0, "x"),
                    None,
                )
                .expect_err("removed create"),
            BookError::BookNotFound
        );
    });
    let after = std::fs::read(&fixture.epub_path).expect("epub after");
    assert_eq!(before, after);
    let reading_states: i64 = connection
        .query_row("SELECT COUNT(*) FROM reading_states", [], |row| row.get(0))
        .expect("reading states");
    assert_eq!(reading_states, 0);
    let settings: i64 = connection
        .query_row("SELECT COUNT(*) FROM book_settings", [], |row| row.get(0))
        .expect("settings");
    assert_eq!(settings, 0);
}
