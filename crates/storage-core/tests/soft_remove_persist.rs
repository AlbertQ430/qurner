//! 软移除必须持久化：关闭并重新打开数据库后仍为 removed。

use reader_model::{BookSort, ListBooksQuery, SortDirection};
use storage_core::{NewBookRecord, StorageService};

fn record() -> NewBookRecord {
    NewBookRecord {
        cover_cache_key: None,
        id: "book-1".to_string(),
        fingerprint: "fp-1".to_string(),
        source_path: "/tmp/book-1.epub".to_string(),
        title: "书名".to_string(),
        author: "作者".to_string(),
        title_search: "书名".to_string(),
        author_search: "作者".to_string(),
        title_sort_key: "书名".as_bytes().to_vec(),
        author_sort_key: "作者".as_bytes().to_vec(),
        manifest_json: "{}".to_string(),
        manifest_version: 1,
        imported_at: "2026-11-15T00:00:00Z".to_string(),
    }
}

#[test]
fn soft_remove_persists_after_reopen() {
    let directory =
        std::env::temp_dir().join(format!("qurner-soft-remove-persist-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("temp");
    let path = directory.join("reader.sqlite3");

    let storage = StorageService::open(&path).expect("open");
    storage.insert_new_book(record()).expect("insert");
    storage.soft_remove_book("book-1").expect("soft remove");
    storage.shutdown().expect("shutdown");

    let reopened = StorageService::open(&path).expect("reopen");
    let books = reopened
        .list_books(ListBooksQuery {
            query: None,
            sort: BookSort::ImportedAt,
            direction: SortDirection::Desc,
        })
        .expect("list");
    assert!(books.is_empty(), "重启后不应显示已移除书籍");
    let existing = reopened
        .find_by_fingerprint("fp-1")
        .expect("find")
        .expect("record");
    assert_eq!(existing.lifecycle, "removed");
    reopened.shutdown().expect("shutdown");
    let _ = std::fs::remove_dir_all(&directory);
}
