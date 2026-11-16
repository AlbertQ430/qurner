//! 设置继承、全局/单书更新与重置。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use library_core::{
    AppSettingsPatch, BookError, BookSettingsPatch, LibraryService, StorageErrorKind,
};
use reader_model::{ReaderMode, ReaderSettings, ReaderTheme};
use storage_core::StorageService;

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "qurner-settings-{}-{name}-{}",
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
    <dc:title>设置测试书</dc:title>
    <dc:creator>作者</dc:creator>
    <dc:identifier id="id">urn:uuid:settings-test</dc:identifier>
    <meta property="dcterms:modified">2026-11-16T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="c1"/></spine>
</package>
"#;
    build_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", CONTAINER.as_bytes()),
        ("OEBPS/content.opf", opf.as_bytes()),
        (
            "OEBPS/nav.xhtml",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><nav epub:type=\"toc\"><ol><li><a href=\"chapter1.xhtml\">第一章</a></li></ol></nav></body></html>".as_bytes(),
        ),
        (
            "OEBPS/chapter1.xhtml",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>正文</p></body></html>".as_bytes(),
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
        book_id,
    }
}

fn with_service<T>(fixture: &Fixture, action: impl FnOnce(&LibraryService) -> T) -> T {
    let storage = StorageService::open(&fixture.db_path).expect("open db");
    let service = LibraryService::new(storage);
    action(&service)
}

fn app_patch(
    mode: Option<ReaderMode>,
    font_scale: Option<f64>,
    theme: Option<ReaderTheme>,
) -> AppSettingsPatch {
    AppSettingsPatch {
        mode,
        font_scale,
        theme,
    }
}

fn book_patch(
    mode: Option<Option<ReaderMode>>,
    font_scale: Option<Option<f64>>,
    theme: Option<Option<ReaderTheme>>,
) -> BookSettingsPatch {
    BookSettingsPatch {
        mode,
        font_scale,
        theme,
    }
}

fn assert_settings(
    settings: &ReaderSettings,
    mode: ReaderMode,
    font_scale: f64,
    theme: ReaderTheme,
) {
    assert_eq!(settings.mode, mode);
    assert!((settings.font_scale - font_scale).abs() < f64::EPSILON);
    assert_eq!(settings.theme, theme);
}

#[test]
fn defaults_global_book_and_reset_inheritance() {
    let fixture = import_fixture("inherit");
    with_service(&fixture, |service| {
        let initial = service
            .get_effective_settings(&fixture.book_id)
            .expect("defaults");
        assert_settings(&initial, ReaderMode::Paged, 1.0, ReaderTheme::Day);

        let global = service
            .update_app_settings(app_patch(None, Some(1.5), Some(ReaderTheme::Night)))
            .expect("global");
        assert_settings(&global, ReaderMode::Paged, 1.5, ReaderTheme::Night);
        let inherited = service
            .get_effective_settings(&fixture.book_id)
            .expect("inherited");
        assert_settings(&inherited, ReaderMode::Paged, 1.5, ReaderTheme::Night);

        let book = service
            .update_book_settings(
                &fixture.book_id,
                book_patch(Some(Some(ReaderMode::Scrolling)), Some(Some(2.0)), None),
            )
            .expect("book");
        assert_settings(&book, ReaderMode::Scrolling, 2.0, ReaderTheme::Night);

        let global2 = service
            .update_app_settings(app_patch(None, None, Some(ReaderTheme::Day)))
            .expect("global2");
        assert_settings(&global2, ReaderMode::Paged, 1.5, ReaderTheme::Day);
        let merged = service
            .get_effective_settings(&fixture.book_id)
            .expect("merged");
        assert_settings(&merged, ReaderMode::Scrolling, 2.0, ReaderTheme::Day);

        let cleared = service
            .update_book_settings(&fixture.book_id, book_patch(None, Some(None), Some(None)))
            .expect("clear");
        assert_settings(&cleared, ReaderMode::Scrolling, 1.5, ReaderTheme::Day);

        let reset = service
            .reset_book_settings(&fixture.book_id)
            .expect("reset");
        assert_settings(&reset, ReaderMode::Paged, 1.5, ReaderTheme::Day);
        let after_reset = service
            .get_effective_settings(&fixture.book_id)
            .expect("after reset");
        assert_settings(&after_reset, ReaderMode::Paged, 1.5, ReaderTheme::Day);
    });
}

