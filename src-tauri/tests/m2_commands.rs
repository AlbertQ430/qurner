use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use library_core::LibraryService;
use qurner_lib::state::{AppState, initialize_state};
use serde_json::{Value, json};
use storage_core::{NewBookRecord, StorageService};
use tauri::test::{MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{Listener, WebviewWindow, WebviewWindowBuilder};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "qurner-m2d-{}-{name}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn db_path(&self) -> PathBuf {
        self.path.join("data").join("reader.sqlite3")
    }

    fn app_data_dir(&self) -> PathBuf {
        self.path.join("data")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

const CONTAINER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

const NAV: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">第一章</a></li></ol></nav></body></html>"#;

struct EpubOptions<'a> {
    title: Option<&'a str>,
    author: Option<&'a str>,
    meta: &'a str,
    body: &'a str,
    extra: &'a [(&'a str, &'a [u8])],
    include_opf: bool,
}

impl Default for EpubOptions<'_> {
    fn default() -> Self {
        Self {
            title: Some("测试书"),
            author: Some("测试作者"),
            meta: "",
            body: "<p>正文</p>",
            extra: &[],
            include_opf: true,
        }
    }
}

fn build_epub(options: EpubOptions<'_>) -> Vec<u8> {
    let meta = options.meta;
    let mut metadata = String::new();
    if let Some(title) = options.title {
        metadata.push_str(&format!("    <dc:title>{title}</dc:title>\n"));
    }
    if let Some(author) = options.author {
        metadata.push_str(&format!("    <dc:creator>{author}</dc:creator>\n"));
    }
    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
{metadata}    <dc:identifier id="id">urn:uuid:m2d-test</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
    {meta}
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="c1"/></spine>
</package>
"#
    );
    let chapter = format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title></title></head><body>{}</body></html>",
        options.body
    );

    let mut entries: Vec<(&str, &[u8])> = vec![
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", CONTAINER.as_bytes()),
        ("OEBPS/nav.xhtml", NAV.as_bytes()),
        ("OEBPS/chapter1.xhtml", chapter.as_bytes()),
    ];
    if options.include_opf {
        entries.push(("OEBPS/content.opf", opf.as_bytes()));
    }
    entries.extend_from_slice(options.extra);
    build_zip(&entries)
}

fn build_encrypted_epub() -> Vec<u8> {
    let encryption = br#"<?xml version="1.0" encoding="UTF-8"?>
<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <EncryptedData xmlns="http://www.w3.org/2001/04/xmlenc#">
    <EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#aes256-cbc"/>
    <CipherData>
      <CipherReference URI="OEBPS/chapter1.xhtml"/>
    </CipherData>
  </EncryptedData>
</encryption>
"#;
    build_epub(EpubOptions {
        extra: &[("META-INF/encryption.xml", encryption)],
        ..EpubOptions::default()
    })
}

fn build_fixed_layout_epub() -> Vec<u8> {
    build_epub(EpubOptions {
        meta: r#"<meta property="rendition:layout">pre-paginated</meta>"#,
        ..EpubOptions::default()
    })
}

fn build_vertical_epub() -> Vec<u8> {
    build_epub(EpubOptions {
        body: r#"<p style="writing-mode: vertical-rl">竖排正文</p>"#,
        ..EpubOptions::default()
    })
}

fn build_scripted_epub() -> Vec<u8> {
    build_epub(EpubOptions {
        body: "<script>alert(1)</script><p>脚本正文</p>",
        ..EpubOptions::default()
    })
}

fn build_broken_epub() -> Vec<u8> {
    build_epub(EpubOptions {
        include_opf: false,
        ..EpubOptions::default()
    })
}

fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut central = Vec::new();
    let mut offset = 0_u32;

    for (name, data) in entries {
        let name = name.as_bytes();
        let size = data.len() as u32;
        let crc = crc32(data);

        output.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
        output.extend_from_slice(&20_u16.to_le_bytes());
        output.extend_from_slice(&0x0800_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0x0021_u16.to_le_bytes());
        output.extend_from_slice(&crc.to_le_bytes());
        output.extend_from_slice(&size.to_le_bytes());
        output.extend_from_slice(&size.to_le_bytes());
        output.extend_from_slice(&(name.len() as u16).to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(name);
        output.extend_from_slice(data);

        central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
        central.extend_from_slice(&20_u16.to_le_bytes());
        central.extend_from_slice(&20_u16.to_le_bytes());
        central.extend_from_slice(&0x0800_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0x0021_u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);

        offset += 30 + name.len() as u32 + size;
    }

    let central_offset = offset;
    output.extend_from_slice(&central);
    output.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    output.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    output.extend_from_slice(&(central.len() as u32).to_le_bytes());
    output.extend_from_slice(&central_offset.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn write_file(directory: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, bytes).expect("write fixture");
    path
}

fn build_app(state: AppState) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            qurner_lib::commands::start_import,
            qurner_lib::commands::list_books,
            qurner_lib::commands::soft_remove_book,
        ])
        .build(mock_context(noop_assets()))
        .expect("build app");
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build window");
    (app, window)
}

fn open_state(temp: &TempDir) -> AppState {
    std::fs::create_dir_all(temp.app_data_dir()).expect("create data dir");
    let storage = StorageService::open(&temp.db_path()).expect("open database");
    AppState::ready(library_core::LibraryService::new(storage))
}

fn invoke(window: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    let request = InvokeRequest {
        cmd: cmd.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: "tauri://localhost".parse().expect("url"),
        body: tauri::ipc::InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: tauri::test::INVOKE_KEY.to_string(),
    };
    get_ipc_response(window, request)
        .map(|response| response.deserialize::<Value>().expect("json response"))
}

