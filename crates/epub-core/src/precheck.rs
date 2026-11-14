//! 流式 ZIP 安全预检。
//!
//! 只通过 seek 读取 EOCD 和 central directory 头部，不读取归档内容，
//! 以便在 `rbook` 解析前拒绝重复条目、不安全路径、超量大小和损坏目录。

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use crate::{EpubError, EpubErrorCode, ResourcePath};

/// 允许的最大 ZIP 条目数。
pub(crate) const MAX_ENTRIES: usize = 100_000;
/// 单个条目解压后的最大字节数。
pub(crate) const MAX_ENTRY_UNCOMPRESSED_BYTES: u64 = 200 * 1024 * 1024;
/// 全部条目解压后的最大字节数。
pub(crate) const MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 1024 * 1024 * 1024;
/// `META-INF/encryption.xml` 允许的最大字节数。
pub(crate) const MAX_ENCRYPTION_XML_BYTES: u64 = 1024 * 1024;

const EOCD_MIN_LEN: u64 = 22;
const MAX_COMMENT_LEN: u64 = 65_535;
const EOCD_SIGNATURE: u32 = 0x0605_4b50;
const CENTRAL_HEADER_SIGNATURE: u32 = 0x0201_4b50;
const CENTRAL_HEADER_LEN: usize = 46;

/// 预检结果。不公开，避免泄漏 ZIP 细节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrecheckReport {
    pub(crate) entry_count: usize,
    pub(crate) total_uncompressed_size: u64,
    pub(crate) has_container: bool,
    pub(crate) has_encryption: bool,
    pub(crate) bytes_read: u64,
}

/// 统计实际读取字节数的 `Read` 包装器；`Seek` 不计入。
pub(crate) struct CountingReader<R> {
    inner: R,
    bytes_read: u64,
}

impl<R> CountingReader<R> {
    pub(crate) fn new(inner: R) -> Self {
        Self {
            inner,
            bytes_read: 0,
        }
    }

    pub(crate) fn bytes_read(&self) -> u64 {
        self.bytes_read
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.bytes_read = self.bytes_read.saturating_add(read as u64);
        Ok(read)
    }
}

impl<R: Seek> Seek for CountingReader<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.inner.seek(position)
    }
}

/// 打开文件并执行流式预检。
pub(crate) fn precheck_epub(path: &Path) -> Result<PrecheckReport, EpubError> {
    let file = File::open(path).map_err(|_| EpubError::io())?;
    let file_len = file.metadata().map_err(|_| EpubError::io())?.len();
    let mut reader = CountingReader::new(file);
    let mut report = precheck_reader(&mut reader, file_len)?;
    report.bytes_read = reader.bytes_read();
    Ok(report)
}

