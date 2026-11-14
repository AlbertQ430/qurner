//! 导入时封面缓存：生成、缩放、失败回退与重复导入不重建。

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use image::{ImageFormat, Rgb, RgbImage};
use library_core::{CoverCache, ImportStatus, LibraryService};
use storage_core::StorageService;

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "qurner-cover-cache-{}-{name}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("创建临时目录");
        Self { path }
    }

    fn covers(&self) -> PathBuf {
        self.path.join("covers")
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

const NAV: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">第一章</a></li></ol></nav></body></html>"#;

fn png_1x1() -> Vec<u8> {
    let image = RgbImage::from_pixel(1, 1, Rgb([200, 120, 40]));
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .expect("编码 PNG");
    bytes
}

fn png_large() -> Vec<u8> {
    let image = RgbImage::from_fn(600, 400, |x, y| {
        Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    });
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .expect("编码 PNG");
    bytes
}

fn build_epub(cover: Option<(&str, &[u8])>) -> Vec<u8> {
    let mut manifest = String::from(
        r#"    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
"#,
    );
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", CONTAINER.as_bytes()),
        ("OEBPS/nav.xhtml", NAV.as_bytes()),
        (
            "OEBPS/chapter1.xhtml",
            b"<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>text</p></body></html>",
        ),
    ];
    if let Some((mime, bytes)) = cover {
        let name = "cover.img";
        manifest.push_str(&format!(
            "    <item id=\"cover\" href=\"{name}\" media-type=\"{mime}\" properties=\"cover-image\"/>\n"
        ));
        entries.push(("OEBPS/cover.img", bytes));
    }
    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>封面测试</dc:title>
    <dc:creator>作者</dc:creator>
    <dc:identifier id="id">urn:uuid:cover-test</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
{manifest}  </manifest>
  <spine><itemref idref="c1"/></spine>
</package>
"#
    );
    entries.push(("OEBPS/content.opf", opf.as_bytes()));
    build_zip(&entries)
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

fn service(directory: &Path) -> (LibraryService, String) {
    let storage_path = directory.join("reader.sqlite3");
    let storage = StorageService::open(&storage_path).expect("打开数据库");
    let covers = CoverCache::new(directory.join("covers")).expect("创建封面目录");
    let service = LibraryService::with_cover_cache(storage, covers);
    let epub_path = directory.join("book.epub");
    std::fs::write(&epub_path, build_epub(Some(("image/png", &png_1x1())))).expect("写 EPUB");
    (service, epub_path.to_string_lossy().into_owned())
}

fn summary(service: &LibraryService) -> Vec<reader_model::BookSummary> {
    service
        .list_books(reader_model::ListBooksQuery {
            query: None,
            sort: reader_model::BookSort::ImportedAt,
            direction: reader_model::SortDirection::Desc,
        })
        .expect("列书库")
}

#[test]
fn generates_small_cover_jpeg_with_fingerprint_key() {
    let temp = TempDir::new("small");
    let (service, epub_path) = service(&temp.path);
    let result = service.import(Path::new(&epub_path));
    assert_eq!(result.status, ImportStatus::Imported);

    let books = summary(&service);
    let key = books[0].cover_cache_key.clone().expect("应有缓存键");
    assert_eq!(key.len(), 64);
    assert!(
        key.bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    );

    let cached = temp.covers().join(format!("{key}.jpg"));
    let bytes = std::fs::read(&cached).expect("缓存文件应存在");
    let image = image::load_from_memory(&bytes).expect("缓存应是有效 JPEG");
    assert_eq!(image.width(), 1);
    assert_eq!(image.height(), 1);
}

#[test]
fn scales_large_covers_to_512_without_upscaling() {
    let temp = TempDir::new("large");
    let storage = StorageService::open(&temp.path.join("reader.sqlite3")).expect("db");
    let covers = CoverCache::new(temp.covers()).expect("covers");
    let service = LibraryService::with_cover_cache(storage, covers);
    let epub_path = temp.path.join("large.epub");
    std::fs::write(&epub_path, build_epub(Some(("image/png", &png_large())))).expect("写 EPUB");

    let result = service.import(&epub_path);
    assert_eq!(result.status, ImportStatus::Imported);
    let key = summary(&service)[0]
        .cover_cache_key
        .clone()
        .expect("缓存键");
    let bytes = std::fs::read(temp.covers().join(format!("{key}.jpg"))).expect("读缓存");
    let image = image::load_from_memory(&bytes).expect("JPEG");
    assert_eq!(image.width().max(image.height()), 512);
    assert!(image.width() <= 512 && image.height() <= 512);
    // 600x400 等比缩放到 512x341。
    assert_eq!((image.width(), image.height()), (512, 341));
}

