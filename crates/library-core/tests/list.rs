use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use library_core::LibraryService;
use reader_model::{BookSort, BookSummary, ListBooksQuery, SortDirection};
use storage_core::{NewBookRecord, StorageService};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "qurner-library-list-{}-{name}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("创建临时目录");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn insert_record(
    storage: &StorageService,
    id: &str,
    title: &str,
    author: &str,
    imported_at: &str,
    title_key: u32,
    author_key: u32,
) {
    let record = NewBookRecord {
        cover_cache_key: None,
        id: id.to_string(),
        fingerprint: format!("fp-{id}"),
        source_path: format!("/tmp/{id}.epub"),
        title: title.to_string(),
        author: author.to_string(),
        title_search: title.to_lowercase(),
        author_search: author.to_lowercase(),
        title_sort_key: title_key.to_be_bytes().to_vec(),
        author_sort_key: author_key.to_be_bytes().to_vec(),
        manifest_json: "{}".to_string(),
        manifest_version: 1,
        imported_at: imported_at.to_string(),
    };
    storage.insert_new_book(record).expect("insert record");
}

fn query(text: Option<&str>, sort: BookSort, direction: SortDirection) -> ListBooksQuery {
    ListBooksQuery {
        query: text.map(str::to_string),
        sort,
        direction,
    }
}

fn ids(
    service: &LibraryService,
    text: Option<&str>,
    sort: BookSort,
    direction: SortDirection,
) -> Vec<String> {
    service
        .list_books(query(text, sort, direction))
        .expect("list")
        .into_iter()
        .map(|summary| summary.id)
        .collect()
}

fn seeded_library(directory: &Path) -> (StorageService, LibraryService) {
    let storage = StorageService::open(&directory.join("reader.sqlite3")).expect("open");
    insert_record(&storage, "a", "Alpha", "Zed", "2026-01-02T00:00:00Z", 2, 2);
    insert_record(&storage, "b", "Beta", "Amy", "2026-01-01T00:00:00Z", 1, 3);
    insert_record(&storage, "c", "Gamma", "Bob", "2026-01-03T00:00:00Z", 3, 1);
    let service = LibraryService::new(storage.clone());
    (storage, service)
}

#[test]
fn lists_books_with_three_sorts_and_both_directions() {
    let directory = TempDir::new("sort");
    let (storage, service) = seeded_library(&directory.path);

    assert_eq!(
        ids(&service, None, BookSort::Title, SortDirection::Asc),
        vec!["b", "a", "c"]
    );
    assert_eq!(
        ids(&service, None, BookSort::Title, SortDirection::Desc),
        vec!["c", "a", "b"]
    );
    assert_eq!(
        ids(&service, None, BookSort::Author, SortDirection::Asc),
        vec!["c", "a", "b"]
    );
    assert_eq!(
        ids(&service, None, BookSort::Author, SortDirection::Desc),
        vec!["b", "a", "c"]
    );
    assert_eq!(
        ids(&service, None, BookSort::ImportedAt, SortDirection::Asc),
        vec!["b", "a", "c"]
    );
    assert_eq!(
        ids(&service, None, BookSort::ImportedAt, SortDirection::Desc),
        vec!["c", "a", "b"]
    );

    storage.shutdown().expect("shutdown");
}

#[test]
fn searches_title_and_author_only() {
    let directory = TempDir::new("search");
    let storage = StorageService::open(&directory.path.join("reader.sqlite3")).expect("open");
    insert_record(
        &storage,
        "needle",
        "Alpha",
        "Alice",
        "2026-01-01T00:00:00Z",
        1,
        1,
    );
    insert_record(
        &storage,
        "b",
        "中文书名",
        "张三",
        "2026-01-02T00:00:00Z",
        2,
        2,
    );
    let service = LibraryService::new(storage.clone());

    assert_eq!(
        ids(&service, Some("alpha"), BookSort::Title, SortDirection::Asc),
        vec!["needle"]
    );
    assert_eq!(
        ids(&service, Some("ALICE"), BookSort::Title, SortDirection::Asc),
        vec!["needle"]
    );
    assert_eq!(
        ids(&service, Some("中文"), BookSort::Title, SortDirection::Asc),
        vec!["b"]
    );
    assert_eq!(
        ids(&service, Some("张三"), BookSort::Title, SortDirection::Asc),
        vec!["b"]
    );
    assert_eq!(
        ids(
            &service,
            Some("needle"),
            BookSort::Title,
            SortDirection::Asc
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        ids(
            &service,
            Some("missing"),
            BookSort::Title,
            SortDirection::Asc
        ),
        Vec::<String>::new()
    );

    storage.shutdown().expect("shutdown");
}

#[test]
fn removed_books_are_hidden_from_list() {
    let directory = TempDir::new("removed");
    let (storage, service) = seeded_library(&directory.path);
    service.soft_remove("b").expect("soft remove");

    let listed = ids(&service, None, BookSort::Title, SortDirection::Asc);
    assert_eq!(listed, vec!["a", "c"]);

    storage.shutdown().expect("shutdown");
}

#[test]
fn summary_round_trips_through_json() {
    let directory = TempDir::new("json");
    let (storage, service) = seeded_library(&directory.path);

    let summaries: Vec<BookSummary> = service
        .list_books(query(None, BookSort::Title, SortDirection::Asc))
        .expect("list");
    let json = serde_json::to_string(&summaries).expect("serialize");
    let restored: Vec<BookSummary> = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored, summaries);
    assert!(json.contains("\"coverCacheKey\""));
    assert!(json.contains("\"importedAt\""));

    storage.shutdown().expect("shutdown");
}

#[test]
fn thousand_books_query_p95_within_threshold() {
    let directory = TempDir::new("perf");
    let storage = StorageService::open(&directory.path.join("reader.sqlite3")).expect("open");
    for index in 0..1_000_u32 {
        insert_record(
            &storage,
            &format!("book-{index:04}"),
            &format!("书名 {index:04}"),
            &format!("作者 {}", index % 20),
            &format!("2026-01-{:02}T00:00:00Z", (index % 28) + 1),
            index,
            index % 20,
        );
    }
    let service = LibraryService::new(storage.clone());

    let request = query(Some("书名"), BookSort::Title, SortDirection::Asc);
    let mut timings = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let books = service.list_books(request.clone()).expect("list");
        timings.push(start.elapsed());
        assert_eq!(books.len(), 1_000);
    }
    timings.sort();
    let p95 = timings[timings.len() - 1];
    eprintln!("p95 query = {p95:?} (10 runs, max)");
    assert!(p95 <= Duration::from_millis(200), "p95 超过 200ms：{p95:?}");

    storage.shutdown().expect("shutdown");
}
