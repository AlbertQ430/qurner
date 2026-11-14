use std::path::{Path, PathBuf};

use storage_core::{StorageError, StorageService};

fn temp_dir(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "qurner-storage-public-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("应能创建测试目录");
    directory
}

fn cleanup(path: &Path) {
    if let Some(directory) = path.parent() {
        let _ = std::fs::remove_dir_all(directory);
    }
}

#[test]
fn public_service_opens_reports_version_and_shuts_down() {
    let directory = temp_dir("smoke");
    let path = directory.join("reader.sqlite3");

    let service = StorageService::open(&path).expect("应能打开");
    assert_eq!(service.schema_version().expect("version"), 1);
    service.shutdown().expect("shutdown");

    cleanup(&path);
}

#[test]
fn shutdown_is_visible_to_cloned_handles() {
    let directory = temp_dir("clone");
    let path = directory.join("reader.sqlite3");

    let service = StorageService::open(&path).expect("应能打开");
    let remaining = service.clone();
    service.shutdown().expect("shutdown");

    let error = remaining.schema_version().expect_err("shutdown 后应不可用");
    assert_eq!(error, StorageError::ThreadUnavailable);
    assert_eq!(error.as_str(), "threadUnavailable");

    let message = error.to_string();
    assert!(!message.contains(path.to_str().expect("utf8 path")));
    assert!(!message.contains("sqlite"));

    cleanup(&path);
}

#[test]
fn opens_path_with_chinese_characters_and_spaces() {
    let directory = temp_dir("中文 路径");
    let path = directory.join("书库 数据.sqlite3");

    let service = StorageService::open(&path).expect("应能打开中文空格路径");
    assert_eq!(service.schema_version().expect("version"), 1);
    service.shutdown().expect("shutdown");

    cleanup(&path);
}

#[test]
fn service_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<StorageService>();
}
