//! 导入、判重与软移除恢复用例。

use std::path::{Path, PathBuf};

use epub_core::{EpubErrorCode, fingerprint_file, parse_epub};
use reader_model::{BookFingerprint, BookSummary, ListBooksQuery};
use storage_core::{NewBookRecord, ReactivateOutcome, StorageService};

use crate::cover_cache::CoverCache;
use crate::error::{BookError, StorageErrorKind};
use crate::keys;

/// 单本书导入状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStatus {
    Imported,
    Reactivated,
    Duplicate,
    Failed,
}

/// 单本书导入结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportItemResult {
    pub status: ImportStatus,
    pub book_id: Option<String>,
    pub error: Option<BookError>,
}

/// 书库用例服务：串联 EPUB Core 与 Storage Service。
pub struct LibraryService {
    storage: StorageService,
    covers: Option<CoverCache>,
}

impl LibraryService {
    #[must_use]
    pub fn new(storage: StorageService) -> Self {
        Self {
            storage,
            covers: None,
        }
    }

    /// 带封面缓存的构造；缓存失败不影响导入。
    #[must_use]
    pub fn with_cover_cache(storage: StorageService, covers: CoverCache) -> Self {
        Self {
            storage,
            covers: Some(covers),
        }
    }

    /// 导入单本书；任何失败都体现在返回的 `ImportItemResult` 中。
    pub fn import(&self, path: &Path) -> ImportItemResult {
        match fingerprint_file(path) {
            Ok(fingerprint) => self.import_with_fingerprint(path, fingerprint),
            Err(error) => failed(map_epub_error(error)),
        }
    }

    /// 按输入顺序逐本导入；单本失败不中止其他文件。
    #[must_use]
    pub fn import_batch(&self, paths: &[PathBuf]) -> Vec<ImportItemResult> {
        paths.iter().map(|path| self.import(path)).collect()
    }

    /// 打开书籍并返回创建会话前所需的完整数据。
    pub fn open_book(&self, book_id: &str) -> Result<crate::open::OpenBook, BookError> {
        crate::open::open_book(&self.storage, book_id)
    }

    /// 记录最近打开时间；会话创建成功后调用。
    pub fn mark_opened(&self, book_id: &str) -> Result<(), BookError> {
        crate::open::mark_opened(&self.storage, book_id)
    }

    /// 列出书签（created_at DESC, id ASC）。
    pub fn list_bookmarks(&self, book_id: &str) -> Result<Vec<reader_model::Bookmark>, BookError> {
        crate::bookmarks::list_bookmarks(&self.storage, book_id)
    }

    /// 创建书签；excerpt 缺省从 Locator quote 截取。
    pub fn create_bookmark(
        &self,
        book_id: &str,
        locator: &reader_model::ReaderLocator,
        excerpt: Option<&str>,
    ) -> Result<reader_model::Bookmark, BookError> {
        crate::bookmarks::create_bookmark(&self.storage, book_id, locator, excerpt)
    }

    /// 删除属于指定书籍的书签。
    pub fn delete_bookmark(&self, book_id: &str, bookmark_id: &str) -> Result<(), BookError> {
        crate::bookmarks::delete_bookmark(&self.storage, book_id, bookmark_id)
    }