fn error_code(error: &Value) -> &str {
    error
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

#[derive(Default)]
struct Events {
    progress: Mutex<Vec<Value>>,
    completed: Mutex<Vec<Value>>,
}

fn listen_events(app: &tauri::App<MockRuntime>) -> Arc<Events> {
    let events = Arc::new(Events::default());
    let progress = Arc::clone(&events);
    app.listen("import-progress", move |event| {
        let value: Value = serde_json::from_str(event.payload()).expect("progress payload");
        progress.progress.lock().expect("lock").push(value);
    });
    let completed = Arc::clone(&events);
    app.listen("import-completed", move |event| {
        let value: Value = serde_json::from_str(event.payload()).expect("completed payload");
        completed.completed.lock().expect("lock").push(value);
    });
    events
}

fn wait_for_completed(events: &Events, count: usize) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let completed = events.completed.lock().expect("lock").clone();
        if completed.len() >= count {
            return completed;
        }
        assert!(Instant::now() < deadline, "等待 import-completed 超时");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn start_import(window: &WebviewWindow<MockRuntime>, paths: &[String]) -> Result<Value, Value> {
    invoke(window, "startImport", json!({ "paths": paths }))
}

fn list_books(
    window: &WebviewWindow<MockRuntime>,
    query: Option<&str>,
    sort: &str,
    direction: &str,
) -> Result<Value, Value> {
    let mut input = json!({ "sort": sort, "direction": direction });
    if let Some(query) = query {
        input["query"] = json!(query);
    }
    invoke(window, "listBooks", json!({ "input": input }))
}

fn items_of(completed: &Value) -> Vec<Value> {
    completed
        .get("items")
        .and_then(Value::as_array)
        .expect("items")
        .clone()
}

#[test]
fn imports_single_book_and_reactivates_after_soft_remove() {
    let temp = TempDir::new("single");
    let file = write_file(
        &temp.path,
        "我的 书.epub",
        &build_epub(EpubOptions::default()),
    );
    let state = open_state(&temp);
    let (app, window) = build_app(state);
    let events = listen_events(&app);

    let result = start_import(&window, &[file.to_string_lossy().into_owned()]).expect("import");
    let job_id = result["jobId"].as_str().expect("job id").to_string();
    assert_eq!(job_id.len(), 36);
    assert_eq!(
        job_id.chars().filter(|character| *character == '-').count(),
        4
    );

    let completed = wait_for_completed(&events, 1);
    assert_eq!(completed.len(), 1);
    let progress = events.progress.lock().expect("lock").clone();
    assert_eq!(progress.len(), 1);
    assert_eq!(progress[0]["jobId"], job_id);
    assert_eq!(progress[0]["completed"], 1);
    assert_eq!(progress[0]["total"], 1);
    assert_eq!(progress[0]["item"]["status"], "imported");
    assert_eq!(progress[0]["item"]["sourceName"], "我的 书.epub");
    assert!(!progress[0].to_string().contains("/Users/"));
    assert!(
        !progress[0]
            .to_string()
            .contains(temp.path.to_str().expect("utf8"))
    );

    assert_eq!(completed[0]["jobId"], job_id);
    let items = items_of(&completed[0]);
    assert_eq!(items.len(), 1);
    let book_id = items[0]["bookId"].as_str().expect("book id").to_string();

    let listed = list_books(&window, None, "title", "asc").expect("list");
    assert_eq!(listed.as_array().expect("array").len(), 1);
    assert_eq!(listed[0]["id"], book_id);
    assert_eq!(listed[0]["title"], "测试书");
    assert_eq!(listed[0]["coverUrl"], Value::Null);

    invoke(&window, "softRemoveBook", json!({ "bookId": book_id })).expect("soft remove");
    assert_eq!(
        list_books(&window, None, "title", "asc")
            .expect("list")
            .as_array()
            .expect("array")
            .len(),
        0
    );

    start_import(&window, &[file.to_string_lossy().into_owned()]).expect("reimport");
    let completed = wait_for_completed(&events, 2);
    let items = items_of(completed.last().expect("completed"));
    assert_eq!(items[0]["status"], "reactivated");
    assert_eq!(items[0]["bookId"], book_id);
}

#[test]
fn imports_batch_in_order_with_duplicate_and_failures() {
    let temp = TempDir::new("batch");
    let success = write_file(
        &temp.path,
        "success.epub",
        &build_epub(EpubOptions::default()),
    );
    let duplicate = write_file(
        &temp.path,
        "duplicate.epub",
        &build_epub(EpubOptions::default()),
    );
    let missing = temp.path.join("missing.epub");
    let broken = write_file(&temp.path, "broken.epub", &build_broken_epub());

    let state = open_state(&temp);
    let (app, window) = build_app(state);
    let events = listen_events(&app);

    let paths: Vec<String> = [success, duplicate, missing, broken]
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    start_import(&window, &paths).expect("start");

    let completed = wait_for_completed(&events, 1);
    let items = items_of(&completed[0]);
    assert_eq!(items.len(), 4);
    assert_eq!(items[0]["status"], "imported");
    assert_eq!(items[1]["status"], "duplicate");
    assert_eq!(items[2]["status"], "failed");
    assert_eq!(items[2]["error"]["code"], "E_FILE_NOT_FOUND");
    assert_eq!(items[3]["status"], "failed");
    assert_eq!(items[3]["error"]["code"], "E_EPUB_INVALID");

    let progress = events.progress.lock().expect("lock").clone();
    assert_eq!(progress.len(), 4);
    for (index, event) in progress.iter().enumerate() {
        assert_eq!(event["completed"], index + 1);
        assert_eq!(event["total"], 4);
        assert_eq!(
            event["item"]["sourceName"],
            [
                "success.epub",
                "duplicate.epub",
                "missing.epub",
                "broken.epub"
            ][index]
        );
    }
}

#[test]
fn maps_domain_errors_to_stable_codes() {
    let temp = TempDir::new("error-codes");
    let missing = temp.path.join("missing.epub");
    let directory = temp.path.join("folder.epub");
    std::fs::create_dir(&directory).expect("create directory");
    let broken = write_file(&temp.path, "broken.epub", &build_broken_epub());
    let encrypted = write_file(&temp.path, "encrypted.epub", &build_encrypted_epub());
    let fixed = write_file(&temp.path, "fixed.epub", &build_fixed_layout_epub());
    let vertical = write_file(&temp.path, "vertical.epub", &build_vertical_epub());
    let scripted = write_file(&temp.path, "scripted.epub", &build_scripted_epub());

    let state = open_state(&temp);
    let (app, window) = build_app(state);
    let events = listen_events(&app);

    let paths: Vec<String> = [
        missing, directory, broken, encrypted, fixed, vertical, scripted,
    ]
    .iter()
    .map(|path| path.to_string_lossy().into_owned())
    .collect();
    start_import(&window, &paths).expect("start");
    let completed = wait_for_completed(&events, 1);
    let items = items_of(&completed[0]);
    assert_eq!(items.len(), 7);
    let expected = [
        "E_FILE_NOT_FOUND",
        "E_FILE_UNREADABLE",
        "E_EPUB_INVALID",
        "E_EPUB_ENCRYPTED",
        "E_EPUB_FIXED_LAYOUT",
        "E_EPUB_VERTICAL",
        "E_EPUB_SCRIPT_REQUIRED",
    ];
    for (index, code) in expected.iter().enumerate() {
        assert_eq!(items[index]["status"], "failed", "第 {index} 项");
        assert_eq!(items[index]["error"]["code"], *code, "第 {index} 项");
        assert!(!items[index].to_string().contains("/Users/"));
        assert!(!items[index].to_string().contains("rusqlite"));
    }
    assert_eq!(
        list_books(&window, None, "title", "asc")
            .expect("list")
            .as_array()
            .expect("array")
            .len(),
        0,
        "失败导入不应留下记录"
    );
}

#[test]
fn rejects_invalid_inputs_without_side_effects() {
    let temp = TempDir::new("invalid");
    let state = open_state(&temp);
    let (app, window) = build_app(state);
    let events = listen_events(&app);

    for body in [
        json!({ "paths": [] }),
        json!({ "paths": ["   "] }),
        json!({ "paths": (0..=1000).map(|index| format!("/tmp/{index}.epub")).collect::<Vec<_>>() }),
        json!({ "paths": "book.epub" }),
    ] {
        let error = invoke(&window, "startImport", body).expect_err("invalid paths");
        assert_eq!(error_code(&error), "E_INVALID_INPUT");
    }

    for body in [
        json!({ "input": { "sort": "unknown", "direction": "asc" } }),
        json!({ "input": { "sort": "title", "direction": "up" } }),
        json!({ "input": "title" }),
        json!({}),
    ] {
        let error = invoke(&window, "listBooks", body).expect_err("invalid list");
        assert_eq!(error_code(&error), "E_INVALID_INPUT");
    }

    for body in [json!({ "bookId": "   " }), json!({})] {
        let error = invoke(&window, "softRemoveBook", body).expect_err("invalid book id");
        assert_eq!(error_code(&error), "E_INVALID_INPUT");
    }

    assert!(events.progress.lock().expect("lock").is_empty());
    assert!(events.completed.lock().expect("lock").is_empty());
    assert_eq!(
        list_books(&window, None, "title", "asc")
            .expect("list")
            .as_array()
            .expect("array")
            .len(),
        0
    );
}

#[test]
fn list_books_sorts_filters_and_serializes() {
    let temp = TempDir::new("list");
    let alpha = write_file(
        &temp.path,
        "alpha.epub",
        &build_epub(EpubOptions {
            title: Some("Alpha"),
            author: Some("Zed"),
            body: "<p>alpha body</p>",
            ..EpubOptions::default()
        }),
    );
    let beta = write_file(
        &temp.path,
        "beta.epub",
        &build_epub(EpubOptions {
            title: Some("Beta"),
            author: Some("Amy"),
            body: "<p>beta body</p>",
            ..EpubOptions::default()
        }),
    );
    let chinese = write_file(
        &temp.path,
        "chinese.epub",
        &build_epub(EpubOptions {
            title: Some("中文书名"),
            author: Some("张三"),
            body: "<p>中文正文</p>",
            ..EpubOptions::default()
        }),
    );

    let state = open_state(&temp);
    let (app, window) = build_app(state);
    let events = listen_events(&app);
    let paths: Vec<String> = [alpha, beta, chinese]
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    start_import(&window, &paths).expect("start");
    wait_for_completed(&events, 1);

    let titles = |value: &Value| -> Vec<String> {
        value
            .as_array()
            .expect("array")
            .iter()
            .map(|summary| summary["title"].as_str().expect("title").to_string())
            .collect()
    };

    let title_asc = titles(&list_books(&window, None, "title", "asc").expect("list"));
    let title_desc = titles(&list_books(&window, None, "title", "desc").expect("list"));
    assert_eq!(title_asc.len(), 3);
    assert_eq!(title_desc.len(), 3);
    let position = |values: &[String], name: &str| {
        values
            .iter()
            .position(|value| value == name)
            .expect("present")
    };
    assert!(position(&title_asc, "Alpha") < position(&title_asc, "Beta"));
    assert!(position(&title_desc, "Alpha") > position(&title_desc, "Beta"));

    let author_asc = list_books(&window, None, "author", "asc").expect("list");
    let author_desc = list_books(&window, None, "author", "desc").expect("list");
    let author_asc = author_asc.as_array().expect("array");
    let author_desc = author_desc.as_array().expect("array");
    assert!(
        author_asc.iter().position(|value| value["title"] == "Beta")
            < author_asc
                .iter()
                .position(|value| value["title"] == "Alpha")
    );
    assert!(
        author_desc
            .iter()
            .position(|value| value["title"] == "Beta")
            > author_desc
                .iter()
                .position(|value| value["title"] == "Alpha")
    );

    let imported = list_books(&window, None, "importedAt", "desc").expect("list");
    assert_eq!(imported.as_array().expect("array").len(), 3);

    let filtered = list_books(&window, Some("alpha"), "title", "asc").expect("list");
    let filtered = filtered.as_array().expect("array");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0]["title"], "Alpha");
    assert_eq!(filtered[0]["coverUrl"], Value::Null);
    assert_eq!(filtered[0]["lastOpenedAt"], Value::Null);
    assert!(filtered[0].get("coverCacheKey").is_none());
    assert!(
        filtered[0]["importedAt"]
            .as_str()
            .expect("importedAt")
            .ends_with('Z')
    );

    let chinese = list_books(&window, Some("中文"), "title", "asc").expect("list");
    assert_eq!(chinese.as_array().expect("array").len(), 1);
    assert_eq!(
        list_books(&window, Some("zzz"), "title", "asc")
            .expect("list")
            .as_array()
            .expect("array")
            .len(),
        0
    );
}

