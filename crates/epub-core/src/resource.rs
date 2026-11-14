//! 受大小限制的单资源读取。
//!
//! 只定位并读取目标 ZIP 条目，不遍历全部条目、不缓存整本解压内容，也不复制原始
//! EPUB。错误类别不包含 ZIP 库、`std::io` 路径或第三方错误文本。

use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

use zip::ZipArchive;

use crate::path::ResourcePath;

/// 单资源字节上限：200 MB。
pub const MAX_RESOURCE_BYTES: u64 = 200 * 1024 * 1024;

/// 单资源读取失败类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceReadError {
    /// 归档中不存在该条目。
    NotFound,
    /// 声明大小或实际解压超过上限。
    LimitExceeded,
    /// 归档损坏、CRC 错误或读取失败。
    Corrupt,
}

/// 从文件路径读取单个资源。
///
/// `max_bytes` 是单资源字节上限；调用方应传入 [`MAX_RESOURCE_BYTES`]。
pub fn read_resource(
    path: &Path,
    resource: &ResourcePath,
    max_bytes: u64,
) -> Result<Vec<u8>, ResourceReadError> {
    let file = File::open(path).map_err(|_| ResourceReadError::Corrupt)?;
    read_resource_from_reader(file, resource, max_bytes)
}

/// 从 `Read + Seek` 读取单个资源；仅读取目标条目所需字节。
pub fn read_resource_from_reader<R: Read + Seek>(
    reader: R,
    resource: &ResourcePath,
    max_bytes: u64,
) -> Result<Vec<u8>, ResourceReadError> {
    let mut archive = ZipArchive::new(reader).map_err(|_| ResourceReadError::Corrupt)?;
    let mut entry = archive
        .by_name(resource.as_str())
        .map_err(|error| match error {
            zip::result::ZipError::FileNotFound => ResourceReadError::NotFound,
            _ => ResourceReadError::Corrupt,
        })?;

    // 分配正文缓冲前检查 ZIP 声明大小。
    if entry.size() > max_bytes {
        return Err(ResourceReadError::LimitExceeded);
    }

    // 解压读取也使用硬上限，防止伪造声明大小导致超大分配。
    let mut buffer = Vec::new();
    let limit = max_bytes.saturating_add(1);
    let mut limited = entry.by_ref().take(limit);
    let read = limited
        .read_to_end(&mut buffer)
        .map_err(|_| ResourceReadError::Corrupt)?;
    if read as u64 > max_bytes {
        return Err(ResourceReadError::LimitExceeded);
    }
    Ok(buffer)
}