/// 在任意 `Read + Seek` 上执行预检，便于测试统计读取字节数。
pub(crate) fn precheck_reader<R: Read + Seek>(
    reader: &mut R,
    file_len: u64,
) -> Result<PrecheckReport, EpubError> {
    if file_len < EOCD_MIN_LEN {
        return Err(invalid_zip());
    }

    let tail_len = file_len.min(EOCD_MIN_LEN + MAX_COMMENT_LEN);
    reader
        .seek(SeekFrom::Start(file_len - tail_len))
        .map_err(|_| EpubError::io())?;
    let mut tail = vec![0_u8; usize::try_from(tail_len).map_err(|_| invalid_zip())?];
    reader.read_exact(&mut tail).map_err(|_| EpubError::io())?;

    let tail_start = file_len - tail_len;
    let eocd_index = (0..=tail.len().saturating_sub(EOCD_MIN_LEN as usize))
        .rev()
        .find(|&index| {
            if read_u32(&tail, index) != Some(EOCD_SIGNATURE) {
                return false;
            }
            let Some(comment_len) = read_u16(&tail, index + 20) else {
                return false;
            };
            tail_start + index as u64 + EOCD_MIN_LEN + u64::from(comment_len) == file_len
        })
        .ok_or_else(invalid_zip)?;
    let eocd_abs = tail_start + eocd_index as u64;

    let disk_number = read_u16(&tail, eocd_index + 4).ok_or_else(invalid_zip)?;
    let central_disk_number = read_u16(&tail, eocd_index + 6).ok_or_else(invalid_zip)?;
    let entries_on_disk = read_u16(&tail, eocd_index + 8).ok_or_else(invalid_zip)?;
    let total_entries = read_u16(&tail, eocd_index + 10).ok_or_else(invalid_zip)?;
    let central_size = u64::from(read_u32(&tail, eocd_index + 12).ok_or_else(invalid_zip)?);
    let central_offset = u64::from(read_u32(&tail, eocd_index + 16).ok_or_else(invalid_zip)?);

    if disk_number == u16::MAX
        || central_disk_number == u16::MAX
        || entries_on_disk == u16::MAX
        || total_entries == u16::MAX
        || central_size == u64::from(u32::MAX)
        || central_offset == u64::from(u32::MAX)
    {
        return Err(EpubError::new(EpubErrorCode::LimitExceeded));
    }
    if disk_number != 0 || central_disk_number != 0 || entries_on_disk != total_entries {
        return Err(invalid_zip());
    }

    let entry_count = usize::from(total_entries);
    if entry_count > MAX_ENTRIES {
        return Err(EpubError::new(EpubErrorCode::LimitExceeded));
    }
    let central_end = central_offset
        .checked_add(central_size)
        .ok_or_else(invalid_zip)?;
    if central_end != eocd_abs {
        return Err(invalid_zip());
    }

    reader
        .seek(SeekFrom::Start(central_offset))
        .map_err(|_| EpubError::io())?;

    let mut names = HashSet::with_capacity(entry_count);
    let mut total_uncompressed_size = 0_u64;
    let mut has_container = false;
    let mut has_encryption = false;
    let mut first_entry: Option<(String, u16, u64, u64)> = None;
    let mut position = central_offset;
    let mut header = [0_u8; CENTRAL_HEADER_LEN];

    for _ in 0..entry_count {
        if position + CENTRAL_HEADER_LEN as u64 > central_end {
            return Err(invalid_zip());
        }
        reader
            .read_exact(&mut header)
            .map_err(|_| EpubError::io())?;
        if read_u32(&header, 0) != Some(CENTRAL_HEADER_SIGNATURE) {
            return Err(invalid_zip());
        }

        let method = read_u16(&header, 10).ok_or_else(invalid_zip)?;
        let uncompressed_size = u64::from(read_u32(&header, 24).ok_or_else(invalid_zip)?);
        let local_offset = u64::from(read_u32(&header, 42).ok_or_else(invalid_zip)?);
        let name_len = usize::from(read_u16(&header, 28).ok_or_else(invalid_zip)?);
        let extra_len = u64::from(read_u16(&header, 30).ok_or_else(invalid_zip)?);
        let comment_len = u64::from(read_u16(&header, 32).ok_or_else(invalid_zip)?);
        let next = position
            .checked_add(CENTRAL_HEADER_LEN as u64)
            .and_then(|value| value.checked_add(name_len as u64))
            .and_then(|value| value.checked_add(extra_len))
            .and_then(|value| value.checked_add(comment_len))
            .ok_or_else(invalid_zip)?;
        if next > central_end {
            return Err(invalid_zip());
        }

        let mut name_bytes = vec![0_u8; name_len];
        reader
            .read_exact(&mut name_bytes)
            .map_err(|_| EpubError::io())?;
        let skip = extra_len.checked_add(comment_len).ok_or_else(invalid_zip)?;
        reader
            .seek(SeekFrom::Current(
                i64::try_from(skip).map_err(|_| invalid_zip())?,
            ))
            .map_err(|_| EpubError::io())?;

        let name = String::from_utf8_lossy(&name_bytes).into_owned();
        if ResourcePath::new(&name).is_err() {
            return Err(EpubError::new(EpubErrorCode::UnsafeResourcePath));
        }
        if !names.insert(name.clone()) {
            return Err(invalid_zip());
        }
        if uncompressed_size > MAX_ENTRY_UNCOMPRESSED_BYTES {
            return Err(EpubError::new(EpubErrorCode::LimitExceeded));
        }
        total_uncompressed_size = total_uncompressed_size.saturating_add(uncompressed_size);
        if total_uncompressed_size > MAX_TOTAL_UNCOMPRESSED_BYTES {
            return Err(EpubError::new(EpubErrorCode::LimitExceeded));
        }

        has_container |= name == "META-INF/container.xml";
        has_encryption |= name == "META-INF/encryption.xml";
        if first_entry.is_none() {
            first_entry = Some((name.clone(), method, local_offset, uncompressed_size));
        }
        position = next;
    }

    let (first_name, first_method, first_local_offset, first_size) =
        first_entry.ok_or_else(invalid_zip)?;
    validate_mimetype(
        reader,
        &first_name,
        first_method,
        first_local_offset,
        first_size,
    )?;

    Ok(PrecheckReport {
        entry_count,
        total_uncompressed_size,
        has_container,
        has_encryption,
        bytes_read: 0,
    })
}

