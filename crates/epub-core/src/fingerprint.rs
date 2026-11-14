//! 原始 EPUB 文件的流式 SHA-256 内容指纹。
//!
//! 只读打开文件，使用固定 1 MiB 缓冲区流式计算，不一次性读取或映射完整文件。

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use reader_model::BookFingerprint;
use sha2::{Digest, Sha256};

use crate::{EpubError, EpubErrorCode};

/// 单次读取的固定缓冲区大小（1 MiB）。
pub(crate) const BUFFER_SIZE: usize = 1024 * 1024;

/// 计算原始文件字节的 SHA-256 内容指纹。
///
/// 文件不存在返回 `FileNotFound`，其余打开或读取错误返回 `IoError`；
/// 错误不包含绝对路径或 OS 错误文本。
pub fn fingerprint_file(path: &Path) -> Result<BookFingerprint, EpubError> {
    let mut file = File::open(path).map_err(|error| map_open_error(&error))?;
    fingerprint_reader(&mut file).map_err(|_| EpubError::new(EpubErrorCode::IoError))
}

/// 从任意 `Read` 流式计算指纹，供测试统计读取请求。
pub(crate) fn fingerprint_reader<R: Read>(reader: &mut R) -> io::Result<BookFingerprint> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; BUFFER_SIZE];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = hasher.finalize();
    let mut bytes = [0_u8; reader_model::SHA256_BYTES];
    bytes.copy_from_slice(&digest);
    Ok(BookFingerprint::from_sha256(bytes))
}

fn map_open_error(error: &io::Error) -> EpubError {
    if error.kind() == io::ErrorKind::NotFound {
        EpubError::new(EpubErrorCode::FileNotFound)
    } else {
        EpubError::new(EpubErrorCode::IoError)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    const SHA256_EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const SHA256_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn fingerprint_bytes(bytes: &[u8]) -> BookFingerprint {
        let mut cursor = Cursor::new(bytes);
        fingerprint_reader(&mut cursor).expect("内存读取不应失败")
    }

    #[test]
    fn standard_vectors() {
        assert_eq!(fingerprint_bytes(b"").to_string(), SHA256_EMPTY);
        assert_eq!(fingerprint_bytes(b"abc").to_string(), SHA256_ABC);
    }

    #[test]
    fn matches_one_shot_digest_across_buffer_boundaries() {
        let data: Vec<u8> = (0..(2 * 1024 * 1024 + 123))
            .map(|index| (index % 251) as u8)
            .collect();

        let mut hasher = Sha256::new();
        hasher.update(&data);
        let digest = hasher.finalize();
        let mut bytes = [0_u8; reader_model::SHA256_BYTES];
        bytes.copy_from_slice(&digest);
        let expected = BookFingerprint::from_sha256(bytes);

        assert_eq!(fingerprint_bytes(&data), expected);
    }

    struct ReadTracker<'a> {
        remaining: &'a [u8],
        max_request: usize,
        requests: usize,
    }

    impl Read for ReadTracker<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.max_request = self.max_request.max(buffer.len());
            self.requests += 1;
            let count = self.remaining.len().min(buffer.len());
            let (head, tail) = self.remaining.split_at(count);
            buffer[..count].copy_from_slice(head);
            self.remaining = tail;
            Ok(count)
        }
    }

    #[test]
    fn never_requests_more_than_one_mib() {
        let data = vec![7_u8; 3 * 1024 * 1024 + 17];
        let mut tracker = ReadTracker {
            remaining: &data,
            max_request: 0,
            requests: 0,
        };

        let fingerprint = fingerprint_reader(&mut tracker).expect("读取应成功");
        assert!(
            tracker.max_request <= BUFFER_SIZE,
            "单次读取请求为 {} 字节",
            tracker.max_request
        );
        assert!(tracker.requests >= 4, "应分多次读取");
        assert_eq!(fingerprint, fingerprint_bytes(&data));
    }

    #[test]
    fn missing_file_is_file_not_found() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("missing.epub");
        let error = fingerprint_file(&path).expect_err("缺失文件应失败");
        assert_eq!(error.code(), EpubErrorCode::FileNotFound);
        assert!(!error.to_string().contains("/"));
    }

    #[test]
    fn directory_is_io_error() {
        let directory = tempfile::tempdir().expect("temp dir");
        let error = fingerprint_file(directory.path()).expect_err("目录应失败");
        assert_eq!(error.code(), EpubErrorCode::IoError);
    }

    #[test]
    fn read_failure_is_io_error() {
        struct FailingReader;

        impl Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated"))
            }
        }

        let mut reader = FailingReader;
        assert!(fingerprint_reader(&mut reader).is_err());
    }
}