#[test]
fn initialization_fails_when_data_directory_cannot_be_created() {
    let temp = TempDir::new("init-dir");
    let blocker = temp.path.join("blocker");
    std::fs::write(&blocker, b"not a directory").expect("write blocker");
    let file = write_file(&temp.path, "book.epub", &build_epub(EpubOptions::default()));

    let state = initialize_state(&blocker.join("sub"));
    assert_failed_state_returns_storage(state, &file);
}

#[test]
fn initialization_fails_for_corrupted_database() {
    let temp = TempDir::new("init-corrupt");
    let directory = temp.path.join("corrupt");
    std::fs::create_dir_all(&directory).expect("create dir");
    std::fs::write(directory.join("reader.sqlite3"), b"not a sqlite database")
        .expect("write corrupted database");
    let file = write_file(&temp.path, "book.epub", &build_epub(EpubOptions::default()));

    let state = initialize_state(&directory);
    assert_failed_state_returns_storage(state, &file);
}

#[test]
fn initialization_fails_for_newer_schema_version() {
    let temp = TempDir::new("init-newer");
    let directory = temp.path.join("newer");
    std::fs::create_dir_all(&directory).expect("create dir");
    let database = directory.join("reader.sqlite3");

    let service = StorageService::open(&database).expect("create database");
    service.shutdown().expect("shutdown");
    let connection = rusqlite::Connection::open(&database).expect("open raw database");
    connection
        .execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES (99, '2026-11-14T00:00:00Z')",
            [],
        )
        .expect("insert newer version");
    drop(connection);

    let file = write_file(&temp.path, "book.epub", &build_epub(EpubOptions::default()));
    let state = initialize_state(&directory);
    assert_failed_state_returns_storage(state, &file);
}

