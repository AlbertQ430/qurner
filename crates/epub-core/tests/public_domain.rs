//! 可选的公有领域 EPUB 兼容性冒烟。
//!
//! 只有设置 `QURNER_TEST_EPUB_DIR` 时才执行；目录路径不得写入仓库，
//! 未设置时测试直接通过，不依赖本地私人书籍。

use std::path::Path;

use epub_core::parse_epub;

#[test]
#[ignore = "requires QURNER_TEST_EPUB_DIR"]
fn parses_public_domain_epubs_from_env_dir() {
    let Ok(directory) = std::env::var("QURNER_TEST_EPUB_DIR") else {
        eprintln!("QURNER_TEST_EPUB_DIR 未设置，跳过公有领域 EPUB 冒烟");
        return;
    };

    let mut paths: Vec<_> = std::fs::read_dir(Path::new(&directory))
        .expect("应能读取 QURNER_TEST_EPUB_DIR")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("epub"))
        .collect();
    paths.sort();

    assert!(paths.len() >= 3, "至少需要 3 本公有领域 EPUB");
    for path in paths {
        let book = parse_epub(&path).unwrap_or_else(|error| {
            panic!(
                "{}: {error}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )
        });
        assert!(!book.spine.is_empty());
        assert!(!book.manifest.is_empty());
        assert_eq!(book.chapter_weights.len(), book.spine.len());
        eprintln!(
            "{}: title={:?} spine={} toc={} cover={} weights_nonzero={}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            book.metadata.title,
            book.spine.len(),
            book.toc.len(),
            book.cover.is_some(),
            book.chapter_weights
                .iter()
                .filter(|weight| **weight > 0)
                .count()
        );
    }
}