    /// 保存阅读位置并返回 UTC RFC3339 时间戳。
    pub fn save_reading_state(
        &self,
        book_id: &str,
        locator_json: &str,
        global_progression: f64,
    ) -> Result<String, BookError> {
        if !global_progression.is_finite() {
            return Err(BookError::InvalidInput);
        }
        let progression = global_progression.clamp(0.0, 1.0);
        let timestamp = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|_| BookError::Internal)?;
        self.storage
            .save_reading_state(book_id, locator_json, 1, progression, &timestamp)
            .map_err(|error| BookError::Storage(error.into()))?;
        Ok(timestamp)
    }

    /// 读取单书 effective settings。
    pub fn get_effective_settings(
        &self,
        book_id: &str,
    ) -> Result<reader_model::ReaderSettings, BookError> {
        crate::settings::get_effective(&self.storage, book_id)
    }

    /// 更新全局默认设置。
    pub fn update_app_settings(
        &self,
        patch: crate::settings::AppSettingsPatch,
    ) -> Result<reader_model::ReaderSettings, BookError> {
        crate::settings::update_app(&self.storage, patch)
    }

    /// 更新单书覆盖设置。
    pub fn update_book_settings(
        &self,
        book_id: &str,
        patch: crate::settings::BookSettingsPatch,
    ) -> Result<reader_model::ReaderSettings, BookError> {
        crate::settings::update_book(&self.storage, book_id, patch)
    }

    /// 清除单书覆盖。
    pub fn reset_book_settings(
        &self,
        book_id: &str,
    ) -> Result<reader_model::ReaderSettings, BookError> {
        crate::settings::reset_book(&self.storage, book_id)
    }

    /// 查询书库列表；搜索与排序由 storage 层参数化完成。
    pub fn list_books(&self, query: ListBooksQuery) -> Result<Vec<BookSummary>, BookError> {
        self.storage.list_books(query).map_err(map_storage_error)
    }

    /// 软移除书籍；保留个人阅读数据。
    pub fn soft_remove(&self, book_id: &str) -> Result<(), BookError> {
        self.storage
            .soft_remove_book(book_id)
            .map_err(map_storage_error)
    }

    fn import_with_fingerprint(&self, path: &Path, initial: BookFingerprint) -> ImportItemResult {
        let fingerprint = initial.to_string();
        match self.storage.find_by_fingerprint(&fingerprint) {
            Ok(Some(existing)) if existing.lifecycle == "removed" => {
                self.reactivate(path, &existing.id, initial)
            }
            Ok(Some(existing)) => success(ImportStatus::Duplicate, existing.id),
            Ok(None) => self.import_new(path, initial),
            Err(error) => failed(map_storage_error(error)),
        }
    }

    fn reactivate(&self, path: &Path, book_id: &str, initial: BookFingerprint) -> ImportItemResult {
        let after = match fingerprint_file(path) {
            Ok(fingerprint) => fingerprint,
            Err(error) => return failed(map_epub_error(error)),
        };
        if after != initial {
            return failed(BookError::SourceChanged);
        }

        let source_path = match absolute_utf8_path(path) {
            Ok(source_path) => source_path,
            Err(error) => return failed(error),
        };
        match self
            .storage
            .reactivate_book(book_id, &source_path, &keys::now_rfc3339())
        {
            Ok(ReactivateOutcome::Reactivated) => {
                success(ImportStatus::Reactivated, book_id.to_string())
            }
            Ok(ReactivateOutcome::AlreadyActive) => {
                success(ImportStatus::Duplicate, book_id.to_string())
            }
            Ok(ReactivateOutcome::NotFound) => failed(BookError::Internal),
            Err(error) => failed(map_storage_error(error)),
        }
    }

    fn import_new(&self, path: &Path, initial: BookFingerprint) -> ImportItemResult {
        let book = match parse_epub(path) {
            Ok(book) => book,
            Err(error) => return failed(map_epub_error(error)),
        };

        let after = match fingerprint_file(path) {
            Ok(fingerprint) => fingerprint,
            Err(error) => return failed(map_epub_error(error)),
        };
        if after != initial {
            return failed(BookError::SourceChanged);
        }

        let source_path = match absolute_utf8_path(path) {
            Ok(source_path) => source_path,
            Err(error) => return failed(error),
        };
        let title = keys::display_title(book.metadata.title.as_deref(), path);
        let author = keys::display_author(&book.metadata.authors);
        let (title_sort_key, author_sort_key) = match keys::sort_keys(&title, &author) {
            Ok(keys) => keys,
            Err(error) => return failed(error),
        };
        let manifest_json = match serde_json::to_string(&book) {
            Ok(json) => json,
            Err(_) => return failed(BookError::Internal),
        };

        let fingerprint = initial.to_string();
        let book_id = keys::new_book_id();
        let cover_cache_key = self
            .covers
            .as_ref()
            .and_then(|covers| covers.generate(path, &initial, &book));
        let record = NewBookRecord {
            id: book_id.clone(),
            fingerprint: fingerprint.clone(),
            cover_cache_key,
            source_path,
            title: title.clone(),
            author: author.clone(),
            title_search: keys::search_key(&title),
            author_search: keys::search_key(&author),
            title_sort_key,
            author_sort_key,
            manifest_json,
            manifest_version: book.schema_version,
            imported_at: keys::now_rfc3339(),
        };

        match self.storage.insert_new_book(record) {
            Ok(()) => success(ImportStatus::Imported, book_id),
            Err(storage_core::StorageError::Conflict) => {
                match self.storage.find_by_fingerprint(&fingerprint) {
                    Ok(Some(existing)) => success(ImportStatus::Duplicate, existing.id),
                    Ok(None) => failed(BookError::Internal),
                    Err(error) => failed(map_storage_error(error)),
                }
            }
            Err(error) => failed(map_storage_error(error)),
        }
    }
}

