mod support;

use epub_core::{EpubErrorCode, fingerprint_file, parse_epub};
use support::fixtures::large_epub;

const SHA256_EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const SHA256_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

#[test]
fn file_fingerprint_matches_standard_vectors() {
    let directory = tempfile::tempdir().expect("temp dir");

    let empty = directory.path().join("empty.bin");
    std::fs::write(&empty, b"").expect("write");
    assert_eq!(
        fingerprint_file(&empty).expect("指纹").to_string(),
        SHA256_EMPTY
    );

    let abc = directory.path().join("abc.bin");
    std::fs::write(&abc, b"abc").expect("write");
    assert_eq!(
        fingerprint_file(&abc).expect("指纹").to_string(),
        SHA256_ABC
    );
}

#[test]
fn missing_file_error_does_not_leak_path() {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("no-such.epub");
    let error = fingerprint_file(&path).expect_err("缺失文件应失败");

    assert_eq!(error.code(), EpubErrorCode::FileNotFound);
    let message = error.to_string();
    assert!(!message.contains(path.to_str().expect("utf8 path")));
    assert!(!message.contains(directory.path().to_str().expect("utf8 dir")));
}

#[test]
fn large_epub_is_unchanged_by_parsing() {
    let fixture = large_epub();
    let path = fixture.path();

    let before_fingerprint = fingerprint_file(path).expect("解析前指纹");
    let before_bytes = std::fs::read(path).expect("解析前读取");
    let before_len = before_bytes.len();

    let book = parse_epub(path).expect("100 MB 夹具应可解析");
    assert_eq!(book.spine.len(), 1);

    let after_fingerprint = fingerprint_file(path).expect("解析后指纹");
    let after_bytes = std::fs::read(path).expect("解析后读取");

    assert_eq!(before_fingerprint, after_fingerprint);
    assert_eq!(before_len, after_bytes.len());
    assert_eq!(before_bytes, after_bytes);
}
