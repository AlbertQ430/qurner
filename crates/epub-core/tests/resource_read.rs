mod support;

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use epub_core::{
    MAX_RESOURCE_BYTES, ResourcePath, ResourceReadError, read_resource, read_resource_from_reader,
};
use support::fixtures;
use support::zip_writer::{ZipEntry, build_zip};
use tempfile::tempdir;

const XHTML: &[u8] = b"<html><body><p>hello</p></body></html>";

struct CountingReader<R> {
    inner: R,
    bytes: Arc<AtomicU64>,
}

impl<R> CountingReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            bytes: Arc::new(AtomicU64::new(0)),
        }
    }

    fn read_bytes(&self) -> u64 {
        self.bytes.load(Ordering::SeqCst)
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.bytes.fetch_add(read as u64, Ordering::SeqCst);
        Ok(read)
    }
}

impl<R: Seek> Seek for CountingReader<R> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(position)
    }
}

fn resource(name: &str) -> ResourcePath {
    ResourcePath::new(name).expect("valid resource path")
}

fn write_zip(directory: &tempfile::TempDir, entries: Vec<ZipEntry>) -> std::path::PathBuf {
    let path = directory.path().join("book.epub");
    fs::write(&path, build_zip(&entries)).expect("write zip");
    path
}

fn central_entry_offset(bytes: &[u8], name: &str) -> usize {
    let eocd = (0..bytes.len() - 4)
        .rev()
        .find(|&index| bytes[index..index + 4] == [0x50, 0x4b, 0x05, 0x06])
        .expect("EOCD");
    let central_offset = u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().unwrap());
    let mut position = central_offset as usize;
    while position + 46 <= bytes.len() && bytes[position..position + 4] == [0x50, 0x4b, 0x01, 0x02]
    {
        let name_len =
            u16::from_le_bytes(bytes[position + 28..position + 30].try_into().unwrap()) as usize;
        let extra_len =
            u16::from_le_bytes(bytes[position + 30..position + 32].try_into().unwrap()) as usize;
        let comment_len =
            u16::from_le_bytes(bytes[position + 32..position + 34].try_into().unwrap()) as usize;
        if &bytes[position + 46..position + 46 + name_len] == name.as_bytes() {
            return position;
        }
        position += 46 + name_len + extra_len + comment_len;
    }
    panic!("central entry not found: {name}");
}

#[test]
fn reads_single_entry_only() {
    let directory = tempdir().expect("temp");
    let path = write_zip(
        &directory,
        vec![
            ZipEntry::new("mimetype", b"application/epub+zip"),
            ZipEntry::new("OEBPS/ch.xhtml", XHTML),
            ZipEntry::new("OEBPS/big.bin", vec![7_u8; 64 * 1024]),
        ],
    );

    let bytes = read_resource(&path, &resource("OEBPS/ch.xhtml"), MAX_RESOURCE_BYTES)
        .expect("read resource");
    assert_eq!(bytes, XHTML);
}

#[test]
fn rejects_declared_size_over_limit() {
    let directory = tempdir().expect("temp");
    let path = write_zip(
        &directory,
        vec![ZipEntry::new("OEBPS/big.bin", vec![1_u8; 64 * 1024])],
    );

    assert_eq!(
        read_resource(&path, &resource("OEBPS/big.bin"), 1024),
        Err(ResourceReadError::LimitExceeded)
    );
}

#[test]
fn rejects_forged_large_declared_size_before_reading_data() {
    let directory = tempdir().expect("temp");
    let mut bytes = build_zip(&[
        ZipEntry::new("mimetype", b"application/epub+zip"),
        ZipEntry::new("OEBPS/ch.xhtml", XHTML),
        ZipEntry::new("OEBPS/big.bin", vec![2_u8; 256 * 1024]),
    ]);
    let central = central_entry_offset(&bytes, "OEBPS/big.bin");
    bytes[central + 24..central + 28].copy_from_slice(&0x7fff_ffff_u32.to_le_bytes());

    let path = directory.path().join("forged.epub");
    fs::write(&path, bytes).expect("write forged zip");
    let file = File::open(&path).expect("open");
    let mut reader = CountingReader::new(file);
    let error = read_resource_from_reader(&mut reader, &resource("OEBPS/big.bin"), 1024)
        .expect_err("伪造声明大小应被拒绝");
    assert_eq!(error, ResourceReadError::LimitExceeded);
    assert!(
        reader.read_bytes() < 64 * 1024,
        "声明超限应在读取数据区前拒绝，实际读取 {} 字节",
        reader.read_bytes()
    );
}

#[test]
fn rejects_crc_corruption() {
    let directory = tempdir().expect("temp");
    let mut bytes = build_zip(&[ZipEntry::new("OEBPS/ch.xhtml", XHTML)]);
    let central = central_entry_offset(&bytes, "OEBPS/ch.xhtml");
    let local_offset =
        u32::from_le_bytes(bytes[central + 42..central + 46].try_into().unwrap()) as usize;
    let name_len = u16::from_le_bytes(
        bytes[local_offset + 26..local_offset + 28]
            .try_into()
            .unwrap(),
    ) as usize;
    let extra_len = u16::from_le_bytes(
        bytes[local_offset + 28..local_offset + 30]
            .try_into()
            .unwrap(),
    ) as usize;
    let data_offset = local_offset + 30 + name_len + extra_len;
    bytes[data_offset] ^= 0xff;

    let path = directory.path().join("crc.epub");
    fs::write(&path, bytes).expect("write corrupted zip");
    assert_eq!(
        read_resource(&path, &resource("OEBPS/ch.xhtml"), MAX_RESOURCE_BYTES),
        Err(ResourceReadError::Corrupt)
    );
}

#[test]
fn missing_entry_is_not_found() {
    let directory = tempdir().expect("temp");
    let path = write_zip(&directory, vec![ZipEntry::new("OEBPS/ch.xhtml", XHTML)]);

    assert_eq!(
        read_resource(&path, &resource("OEBPS/missing.xhtml"), MAX_RESOURCE_BYTES),
        Err(ResourceReadError::NotFound)
    );
}

#[test]
fn large_epub_reads_only_requested_small_resource() {
    let fixture = fixtures::large_epub();
    let file = File::open(fixture.path()).expect("open large epub");
    let mut reader = CountingReader::new(file);

    let bytes = read_resource_from_reader(
        &mut reader,
        &resource("OEBPS/chapter1.xhtml"),
        MAX_RESOURCE_BYTES,
    )
    .expect("read small chapter");
    assert!(bytes.len() < 1024, "章节应很小");

    let read = reader.read_bytes();
    eprintln!("M3D_LARGE_READ bytes={read} chapter_len={}", bytes.len());
    assert!(
        read < 1024 * 1024,
        "只应读取目标条目与 ZIP 元数据，实际读取 {read} 字节"
    );

    drop(fixture);
}