#[test]
fn rejects_invalid_input_and_removed_books() {
    let fixture = import_fixture("invalid");
    with_service(&fixture, |service| {
        assert_eq!(
            service
                .update_book_settings(&fixture.book_id, book_patch(None, None, None))
                .expect_err("empty patch"),
            BookError::InvalidInput
        );
        assert_eq!(
            service
                .update_app_settings(app_patch(None, Some(3.5), None))
                .expect_err("font scale"),
            BookError::InvalidInput
        );
        assert_eq!(
            service
                .get_effective_settings("missing")
                .expect_err("missing"),
            BookError::BookNotFound
        );
        service.soft_remove(&fixture.book_id).expect("soft remove");
        assert_eq!(
            service
                .get_effective_settings(&fixture.book_id)
                .expect_err("removed"),
            BookError::BookNotFound
        );
        assert_eq!(
            service
                .update_book_settings(
                    &fixture.book_id,
                    book_patch(Some(Some(ReaderMode::Paged)), None, None)
                )
                .expect_err("removed update"),
            BookError::BookNotFound
        );
    });
}

#[test]
fn corrupted_persisted_json_is_storage_error_and_is_not_overwritten() {
    let fixture = import_fixture("corrupt");
    let connection = rusqlite::Connection::open(&fixture.db_path).expect("sqlite");
    connection
        .execute_batch("PRAGMA busy_timeout = 5000;")
        .expect("busy");
    connection
        .execute(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES ('reader_defaults', '{bad}', '2026-11-16T00:00:00Z')",
            [],
        )
        .expect("insert corrupt");
    with_service(&fixture, |service| {
        assert_eq!(
            service
                .get_effective_settings(&fixture.book_id)
                .expect_err("corrupt"),
            BookError::Storage(StorageErrorKind::QueryFailed)
        );
        assert_eq!(
            service
                .update_app_settings(app_patch(Some(ReaderMode::Paged), None, None))
                .expect_err("corrupt update"),
            BookError::Storage(StorageErrorKind::QueryFailed)
        );
    });
    let stored: String = connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key='reader_defaults'",
            [],
            |row| row.get(0),
        )
        .expect("stored");
    assert_eq!(stored, "{bad}");
}

#[test]
fn concurrent_patches_merge_without_losing_fields() {
    let fixture = import_fixture("concurrent");
    let db_path = fixture.db_path.clone();
    let book_id = fixture.book_id.clone();
    let service = std::sync::Arc::new({
        let storage = StorageService::open(&db_path).expect("db");
        LibraryService::new(storage)
    });

    let mode_thread = {
        let service = std::sync::Arc::clone(&service);
        let book_id = book_id.clone();
        std::thread::spawn(move || {
            service
                .update_book_settings(
                    &book_id,
                    book_patch(Some(Some(ReaderMode::Scrolling)), None, None),
                )
                .expect("mode patch");
        })
    };
    let scale_thread = {
        let service = std::sync::Arc::clone(&service);
        let book_id = book_id.clone();
        std::thread::spawn(move || {
            service
                .update_book_settings(&book_id, book_patch(None, Some(Some(1.5)), None))
                .expect("scale patch");
        })
    };
    mode_thread.join().expect("mode thread");
    scale_thread.join().expect("scale thread");

    let merged = service
        .get_effective_settings(&book_id)
        .expect("merged settings");
    assert_settings(&merged, ReaderMode::Scrolling, 1.5, ReaderTheme::Day);

    let global_scale = {
        let service = std::sync::Arc::clone(&service);
        std::thread::spawn(move || {
            service
                .update_app_settings(app_patch(None, Some(1.8), None))
                .expect("global scale");
        })
    };
    let global_theme = {
        let service = std::sync::Arc::clone(&service);
        std::thread::spawn(move || {
            service
                .update_app_settings(app_patch(None, None, Some(ReaderTheme::Night)))
                .expect("global theme");
        })
    };
    global_scale.join().expect("global scale thread");
    global_theme.join().expect("global theme thread");

    let global = service
        .get_effective_settings(&book_id)
        .expect("global settings");
    assert_settings(&global, ReaderMode::Scrolling, 1.5, ReaderTheme::Night);

    let connection = rusqlite::Connection::open(&fixture.db_path).expect("sqlite");
    connection
        .execute_batch("PRAGMA busy_timeout = 5000;")
        .expect("busy");
    let json: String = connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key='reader_defaults'",
            [],
            |row| row.get(0),
        )
        .expect("global json");
    let value: serde_json::Value = serde_json::from_str(&json).expect("json");
    let object = value.as_object().expect("object");
    assert!(!object.contains_key("mode"), "全局只保存显式 patch 字段");
    assert!(object.contains_key("fontScale"));
    assert!(object.contains_key("theme"));
}