fn validate_mimetype<R: Read + Seek>(
    reader: &mut R,
    name: &str,
    method: u16,
    local_offset: u64,
    uncompressed_size: u64,
) -> Result<(), EpubError> {
    if name != "mimetype" || method != 0 || local_offset != 0 || uncompressed_size != 20 {
        return Err(invalid_zip());
    }

    reader
        .seek(SeekFrom::Start(0))
        .map_err(|_| EpubError::io())?;
    let mut header = [0_u8; 30];
    reader
        .read_exact(&mut header)
        .map_err(|_| EpubError::io())?;

    if read_u32(&header, 0) != Some(0x0403_4b50)
        || read_u16(&header, 8) != Some(0)
        || read_u32(&header, 18) != Some(20)
        || read_u32(&header, 22) != Some(20)
    {
        return Err(invalid_zip());
    }

    let name_len = usize::from(read_u16(&header, 26).ok_or_else(invalid_zip)?);
    let extra_len = u64::from(read_u16(&header, 28).ok_or_else(invalid_zip)?);
    if name_len != 8 {
        return Err(invalid_zip());
    }

    let mut name_bytes = [0_u8; 8];
    reader
        .read_exact(&mut name_bytes)
        .map_err(|_| EpubError::io())?;
    if &name_bytes != b"mimetype" {
        return Err(invalid_zip());
    }

    if extra_len > 0 {
        reader
            .seek(SeekFrom::Current(
                i64::try_from(extra_len).map_err(|_| invalid_zip())?,
            ))
            .map_err(|_| EpubError::io())?;
    }

    let mut content = [0_u8; 20];
    reader
        .read_exact(&mut content)
        .map_err(|_| EpubError::io())?;
    if &content != b"application/epub+zip" {
        return Err(invalid_zip());
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn invalid_zip() -> EpubError {
    EpubError::new(EpubErrorCode::InvalidZip)
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write as _;

    use super::*;

    /// 构造只含 central directory 声明的伪造 ZIP；数据区为空。
    ///
    /// 预检只读取 central directory，因此可用于测试大小限制而不写真实大文件。
    struct TestZipEntry {
        name: String,
        data: Vec<u8>,
        method: u16,
        declared_uncompressed: u32,
    }

    fn mimetype_entry() -> TestZipEntry {
        TestZipEntry {
            name: "mimetype".to_string(),
            data: b"application/epub+zip".to_vec(),
            method: 0,
            declared_uncompressed: 20,
        }
    }

    fn build_test_zip(entries: &[TestZipEntry]) -> Vec<u8> {
        let mut output = Vec::new();
        let mut central = Vec::new();
        let mut offset = 0_u32;

        for entry in entries {
            let name = entry.name.as_bytes();
            let local_size = entry.data.len() as u32;

            output.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
            output.extend_from_slice(&20_u16.to_le_bytes());
            output.extend_from_slice(&0x0800_u16.to_le_bytes());
            output.extend_from_slice(&entry.method.to_le_bytes());
            output.extend_from_slice(&0_u16.to_le_bytes());
            output.extend_from_slice(&0x0021_u16.to_le_bytes());
            output.extend_from_slice(&0_u32.to_le_bytes());
            output.extend_from_slice(&local_size.to_le_bytes());
            output.extend_from_slice(&local_size.to_le_bytes());
            output.extend_from_slice(&(name.len() as u16).to_le_bytes());
            output.extend_from_slice(&0_u16.to_le_bytes());
            output.extend_from_slice(name);
            output.extend_from_slice(&entry.data);

            central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&0x0800_u16.to_le_bytes());
            central.extend_from_slice(&entry.method.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0x0021_u16.to_le_bytes());
            central.extend_from_slice(&0_u32.to_le_bytes());
            central.extend_from_slice(&local_size.to_le_bytes());
            central.extend_from_slice(&entry.declared_uncompressed.to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u32.to_le_bytes());
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name);

            offset += 30 + name.len() as u32 + local_size;
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

    fn fake_zip(entries: &[(&str, u32)]) -> Vec<u8> {
        let mut all = vec![mimetype_entry()];
        all.extend(entries.iter().map(|(name, declared_size)| TestZipEntry {
            name: (*name).to_string(),
            data: Vec::new(),
            method: 0,
            declared_uncompressed: *declared_size,
        }));
        build_test_zip(&all)
    }

    fn run_precheck(bytes: &[u8]) -> Result<PrecheckReport, EpubError> {
        let mut cursor = std::io::Cursor::new(bytes);
        let len = bytes.len() as u64;
        precheck_reader(&mut cursor, len)
    }

    #[test]
    fn reads_only_tail_and_central_directory_for_large_archive() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("large.epub");
        {
            let file = File::create(&path).expect("file");
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("mimetype", options).expect("mimetype");
            zip.write_all(b"application/epub+zip").expect("write");
            zip.start_file("OEBPS/big.bin", options).expect("big");
            let big = vec![0_u8; 100 * 1024 * 1024];
            zip.write_all(&big).expect("write big");
            zip.finish().expect("finish");
        }

        let file = File::open(&path).expect("open");
        let file_len = file.metadata().expect("metadata").len();
        let mut reader = CountingReader::new(file);
        let report = precheck_reader(&mut reader, file_len).expect("precheck");

        assert_eq!(report.entry_count, 2);
        assert_eq!(report.total_uncompressed_size, 100 * 1024 * 1024 + 20);
        eprintln!("precheck bytes_read={}", reader.bytes_read());
        assert!(
            reader.bytes_read() < 100_000,
            "预检读取了 {} 字节，不应读取完整文件",
            reader.bytes_read()
        );
    }

    #[test]
    fn rejects_zip64_marker() {
        let mut bytes = fake_zip(&[("mimetype", 20)]);
        let eocd = bytes.len() - 22;
        bytes[eocd + 10] = 0xff;
        bytes[eocd + 11] = 0xff;
        assert_eq!(
            run_precheck(&bytes).expect_err("应拒绝 ZIP64").code(),
            EpubErrorCode::LimitExceeded
        );
    }

    #[test]
    fn rejects_duplicate_names() {
        let bytes = fake_zip(&[("mimetype", 20), ("mimetype", 20)]);
        assert_eq!(
            run_precheck(&bytes).expect_err("应拒绝重复名").code(),
            EpubErrorCode::InvalidZip
        );
    }

    #[test]
    fn rejects_unsafe_names() {
        for name in ["../evil.txt", "/absolute.txt", "C:\\evil.txt", "OEBPS/../x"] {
            let bytes = fake_zip(&[(name, 10)]);
            assert_eq!(
                run_precheck(&bytes).expect_err("应拒绝不安全路径").code(),
                EpubErrorCode::UnsafeResourcePath,
                "路径 {name}"
            );
        }
    }

    #[test]
    fn rejects_oversized_single_entry() {
        let size = u32::try_from(MAX_ENTRY_UNCOMPRESSED_BYTES + 1).expect("fits u32");
        let bytes = fake_zip(&[("big.bin", size)]);
        assert_eq!(
            run_precheck(&bytes).expect_err("应拒绝超大条目").code(),
            EpubErrorCode::LimitExceeded
        );
    }

    #[test]
    fn rejects_oversized_total() {
        let names: Vec<String> = (0..11).map(|index| format!("chunk-{index}.bin")).collect();
        let entries: Vec<(&str, u32)> = names
            .iter()
            .map(|name| (name.as_str(), 100 * 1024 * 1024))
            .collect();
        let bytes = fake_zip(&entries);
        assert_eq!(
            run_precheck(&bytes).expect_err("应拒绝总解压超限").code(),
            EpubErrorCode::LimitExceeded
        );
    }

    fn fake_zip_with_comment(entries: &[(&str, u32)], comment: &[u8]) -> Vec<u8> {
        let bytes = fake_zip(entries);
        let eocd_start = bytes.len() - 22;
        let mut output = bytes[..eocd_start + 20].to_vec();
        output.extend_from_slice(&(comment.len() as u16).to_le_bytes());
        output.extend_from_slice(comment);
        output
    }

    #[test]
    fn ignores_fake_eocd_signature_inside_zip_comment() {
        let mut comment = b"note:".to_vec();
        comment.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
        comment.extend_from_slice(&[0_u8; 18]);
        comment.extend_from_slice(b"tail");

        let bytes = fake_zip_with_comment(&[("other.bin", 5)], &comment);
        let report = run_precheck(&bytes).expect("应忽略 comment 中的伪 EOCD");
        assert_eq!(report.entry_count, 2);
    }

    #[test]
    fn rejects_multi_disk_markers() {
        let base = fake_zip(&[("a", 1), ("b", 1)]);
        let eocd = base.len() - 22;

        let mut first_disk = base.clone();
        first_disk[eocd + 4] = 1;
        assert_eq!(
            run_precheck(&first_disk)
                .expect_err("非零磁盘号应拒绝")
                .code(),
            EpubErrorCode::InvalidZip
        );

        let mut central_disk = base.clone();
        central_disk[eocd + 6] = 2;
        assert_eq!(
            run_precheck(&central_disk)
                .expect_err("central directory 磁盘号应拒绝")
                .code(),
            EpubErrorCode::InvalidZip
        );

        let mut inconsistent = base;
        inconsistent[eocd + 8] = 1;
        assert_eq!(
            run_precheck(&inconsistent)
                .expect_err("跨磁盘条目计数不一致应拒绝")
                .code(),
            EpubErrorCode::InvalidZip
        );
    }

    #[test]
    fn rejects_truncated_or_misaligned_central_directory() {
        let base = fake_zip(&[("a", 1), ("b", 2)]);
        let eocd = base.len() - 22;
        let central_size = u32::from_le_bytes([
            base[eocd + 12],
            base[eocd + 13],
            base[eocd + 14],
            base[eocd + 15],
        ]);
        let central_offset = u32::from_le_bytes([
            base[eocd + 16],
            base[eocd + 17],
            base[eocd + 18],
            base[eocd + 19],
        ]);

        let mut truncated = base.clone();
        truncated[eocd + 12..eocd + 16].copy_from_slice(&(central_size - 4).to_le_bytes());
        assert_eq!(
            run_precheck(&truncated)
                .expect_err("central directory 截断应拒绝")
                .code(),
            EpubErrorCode::InvalidZip
        );

        let mut misaligned = base;
        misaligned[eocd + 16..eocd + 20].copy_from_slice(&(central_offset + 4).to_le_bytes());
        assert_eq!(
            run_precheck(&misaligned)
                .expect_err("central directory 偏移错位应拒绝")
                .code(),
            EpubErrorCode::InvalidZip
        );
    }

    #[test]
    fn accepts_valid_mimetype() {
        let report =
            run_precheck(&fake_zip(&[("OEBPS/chapter1.xhtml", 10)])).expect("合法 mimetype 应通过");
        assert_eq!(report.entry_count, 2);
    }

    #[test]
    fn rejects_missing_or_displaced_mimetype() {
        let chapter = TestZipEntry {
            name: "OEBPS/chapter1.xhtml".to_string(),
            data: b"x".to_vec(),
            method: 0,
            declared_uncompressed: 1,
        };
        let missing = build_test_zip(&[chapter]);
        assert_eq!(
            run_precheck(&missing)
                .expect_err("缺失 mimetype 应拒绝")
                .code(),
            EpubErrorCode::InvalidZip
        );

        let displaced = build_test_zip(&[
            TestZipEntry {
                name: "OEBPS/chapter1.xhtml".to_string(),
                data: b"x".to_vec(),
                method: 0,
                declared_uncompressed: 1,
            },
            mimetype_entry(),
        ]);
        assert_eq!(
            run_precheck(&displaced)
                .expect_err("mimetype 非首条目应拒绝")
                .code(),
            EpubErrorCode::InvalidZip
        );
    }

    #[test]
    fn rejects_wrong_mimetype_content() {
        let mut entry = mimetype_entry();
        entry.data = b"application/zip".to_vec();
        assert_eq!(
            run_precheck(&build_test_zip(&[entry]))
                .expect_err("错误 mimetype 内容应拒绝")
                .code(),
            EpubErrorCode::InvalidZip
        );
    }

    #[test]
    fn rejects_compressed_mimetype() {
        let mut entry = mimetype_entry();
        entry.method = 8;
        assert_eq!(
            run_precheck(&build_test_zip(&[entry]))
                .expect_err("压缩 mimetype 应拒绝")
                .code(),
            EpubErrorCode::InvalidZip
        );
    }
}
