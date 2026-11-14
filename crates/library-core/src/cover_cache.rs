//! 导入时封面缓存：解码 EPUB 内嵌封面，缩放并编码为 JPEG。
//!
//! 缓存失败不得阻断书籍导入；失败只记录不含路径的稳定阶段标记。

use std::fs::{self, File};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use epub_core::{ParsedBook, read_resource};
use reader_model::BookFingerprint;

/// 封面源资源读取上限（SDS：20 MiB）。
pub const MAX_COVER_SOURCE_BYTES: u64 = 20 * 1024 * 1024;
/// 封面最长边。
pub const MAX_COVER_EDGE: u32 = 512;
/// JPEG 编码质量。
pub const JPEG_QUALITY: u8 = 85;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 封面缓存目录；`covers/<指纹>.jpg`。
#[derive(Debug, Clone)]
pub struct CoverCache {
    directory: PathBuf,
}

impl CoverCache {
    /// 创建缓存；目录不可创建时返回 `None`（调用方回退为无缓存）。
    #[must_use]
    pub fn new(directory: impl Into<PathBuf>) -> Option<Self> {
        let directory = directory.into();
        fs::create_dir_all(&directory).ok()?;
        Some(Self { directory })
    }

    /// 缓存目录。
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// 生成封面 JPEG；成功返回缓存键（内容指纹），失败返回 `None`。
    ///
    /// 只读取 EPUB 内声明的封面资源，不修改原始文件。
    #[must_use]
    pub fn generate(
        &self,
        epub_path: &Path,
        fingerprint: &BookFingerprint,
        book: &ParsedBook,
    ) -> Option<String> {
        let cover = book.cover.as_ref()?;
        let bytes = match read_resource(epub_path, &cover.href, MAX_COVER_SOURCE_BYTES) {
            Ok(bytes) => bytes,
            Err(_) => {
                log_stage("cover_read");
                return None;
            }
        };
        let decoded = match decode(&bytes) {
            Some(image) => image,
            None => {
                log_stage("cover_decode");
                return None;
            }
        };
        // 不放大较小图片；只有超出最长边时才等比缩放。
        let scaled = if decoded.width() > MAX_COVER_EDGE || decoded.height() > MAX_COVER_EDGE {
            decoded.thumbnail(MAX_COVER_EDGE, MAX_COVER_EDGE)
        } else {
            decoded
        };
        let rgb = scaled.to_rgb8();

        let key = fingerprint.to_string();
        let target = self.directory.join(format!("{key}.jpg"));
        let temp = self.directory.join(format!(
            ".{key}.{}.{}.tmp",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        if write_jpeg(&temp, &rgb).is_err() {
            let _ = fs::remove_file(&temp);
            log_stage("cover_write");
            return None;
        }
        if fs::rename(&temp, &target).is_err() {
            let _ = fs::remove_file(&temp);
            log_stage("cover_rename");
            return None;
        }
        Some(key)
    }
}

fn decode(bytes: &[u8]) -> Option<image::DynamicImage> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.decode().ok()
}

fn write_jpeg(path: &Path, image: &image::RgbImage) -> Result<(), ()> {
    let file = File::create(path).map_err(|_| ())?;
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, JPEG_QUALITY);
    encoder.encode_image(image).map_err(|_| ())
}

fn log_stage(stage: &str) {
    eprintln!("cover cache unavailable: stage={stage}");
}