#[test]
fn semantic_corrupt_settings_fail_without_writing() {
    let fixture = import_fixture("semantic");
    let book_id = fixture.book_id.clone();
    let connection = rusqlite::Connection::open(&fixture.db_path).expect("sqlite");
    connection
        .execute_batch("PRAGMA busy_timeout = 5000;")
        .expect("busy");
    connection
        .execute(
            "INSERT INTO app_settings (key, value_json, updated_at)
             VALUES ('reader_defaults', '{\"mode\":\"paged\",\"extra\":true}', '2026-11-16T00:00:00Z')",
            [],
        )
        .expect("unknown key");
    with_service(&fixture, |service| {
        assert_eq!(
            service
                .update_app_settings(app_patch(None, Some(1.2), None))
                .expect_err("unknown key"),
            BookError::Storage(StorageErrorKind::QueryFailed)
        );
    });
    let (json, updated): (String, String) = connection
        .query_row(
            "SELECT value_json, updated_at FROM app_settings WHERE key='reader_defaults'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("stored");
    assert_eq!(json, "{\"mode\":\"paged\",\"extra\":true}");
    assert_eq!(updated, "2026-11-16T00:00:00Z");

    connection
        .execute(
            "UPDATE app_settings SET value_json='{\"fontScale\":4.0}', updated_at='2026-11-16T00:00:01Z'
             WHERE key='reader_defaults'",
            [],
        )
        .expect("invalid value");
    with_service(&fixture, |service| {
        assert_eq!(
            service
                .update_app_settings(app_patch(None, None, Some(ReaderTheme::Night)))
                .expect_err("invalid value"),
            BookError::Storage(StorageErrorKind::QueryFailed)
        );
    });
    let (json, updated): (String, String) = connection
        .query_row(
            "SELECT value_json, updated_at FROM app_settings WHERE key='reader_defaults'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("stored");
    assert_eq!(json, "{\"fontScale\":4.0}");
    assert_eq!(updated, "2026-11-16T00:00:01Z");

    connection
        .execute(
            "INSERT INTO book_settings (book_id, mode, font_scale, theme, updated_at)
             VALUES (?1, 'sepia', NULL, NULL, '2026-11-16T00:00:02Z')",
            [&book_id],
        )
        .expect("invalid book");
    with_service(&fixture, |service| {
        assert_eq!(
            service
                .update_book_settings(&book_id, book_patch(None, Some(Some(1.3)), None))
                .expect_err("invalid book"),
            BookError::Storage(StorageErrorKind::QueryFailed)
        );
    });
    let (mode, font_scale, updated): (String, Option<f64>, String) = connection
        .query_row(
            "SELECT mode, font_scale, updated_at FROM book_settings WHERE book_id=?1",
            [&book_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("book row");
    assert_eq!(mode, "sepia");
    assert_eq!(font_scale, None);
    assert_eq!(updated, "2026-11-16T00:00:02Z");
}

#[test]
fn writes_touch_only_settings_tables_and_updated_at() {
    let fixture = import_fixture("atomic");
    let book_id = fixture.book_id.clone();
    with_service(&fixture, |service| {
        service
            .update_app_settings(app_patch(None, Some(1.2), None))
            .expect("global");
        service
            .update_book_settings(
                &book_id,
                book_patch(Some(Some(ReaderMode::Scrolling)), None, None),
            )
            .expect("book");
    });
    let connection = rusqlite::Connection::open(&fixture.db_path).expect("sqlite");
    connection
        .execute_batch("PRAGMA busy_timeout = 5000;")
        .expect("busy");
    let (book_count, state_count): (i64, i64) = connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM books), (SELECT COUNT(*) FROM reading_states)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("counts");
    assert_eq!(book_count, 1);
    assert_eq!(state_count, 0);
    let app_updated: String = connection
        .query_row(
            "SELECT updated_at FROM app_settings WHERE key='reader_defaults'",
            [],
            |row| row.get(0),
        )
        .expect("app updated_at");
    let book_updated: String = connection
        .query_row(
            "SELECT updated_at FROM book_settings WHERE book_id=?1",
            [&book_id],
            |row| row.get(0),
        )
        .expect("book updated_at");
    assert!(app_updated.ends_with('Z'));
    assert!(book_updated.ends_with('Z'));
}
