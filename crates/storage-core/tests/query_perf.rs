//! 1,000 本书查询性能证据（debug 构建）。

use std::time::Instant;

use reader_model::{BookSort, ListBooksQuery, SortDirection};
use storage_core::{NewBookRecord, StorageService};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("qurner-query-perf-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("创建临时目录");
    path
}

#[test]
fn thousand_books_query_p95() {
    let directory = temp_dir("list");
    let storage = StorageService::open(&directory.join("reader.sqlite3")).expect("db");
    for index in 0..1_000 {
        let id = format!("book-{index:04}");
        storage
            .insert_new_book(NewBookRecord {
                cover_cache_key: None,
                id: id.clone(),
                fingerprint: format!("fp-{index:04}"),
                source_path: format!("/tmp/{id}.epub"),
                title: format!("书 {index:04}"),
                author: format!("作者 {}", index % 50),
                title_search: format!("书 {index:04}").to_lowercase(),
                author_search: format!("作者 {}", index % 50).to_lowercase(),
                title_sort_key: format!("书 {index:04}").into_bytes(),
                author_sort_key: format!("作者 {}", index % 50).into_bytes(),
                manifest_json: "{}".to_string(),
                manifest_version: 1,
                imported_at: "2026-11-14T00:00:00Z".to_string(),
            })
            .expect("插入");
    }

    let mut samples = Vec::with_capacity(20);
    for _ in 0..20 {
        let start = Instant::now();
        let books = storage
            .list_books(ListBooksQuery {
                query: Some("0001".to_string()),
                sort: BookSort::Title,
                direction: SortDirection::Asc,
            })
            .expect("查询");
        assert!(!books.is_empty());
        samples.push(start.elapsed().as_secs_f64() * 1_000.0);
    }
    let mut sorted = samples.clone();
    sorted.sort_by(|left, right| left.total_cmp(right));
    let p50 = sorted[10];
    let p95 = sorted[18];
    eprintln!("M4D_DB_QUERY_P50_MS={p50:.3} M4D_DB_QUERY_P95_MS={p95:.3} samples={sorted:?}");
    assert!(p95 <= 200.0, "后端查询 p95 应 ≤200ms，实际 {p95:.3}ms");
    let _ = std::fs::remove_dir_all(&directory);
}