#[test]
fn missing_or_corrupt_covers_do_not_block_import() {
    for (name, cover) in [
        ("none", None),
        ("corrupt", Some(("image/png", b"not-an-image".as_slice()))),
        ("unsupported", Some(("image/svg+xml", b"<svg/>".as_slice()))),
    ] {
        let temp = TempDir::new(name);
        let storage = StorageService::open(&temp.path.join("reader.sqlite3")).expect("db");
        let covers = CoverCache::new(temp.covers()).expect("covers");
        let service = LibraryService::with_cover_cache(storage, covers);
        let epub_path = temp.path.join("book.epub");
        std::fs::write(&epub_path, build_epub(cover)).expect("写 EPUB");
        let result = service.import(&epub_path);
        assert_eq!(result.status, ImportStatus::Imported, "{name}");
        assert_eq!(summary(&service)[0].cover_cache_key, None, "{name}");
        let entries = std::fs::read_dir(temp.covers()).expect("目录");
        assert_eq!(entries.count(), 0, "{name} 不应留下部分缓存");
    }
}

#[test]
fn oversized_cover_source_is_ignored() {
    let temp = TempDir::new("oversized");
    let storage = StorageService::open(&temp.path.join("reader.sqlite3")).expect("db");
    let covers = CoverCache::new(temp.covers()).expect("covers");
    let service = LibraryService::with_cover_cache(storage, covers);
    let epub_path = temp.path.join("big.epub");
    let oversized = vec![0_u8; 20 * 1024 * 1024 + 1];
    std::fs::write(
        &epub_path,
        build_epub(Some(("image/png", oversized.as_slice()))),
    )
    .expect("写 EPUB");
    let result = service.import(&epub_path);
    assert_eq!(result.status, ImportStatus::Imported);
    assert_eq!(summary(&service)[0].cover_cache_key, None);
}

#[test]
fn unwritable_cache_directory_does_not_block_import() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new("readonly");
    let covers = temp.covers();
    std::fs::create_dir_all(&covers).expect("covers");
    let mut perms = std::fs::metadata(&covers).expect("meta").permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&covers, perms).expect("chmod readonly");

    let storage = StorageService::open(&temp.path.join("reader.sqlite3")).expect("db");
    let cache = CoverCache::new(&covers).expect("cache");
    let service = LibraryService::with_cover_cache(storage, cache);
    let epub_path = temp.path.join("book.epub");
    std::fs::write(&epub_path, build_epub(Some(("image/png", &png_1x1())))).expect("epub");

    let result = service.import(&epub_path);
    assert_eq!(result.status, ImportStatus::Imported);
    assert_eq!(summary(&service)[0].cover_cache_key, None);
    let entries = std::fs::read_dir(&covers).expect("dir").count();
    assert_eq!(entries, 0, "写盘失败不应留下缓存或临时文件");

    let mut perms = std::fs::metadata(&covers).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&covers, perms).expect("restore permissions");
}

#[test]
fn import_does_not_modify_original_epub_bytes() {
    let temp = TempDir::new("bytes");
    let (service, epub_path) = service(&temp.path);
    let before = std::fs::read(&epub_path).expect("read before");
    let result = service.import(Path::new(&epub_path));
    assert_eq!(result.status, ImportStatus::Imported);
    let after = std::fs::read(&epub_path).expect("read after");
    assert_eq!(before, after, "导入不得修改原始 EPUB");
}

#[test]
fn duplicate_import_reuses_cache_and_missing_file_stays_null() {
    let temp = TempDir::new("duplicate");
    let (service, epub_path) = service(&temp.path);
    let first = service.import(Path::new(&epub_path));
    assert_eq!(first.status, ImportStatus::Imported);
    let key = summary(&service)[0].cover_cache_key.clone().expect("key");
    let cached = temp.covers().join(format!("{key}.jpg"));
    std::fs::remove_file(&cached).expect("删除缓存");

    let second = service.import(Path::new(&epub_path));
    assert_eq!(second.status, ImportStatus::Duplicate);
    assert_eq!(
        summary(&service)[0].cover_cache_key.as_deref(),
        Some(key.as_str())
    );
    assert!(!cached.exists(), "重复导入不应重建缓存");
}