fn assert_failed_state_returns_storage(state: AppState, file: &Path) {
    let (app, window) = build_app(state);
    let events = listen_events(&app);
    let path = file.to_string_lossy().into_owned();

    let error = invoke(&window, "startImport", json!({ "paths": [path] })).expect_err("storage");
    assert_eq!(error_code(&error), "E_STORAGE");
    let error = invoke(
        &window,
        "listBooks",
        json!({ "input": { "sort": "title", "direction": "asc" } }),
    )
    .expect_err("storage");
    assert_eq!(error_code(&error), "E_STORAGE");
    let error =
        invoke(&window, "softRemoveBook", json!({ "bookId": "book-1" })).expect_err("storage");
    assert_eq!(error_code(&error), "E_STORAGE");

    assert!(events.progress.lock().expect("lock").is_empty());
    assert!(events.completed.lock().expect("lock").is_empty());
}

#[test]
fn preserves_paths_with_trailing_spaces() {
    let temp = TempDir::new("trailing-space");
    let file = write_file(
        &temp.path,
        "trailing space .epub ",
        &build_epub(EpubOptions::default()),
    );
    let raw = file.to_string_lossy().into_owned();
    assert!(raw.ends_with(' '), "测试路径应以空格结尾");

    let state = open_state(&temp);
    let (app, window) = build_app(state);
    let events = listen_events(&app);

    start_import(&window, &[raw]).expect("start");
    let completed = wait_for_completed(&events, 1);
    let items = items_of(&completed[0]);
    assert_eq!(items[0]["status"], "imported");
    assert_eq!(items[0]["sourceName"], "trailing space .epub ");

    assert_eq!(
        list_books(&window, None, "title", "asc")
            .expect("list")
            .as_array()
            .expect("array")
            .len(),
        1
    );
}

