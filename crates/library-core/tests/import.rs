use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use epub_core::fingerprint_file;
use library_core::{ImportStatus, LibraryService};
use storage_core::StorageService;

const CONTAINER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

const NAV: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">第一章</a></li></ol></nav></body></html>"#;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "qurner-library-import-{}-{name}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("创建临时目录");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn build_epub(title: Option<&str>, author: Option<&str>, body: &str) -> Vec<u8> {
    let mut metadata = String::new();
    if let Some(title) = title {
        metadata.push_str(&format!("    <dc:title>{title}</dc:title>\n"));
    }
    if let Some(author) = author {
        metadata.push_str(&format!("    <dc:creator>{author}</dc:creator>\n"));
    }

    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
{metadata}    <dc:identifier id="id">urn:uuid:import-test</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="c1"/></spine>
</package>
"#
    );
    let chapter = format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title></title></head><body><p>{body}</p></body></html>"
    );

    build_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", CONTAINER.as_bytes()),
        ("OEBPS/content.opf", opf.as_bytes()),
        ("OEBPS/nav.xhtml", NAV.as_bytes()),
        ("OEBPS/chapter1.xhtml", chapter.as_bytes()),
    ])
}

fn build_broken_epub() -> Vec<u8> {
    build_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", CONTAINER.as_bytes()),
        (
            "OEBPS/chapter1.xhtml",
            b"<html><body><p>broken</p></body></html>",
        ),
    ])
}

fn write_file(directory: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, bytes).expect("写测试文件");
    path
}

fn open_service(database: &Path) -> StorageService {
    StorageService::open(database).expect("打开数据库")
}

#[test]
fn imports_six_valid_fixtures() {
    let directory = TempDir::new("six");
    let storage = open_service(&directory.path.join("reader.sqlite3"));
    let service = LibraryService::new(storage.clone());

    let mut book_ids = Vec::new();
    for index in 0..6 {
        let path = write_file(
            &directory.path,
            &format!("book-{index}.epub"),
            &build_epub(
                Some(&format!("书名{index}")),
                Some("作者"),
                &format!("正文{index}"),
            ),
        );
        let result = service.import(&path);
        assert_eq!(result.status, ImportStatus::Imported, "第 {index} 本");
        assert!(result.error.is_none());
        let book_id = result.book_id.expect("book id");
        book_ids.push(book_id);
    }

    book_ids.sort();
    book_ids.dedup();
    assert_eq!(book_ids.len(), 6, "每本书应获得不同 ID");

    storage.shutdown().expect("shutdown");
}

#[test]
fn same_bytes_different_names_is_duplicate() {
    let directory = TempDir::new("duplicate");
    let storage = open_service(&directory.path.join("reader.sqlite3"));
    let service = LibraryService::new(storage.clone());

    let bytes = build_epub(Some("重复书"), Some("作者"), "正文");
    let first = write_file(&directory.path, "a.epub", &bytes);
    let second = write_file(&directory.path, "b.epub", &bytes);

    let first_result = service.import(&first);
    assert_eq!(first_result.status, ImportStatus::Imported);

    let second_result = service.import(&second);
    assert_eq!(second_result.status, ImportStatus::Duplicate);
    assert_eq!(second_result.book_id, first_result.book_id);

    storage.shutdown().expect("shutdown");
}

#[test]
fn one_byte_change_creates_new_book() {
    let directory = TempDir::new("changed");
    let storage = open_service(&directory.path.join("reader.sqlite3"));
    let service = LibraryService::new(storage.clone());

    let first = write_file(
        &directory.path,
        "book.epub",
        &build_epub(Some("书"), Some("作者"), "正文一"),
    );
    let second = write_file(
        &directory.path,
        "book-v2.epub",
        &build_epub(Some("书"), Some("作者"), "正文二"),
    );

    let first_result = service.import(&first);
    let second_result = service.import(&second);
    assert_eq!(first_result.status, ImportStatus::Imported);
    assert_eq!(second_result.status, ImportStatus::Imported);
    assert_ne!(first_result.book_id, second_result.book_id);

    storage.shutdown().expect("shutdown");
}

#[test]
fn removed_book_is_reactivated_on_reimport() {
    let directory = TempDir::new("reactivate");
    let storage = open_service(&directory.path.join("reader.sqlite3"));
    let service = LibraryService::new(storage.clone());

    let path = write_file(
        &directory.path,
        "book.epub",
        &build_epub(Some("书"), Some("作者"), "正文"),
    );
    let imported = service.import(&path);
    assert_eq!(imported.status, ImportStatus::Imported);
    let book_id = imported.book_id.expect("book id");

    service.soft_remove(&book_id).expect("soft remove");
    let fingerprint = fingerprint_file(&path).expect("fingerprint");
    let removed = storage
        .find_by_fingerprint(&fingerprint.to_string())
        .expect("find")
        .expect("exists");
    assert_eq!(removed.lifecycle, "removed");

    let reactivated = service.import(&path);
    assert_eq!(reactivated.status, ImportStatus::Reactivated);
    assert_eq!(reactivated.book_id.as_deref(), Some(book_id.as_str()));

    let active = storage
        .find_by_fingerprint(&fingerprint.to_string())
        .expect("find")
        .expect("exists");
    assert_eq!(active.lifecycle, "active");

    storage.shutdown().expect("shutdown");
}

