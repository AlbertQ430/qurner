//! M6B 基准夹具：为目录下每个 EPUB 生成 ParsedBook manifest JSONL。
//!
//! 用法：`cargo run --release -p epub-core --example scale_manifest -- <fixtures-dir> <out.jsonl>`

use std::io::Write;

fn main() {
    let directory = std::env::args().nth(1).expect("fixtures-dir");
    let output = std::env::args().nth(2).expect("out.jsonl");
    let mut file = std::fs::File::create(&output).expect("create out");
    let mut files: Vec<_> = std::fs::read_dir(&directory)
        .expect("read fixtures")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "epub"))
        .collect();
    files.sort();
    for (index, path) in files.iter().enumerate() {
        let book = epub_core::parse_epub(path).expect("parse fixture");
        let fingerprint = epub_core::fingerprint_file(path).expect("fingerprint");
        let record = serde_json::json!({
            "index": index,
            "path": path.to_string_lossy(),
            "fingerprint": fingerprint.to_string(),
            "title": book.metadata.title.clone().unwrap_or_default(),
            "author": book.metadata.authors.join(", "),
            "schema_version": book.schema_version,
            "manifest": book,
        });
        writeln!(file, "{}", serde_json::to_string(&record).expect("json")).expect("write");
    }
    println!("{}", files.len());
}