#[test]
fn limits_import_jobs_to_two_concurrent_executions() {
    let temp = TempDir::new("job-limit");
    std::fs::create_dir_all(temp.app_data_dir()).expect("create data dir");
    let storage = StorageService::open(&temp.db_path()).expect("open database");
    let state = AppState::ready(LibraryService::new(storage));
    let scheduler = state.scheduler();
    let (app, window) = build_app(state);
    let events = listen_events(&app);

    let big_body = format!("<p>{}</p>", "字".repeat(800_000));
    let mut paths = Vec::new();
    for index in 0..3 {
        let title = format!("大书{index}");
        let file = write_file(
            &temp.path,
            &format!("big-{index}.epub"),
            &build_epub(EpubOptions {
                title: Some(&title),
                body: &big_body,
                ..EpubOptions::default()
            }),
        );
        paths.push(file.to_string_lossy().into_owned());
    }

    let job_ids: Vec<String> = paths
        .iter()
        .map(|path| {
            let result = start_import(&window, std::slice::from_ref(path)).expect("job");
            result["jobId"].as_str().expect("job id").to_string()
        })
        .collect();
    assert_eq!(job_ids.len(), 3);

    let mut max_active = 0;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let active = scheduler.active_count();
        max_active = max_active.max(active);
        assert!(active <= 2, "任一时刻最多 2 个 job 执行，实际 {active}");
        if events.completed.lock().expect("lock").len() >= 3 {
            break;
        }
        assert!(Instant::now() < deadline, "等待 3 个 job 完成超时");
        std::thread::sleep(Duration::from_millis(1));
    }

    assert_eq!(max_active, 2, "应观察到两个 job 并发执行");
    let completed = events.completed.lock().expect("lock").clone();
    assert_eq!(completed.len(), 3);
    let mut completed_ids: Vec<String> = completed
        .iter()
        .map(|event| event["jobId"].as_str().expect("job id").to_string())
        .collect();
    completed_ids.sort();
    let mut expected = job_ids.clone();
    expected.sort();
    assert_eq!(completed_ids, expected, "所有 job 都应发出完成事件");

    for event in &completed {
        let items = items_of(event);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["status"], "imported");
        assert!(items[0].get("error").is_none());
    }
    assert_eq!(
        list_books(&window, None, "title", "asc")
            .expect("list")
            .as_array()
            .expect("array")
            .len(),
        3,
        "不应出现重复 active 记录"
    );
}

#[test]
fn initializes_data_directory_and_database() {
    let temp = TempDir::new("init");
    let state = initialize_state(&temp.app_data_dir());
    assert!(temp.app_data_dir().is_dir());
    assert!(temp.db_path().is_file());

    let (_app, window) = build_app(state);
    assert_eq!(
        list_books(&window, None, "title", "asc")
            .expect("list")
            .as_array()
            .expect("array")
            .len(),
        0
    );
}