fn success(status: ImportStatus, book_id: String) -> ImportItemResult {
    ImportItemResult {
        status,
        book_id: Some(book_id),
        error: None,
    }
}

fn failed(error: BookError) -> ImportItemResult {
    ImportItemResult {
        status: ImportStatus::Failed,
        book_id: None,
        error: Some(error),
    }
}

fn absolute_utf8_path(path: &Path) -> Result<String, BookError> {
    let absolute = path.canonicalize().map_err(|_| BookError::FileUnreadable)?;
    absolute
        .to_str()
        .map(str::to_string)
        .ok_or(BookError::FileUnreadable)
}

fn map_epub_error(error: epub_core::EpubError) -> BookError {
    match error.code() {
        EpubErrorCode::FileNotFound => BookError::FileNotFound,
        EpubErrorCode::IoError => BookError::FileUnreadable,
        code => BookError::Epub(code),
    }
}

fn map_storage_error(error: storage_core::StorageError) -> BookError {
    BookError::Storage(StorageErrorKind::from(error))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use storage_core::StorageService;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "qurner-library-service-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("应能创建测试目录");
        directory
    }

    fn write_epub(directory: &Path, name: &str, title: &str, body: &str) -> PathBuf {
        let path = directory.join(name);
        std::fs::write(&path, build_epub(title, body)).expect("写 EPUB");
        path
    }

    fn build_epub(title: &str, body: &str) -> Vec<u8> {
        let container = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;
        let opf = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>{title}</dc:title>
    <dc:creator>作者</dc:creator>
    <dc:identifier id="id">urn:uuid:service-test</dc:identifier>
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
        let nav = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">第一章</a></li></ol></nav></body></html>"#;
        let chapter = format!(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title></title></head><body><p>{body}</p></body></html>"
        );

        build_zip(&[
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", container.as_bytes()),
            ("OEBPS/content.opf", opf.as_bytes()),
            ("OEBPS/nav.xhtml", nav.as_bytes()),
            ("OEBPS/chapter1.xhtml", chapter.as_bytes()),
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
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            }
        }
        !crc
    }

    #[test]
    fn source_changed_is_detected_and_not_persisted() {
        let directory = temp_dir("source-changed");
        let path = write_epub(&directory, "book.epub", "原标题", "正文");

        let storage = StorageService::open(&directory.join("reader.sqlite3")).expect("open");
        let service = LibraryService::new(storage.clone());

        let initial = fingerprint_file(&path).expect("initial fingerprint");
        write_epub(&directory, "book.epub", "原标题", "正文已改变");

        let result = service.import_with_fingerprint(&path, initial);
        assert_eq!(result.status, ImportStatus::Failed);
        assert_eq!(result.error, Some(BookError::SourceChanged));
        assert!(result.book_id.is_none());

        assert!(
            storage
                .find_by_fingerprint(&initial.to_string())
                .expect("find")
                .is_none(),
            "SourceChanged 不应写入记录"
        );

        storage.shutdown().expect("shutdown");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn removed_source_changed_keeps_record_removed() {
        let directory = temp_dir("removed-changed");
        let path = write_epub(&directory, "book.epub", "原标题", "正文");

        let storage = StorageService::open(&directory.join("reader.sqlite3")).expect("open");
        let service = LibraryService::new(storage.clone());

        let imported = service.import(&path);
        assert_eq!(imported.status, ImportStatus::Imported);
        let book_id = imported.book_id.expect("book id");
        service.soft_remove(&book_id).expect("soft remove");

        let initial = fingerprint_file(&path).expect("initial fingerprint");
        write_epub(&directory, "book.epub", "原标题", "正文已改变");

        let result = service.import_with_fingerprint(&path, initial);
        assert_eq!(result.status, ImportStatus::Failed);
        assert_eq!(result.error, Some(BookError::SourceChanged));

        let found = storage
            .find_by_fingerprint(&initial.to_string())
            .expect("find")
            .expect("exists");
        assert_eq!(found.lifecycle, "removed", "恢复失败时记录应保持 removed");

        storage.shutdown().expect("shutdown");
        let _ = std::fs::remove_dir_all(&directory);
    }
}