#[test]
fn parse_failure_leaves_no_records() {
    let directory = TempDir::new("failed");
    let storage = open_service(&directory.path.join("reader.sqlite3"));
    let service = LibraryService::new(storage.clone());

    let bytes = build_broken_epub();
    let path = write_file(&directory.path, "broken.epub", &bytes);
    let fingerprint = fingerprint_file(&path).expect("fingerprint");

    let result = service.import(&path);
    assert_eq!(result.status, ImportStatus::Failed);
    assert!(result.error.is_some());
    let error = result.error.expect("error").to_string();
    assert!(!error.contains("/Users/"));
    assert!(!error.contains(path.to_str().expect("utf8")));

    assert!(
        storage
            .find_by_fingerprint(&fingerprint.to_string())
            .expect("find")
            .is_none(),
        "失败导入不应留下记录"
    );

    storage.shutdown().expect("shutdown");
}

#[test]
fn batch_results_keep_input_order() {
    let directory = TempDir::new("batch");
    let storage = open_service(&directory.path.join("reader.sqlite3"));
    let service = LibraryService::new(storage.clone());

    let success = write_file(
        &directory.path,
        "success.epub",
        &build_epub(Some("成功"), Some("作者"), "正文"),
    );
    let duplicate = write_file(
        &directory.path,
        "duplicate.epub",
        &build_epub(Some("成功"), Some("作者"), "正文"),
    );
    let failed = write_file(&directory.path, "failed.epub", &build_broken_epub());

    let results = service.import_batch(&[success, duplicate, failed]);
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].status, ImportStatus::Imported);
    assert_eq!(results[1].status, ImportStatus::Duplicate);
    assert_eq!(results[2].status, ImportStatus::Failed);

    storage.shutdown().expect("shutdown");
}

#[test]
fn concurrent_imports_of_same_file_produce_single_import() {
    let directory = TempDir::new("concurrent");
    let storage = open_service(&directory.path.join("reader.sqlite3"));
    let service = Arc::new(LibraryService::new(storage.clone()));
    let path = Arc::new(write_file(
        &directory.path,
        "book.epub",
        &build_epub(Some("并发"), Some("作者"), "正文"),
    ));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let service = Arc::clone(&service);
            let path = Arc::clone(&path);
            std::thread::spawn(move || service.import(&path))
        })
        .collect();

    let mut imported = 0;
    let mut duplicates = 0;
    for handle in handles {
        let result = handle.join().expect("join");
        match result.status {
            ImportStatus::Imported => imported += 1,
            ImportStatus::Duplicate => duplicates += 1,
            other => panic!("意外状态：{other:?}"),
        }
    }
    assert_eq!(imported, 1, "只应有一本成功写入");
    assert_eq!(duplicates, 1, "后完成者应返回 Duplicate");

    storage.shutdown().expect("shutdown");
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
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[test]
fn concurrent_reimport_of_removed_book_is_reactivated_and_duplicate() {
    let directory = TempDir::new("concurrent-reactivate");
    let storage = open_service(&directory.path.join("reader.sqlite3"));
    let service = Arc::new(LibraryService::new(storage.clone()));
    let path = Arc::new(write_file(
        &directory.path,
        "book.epub",
        &build_epub(Some("并发恢复"), Some("作者"), "正文"),
    ));

    let imported = service.import(&path);
    assert_eq!(imported.status, ImportStatus::Imported);
    let book_id = imported.book_id.expect("book id");
    service.soft_remove(&book_id).expect("soft remove");

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let service = Arc::clone(&service);
            let path = Arc::clone(&path);
            std::thread::spawn(move || service.import(&path))
        })
        .collect();

    let mut statuses = Vec::new();
    for handle in handles {
        let result = handle.join().expect("join");
        assert!(
            result.status == ImportStatus::Reactivated || result.status == ImportStatus::Duplicate,
            "意外状态：{:?}",
            result.status
        );
        statuses.push((result.status, result.book_id.expect("book id")));
    }

    let reactivated = statuses
        .iter()
        .filter(|(status, _)| *status == ImportStatus::Reactivated)
        .count();
    let duplicates = statuses
        .iter()
        .filter(|(status, _)| *status == ImportStatus::Duplicate)
        .count();
    assert_eq!((reactivated, duplicates), (1, 1));
    assert_eq!(statuses[0].1, statuses[1].1, "两个结果应返回同一 book_id");
    assert_eq!(statuses[0].1, book_id);

    let fingerprint = fingerprint_file(&path).expect("fingerprint");
    let record = storage
        .find_by_fingerprint(&fingerprint.to_string())
        .expect("find")
        .expect("exists");
    assert_eq!(record.lifecycle, "active");
    assert_eq!(record.id, book_id);

    storage.shutdown().expect("shutdown");
}