#[test]
fn concurrent_queries_and_imports_on_thousand_books() {
    let temp = TempDir::new("perf");
    std::fs::create_dir_all(temp.app_data_dir()).expect("create data dir");
    let storage = StorageService::open(&temp.db_path()).expect("open database");
    for index in 0..1_000_u32 {
        storage
            .insert_new_book(NewBookRecord {
                id: format!("book-{index:04}"),
                fingerprint: format!("fp-{index:04}"),
                source_path: format!("/tmp/book-{index:04}.epub"),
                title: format!("书名 {index:04}"),
                author: format!("作者 {}", index % 20),
                title_search: format!("书名 {index:04}"),
                author_search: format!("作者 {}", index % 20),
                title_sort_key: index.to_be_bytes().to_vec(),
                author_sort_key: (index % 20).to_be_bytes().to_vec(),
                manifest_json: "{}".to_string(),
                manifest_version: 1,
                imported_at: format!("2026-01-{:02}T00:00:00Z", (index % 28) + 1),
            })
            .expect("insert");
    }
    let state = AppState::ready(LibraryService::new(storage));
    let (app, window) = build_app(state);
    let events = listen_events(&app);

    let file = write_file(
        &temp.path,
        "concurrent.epub",
        &build_epub(EpubOptions {
            title: Some("并发书"),
            ..EpubOptions::default()
        }),
    );
    let path = file.to_string_lossy().into_owned();

    let first = start_import(&window, std::slice::from_ref(&path)).expect("job 1");
    let second = start_import(&window, &[path]).expect("job 2");
    assert_ne!(first["jobId"], second["jobId"]);

    let mut timings = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let listed = list_books(&window, Some("书名"), "title", "asc").expect("query");
        timings.push(start.elapsed());
        assert_eq!(listed.as_array().expect("array").len(), 1_000);
    }
    timings.sort();
    let p95 = timings[9];
    eprintln!("debug build p95 query = {p95:?} (10 runs, max)");
    assert!(p95 <= Duration::from_millis(200), "p95 = {p95:?}");

    let completed = wait_for_completed(&events, 2);
    assert_eq!(completed.len(), 2);
    let mut statuses = Vec::new();
    let mut job_ids = Vec::new();
    for event in &completed {
        job_ids.push(event["jobId"].as_str().expect("job id").to_string());
        let items = items_of(event);
        assert_eq!(items.len(), 1);
        assert!(
            items[0].get("error").is_none(),
            "不应出现 E_STORAGE: {items:?}"
        );
        statuses.push(items[0]["status"].as_str().expect("status").to_string());
    }
    statuses.sort();
    assert_eq!(
        statuses,
        vec!["duplicate".to_string(), "imported".to_string()]
    );
    job_ids.sort();
    job_ids.dedup();
    assert_eq!(job_ids.len(), 2);

    let concurrent = list_books(&window, Some("并发书"), "title", "asc").expect("list");
    assert_eq!(
        concurrent.as_array().expect("array").len(),
        1,
        "同一指纹只能有一条 active 记录"
    );
}

#[test]
fn capabilities_and_dependencies_are_minimal() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let capabilities = std::fs::read_to_string(manifest_dir.join("capabilities/default.json"))
        .expect("read capabilities");
    let value: Value = serde_json::from_str(&capabilities).expect("parse capabilities");
    assert_eq!(value["permissions"], json!(["core:default"]));
    assert_eq!(value["windows"], json!(["main"]));

    let cargo = std::fs::read_to_string(manifest_dir.join("Cargo.toml")).expect("read Cargo.toml");
    for needle in [
        "tauri-plugin-",
        "plugin-http",
        "plugin-shell",
        "plugin-fs",
        "plugin-dialog",
        "reqwest",
        "hyper",
    ] {
        assert!(!cargo.contains(needle), "不应包含 {needle}");
    }

    let commands =
        std::fs::read_to_string(manifest_dir.join("src/commands.rs")).expect("read commands");
    for needle in ["reqwest", "hyper", "TcpStream", "std::net"] {
        assert!(!commands.contains(needle), "不应包含网络调用 {needle}");
    }
}
