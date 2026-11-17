//! M6B 基准：对真实 SQLite 执行 list_books（标题/作者搜索）并输出样本与 p50/p95。
//!
//! 用法：`cargo run --release -p storage-core --example list_bench -- <db> <query> [runs] [expected_rows]`
//!
//! 期望行数（默认 1）与实际结果不一致时退出码为 1，避免空/错误结果仍按延迟通过。

use std::time::Instant;

fn percentile(sorted: &[f64], percent: f64) -> f64 {
    let index = ((percent / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

fn main() {
    let database = std::env::args().nth(1).expect("db path");
    let query = std::env::args().nth(2).expect("query");
    let runs: usize = std::env::args()
        .nth(3)
        .and_then(|value| value.parse().ok())
        .unwrap_or(10);
    let expected_rows: usize = std::env::args()
        .nth(4)
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let service =
        storage_core::StorageService::open(std::path::Path::new(&database)).expect("open storage");
    let make_query = || reader_model::ListBooksQuery {
        query: Some(query.clone()),
        sort: reader_model::BookSort::ImportedAt,
        direction: reader_model::SortDirection::Desc,
    };
    let _ = service.list_books(make_query()).expect("warmup");
    let mut samples = Vec::with_capacity(runs);
    let mut row_count = 0_usize;
    for _ in 0..runs {
        let start = Instant::now();
        let rows = service.list_books(make_query()).expect("list books");
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        row_count = rows.len();
        if rows.len() != expected_rows {
            eprintln!(
                "query returned {} rows, expected {expected_rows}; benchmark result is not meaningful",
                rows.len()
            );
            service.shutdown().expect("shutdown");
            std::process::exit(1);
        }
    }
    let mut sorted = samples.clone();
    sorted.sort_by(|left, right| left.total_cmp(right));
    let p50 = percentile(&sorted, 50.0);
    let p95 = percentile(&sorted, 95.0);
    let summary = format!(
        "{{\"label\":\"search\",\"unit\":\"ms\",\"runs\":{runs},\"rows\":{row_count},\"expected_rows\":{expected_rows},\"samples\":{samples:?},\"p50\":{p50:.2},\"p95\":{p95:.2},\"threshold\":200,\"pass\":{}}}",
        p95 <= 200.0 && row_count == expected_rows
    );
    println!("{summary}");
    service.shutdown().expect("shutdown");
    std::process::exit(if p95 <= 200.0 { 0 } else { 1 });
}
