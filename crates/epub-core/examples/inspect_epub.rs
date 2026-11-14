//! 只读 EPUB 检查示例。
//!
//! 用法：
//!
//! ```bash
//! cargo run -p epub-core --release --example inspect_epub -- <path>
//! ```
//!
//! 成功时输出一行 JSON 摘要；失败时只输出稳定错误码并以非零状态退出。

use std::path::Path;

use epub_core::{EpubErrorCode, fingerprint_file, parse_epub};
use serde_json::json;

fn main() {
    let Some(argument) = std::env::args().nth(1) else {
        eprintln!("usage: inspect_epub <path>");
        std::process::exit(2);
    };

    let path = Path::new(&argument);
    let fingerprint = match fingerprint_file(path) {
        Ok(fingerprint) => fingerprint,
        Err(error) => fail(error.code()),
    };
    let book = match parse_epub(path) {
        Ok(book) => book,
        Err(error) => fail(error.code()),
    };

    let summary = json!({
        "fingerprint": fingerprint.to_string(),
        "title": book.metadata.title.clone(),
        "authors": book.metadata.authors.len(),
        "language": book.metadata.language.clone(),
        "manifest": book.manifest.len(),
        "spine": book.spine.len(),
        "toc": book.toc.len(),
        "cover": book.cover.is_some(),
        "chapterWeights": book.chapter_weights.len(),
        "warnings": book
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect::<Vec<_>>(),
    });

    match serde_json::to_string(&summary) {
        Ok(line) => println!("{line}"),
        Err(_) => {
            eprintln!("error: internal");
            std::process::exit(1);
        }
    }
}

fn fail(code: EpubErrorCode) -> ! {
    eprintln!("error: {}", code.as_str());
    std::process::exit(1);
}
