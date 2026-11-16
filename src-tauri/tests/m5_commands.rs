//! M5A openBook/closeBook 契约：会话、frameUrl、TOC、初始位置与关闭失效。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use qurner_lib::protocol::handle as protocol_handle;
use qurner_lib::session::SessionRegistry;
use qurner_lib::state::{AppState, initialize_state_with_sessions};
use serde_json::{Value, json};
use tauri::test::{MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "qurner-m5a-{}-{name}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("temp");
        Self { path }
    }

    fn app_data_dir(&self) -> PathBuf {
        self.path.join("data")
    }

    fn db_path(&self) -> PathBuf {
        self.app_data_dir().join("reader.sqlite3")
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

fn build_epub() -> Vec<u8> {
    let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>M5A 测试书</dc:title>
    <dc:creator>作者</dc:creator>
    <dc:identifier id="id">urn:uuid:m5a-test</dc:identifier>
    <meta property="dcterms:modified">2026-11-15T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="c1"/><itemref idref="c1"/></spine>
</package>
"#;
    build_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", CONTAINER.as_bytes()),
        ("OEBPS/content.opf", opf.as_bytes()),
        (
            "OEBPS/nav.xhtml",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><nav epub:type=\"toc\"><ol><li><a href=\"chapter1.xhtml\">第一章</a></li></ol></nav></body></html>".as_bytes(),
        ),
        (
            "OEBPS/chapter1.xhtml",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>M5A 正文</p></body></html>".as_bytes(),
        ),
    ])
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
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & (crc & 1).wrapping_neg());
        }
    }
    !crc
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

struct App {
    _temp: TempDir,
    _app: tauri::App<MockRuntime>,
    window: WebviewWindow<MockRuntime>,
    sessions: Arc<SessionRegistry>,
    epub_path: PathBuf,
}

fn setup_app(name: &str) -> App {
    let temp = TempDir::new(name);
    let epub_path = temp.path.join("book.epub");
    std::fs::write(&epub_path, build_epub()).expect("epub");
    std::fs::create_dir_all(temp.app_data_dir()).expect("data dir");
    let sessions = Arc::new(SessionRegistry::new());
    let state: AppState =
        initialize_state_with_sessions(&temp.app_data_dir(), Arc::clone(&sessions));
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            qurner_lib::commands::start_import,
            qurner_lib::commands::list_books,
            qurner_lib::commands::soft_remove_book,
            qurner_lib::commands::open_book,
            qurner_lib::commands::close_book,
        ])
        .build(mock_context(noop_assets()))
        .expect("build app");
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("window");
    App {
        _temp: temp,
        _app: app,
        window,
        sessions,
        epub_path,
    }
}

fn import_book(app: &App) -> Value {
    let path = app.epub_path.to_string_lossy().into_owned();
    invoke(&app.window, "startImport", json!({ "paths": [path] })).expect("start import");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let books = invoke(
            &app.window,
            "listBooks",
            json!({
                "input": { "sort": "importedAt", "direction": "desc" }
            }),
        )
        .expect("list books");
        if let Some(book) = books.as_array().and_then(|items| items.first()) {
            return book.clone();
        }
        assert!(Instant::now() < deadline, "导入超时");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn open(app: &App, book_id: &str) -> Value {
    invoke(&app.window, "openBook", json!({ "bookId": book_id })).expect("open book")
}

#[test]
fn opens_book_with_session_frame_url_toc_and_defaults() {
    let app = setup_app("open");
    let before = std::fs::read(&app.epub_path).expect("read before");
    let book = import_book(&app);
    let book_id = book["id"].as_str().expect("id");

    let session = open(&app, book_id);
    let keys: Vec<&str> = session
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    for forbidden in ["sourcePath", "source_path", "token", "sessionToken"] {
        assert!(!keys.contains(&forbidden), "响应不得包含 {forbidden}");
    }
    let session_id = session["sessionId"].as_str().expect("sessionId");
    let parsed = uuid::Uuid::parse_str(session_id).expect("uuid");
    assert_eq!(parsed.get_version(), Some(uuid::Version::Random));
    assert_eq!(session["bookId"], book_id);
    assert_eq!(session["title"], "M5A 测试书");
    assert!(session["initialLocator"].is_null());
    assert_eq!(session["initialProgression"], 0.0);
    assert_eq!(session["settings"]["mode"], "paged");
    assert_eq!(session["settings"]["fontScale"], 1.0);
    assert_eq!(session["settings"]["theme"], "day");
    let spine = session["spine"].as_array().expect("spine");
    assert_eq!(spine.len(), 2, "重复 itemref 必须保留为两个逻辑 spine 项");
    assert_eq!(spine[0]["index"], 0);
    assert_eq!(spine[1]["index"], 1);
    assert_eq!(spine[0]["href"], "OEBPS/chapter1.xhtml");
    assert_eq!(spine[1]["href"], "OEBPS/chapter1.xhtml");
    assert_eq!(session["toc"][0]["label"], "第一章");
    assert_eq!(session["toc"][0]["spineIndex"], 0);

    let frame_url = session["frameUrl"].as_str().expect("frameUrl");
    assert!(frame_url.starts_with("epub://"));
    let response = protocol_handle(&app.sessions, "GET", frame_url);
    assert_eq!(response.status, 200);
    let body = String::from_utf8_lossy(&response.body);
    assert!(body.contains("M5A 正文"));
    assert!(body.contains("reader-engine.js"));

    let after = std::fs::read(&app.epub_path).expect("read after");
    assert_eq!(before, after);

    let storage = storage_core::StorageService::open(&app._temp.db_path()).expect("db");
    let record = storage
        .open_book_record(book_id)
        .expect("query")
        .expect("record");
    assert!(record.last_opened_at.is_some(), "应更新 last_opened_at");
}

#[test]
fn close_invalidates_frame_url_and_rejects_repeat() {
    let app = setup_app("close");
    let book = import_book(&app);
    let book_id = book["id"].as_str().expect("id");
    let session = open(&app, book_id);
    let session_id = session["sessionId"].as_str().expect("sessionId");
    let frame_url = session["frameUrl"].as_str().expect("frameUrl");
    assert_eq!(protocol_handle(&app.sessions, "GET", frame_url).status, 200);

    invoke(&app.window, "closeBook", json!({ "sessionId": session_id })).expect("close");
    assert_eq!(protocol_handle(&app.sessions, "GET", frame_url).status, 404);

    let again = invoke(&app.window, "closeBook", json!({ "sessionId": session_id }))
        .expect_err("repeat close");
    assert_eq!(error_code(&again), "E_SESSION_INVALID");
}

#[test]
fn sessions_are_independent() {
    let app = setup_app("independent");
    let book = import_book(&app);
    let book_id = book["id"].as_str().expect("id");
    let first = open(&app, book_id);
    let second = open(&app, book_id);
    assert_ne!(first["sessionId"], second["sessionId"]);
    let first_frame = first["frameUrl"].as_str().expect("frame");
    let second_frame = second["frameUrl"].as_str().expect("frame");

    invoke(
        &app.window,
        "closeBook",
        json!({ "sessionId": first["sessionId"] }),
    )
    .expect("close first");
    assert_eq!(
        protocol_handle(&app.sessions, "GET", first_frame).status,
        404
    );
    assert_eq!(
        protocol_handle(&app.sessions, "GET", second_frame).status,
        200
    );
}

#[test]
fn rejects_invalid_input_and_missing_books() {
    let app = setup_app("invalid");
    let blank = invoke(&app.window, "openBook", json!({ "bookId": "   " })).expect_err("blank");
    assert_eq!(error_code(&blank), "E_INVALID_INPUT");
    let missing =
        invoke(&app.window, "openBook", json!({ "bookId": "missing" })).expect_err("missing");
    assert_eq!(error_code(&missing), "E_BOOK_NOT_FOUND");
    let bad_session =
        invoke(&app.window, "closeBook", json!({ "sessionId": "no" })).expect_err("bad session");
    assert_eq!(error_code(&bad_session), "E_INVALID_INPUT");
    let unknown = invoke(
        &app.window,
        "closeBook",
        json!({ "sessionId": "123e4567-e89b-42d3-a456-426614174000" }),
    )
    .expect_err("unknown session");
    assert_eq!(error_code(&unknown), "E_SESSION_INVALID");
}

#[test]
fn removed_books_cannot_be_opened() {
    let app = setup_app("removed");
    let book = import_book(&app);
    let book_id = book["id"].as_str().expect("id").to_string();
    invoke(&app.window, "softRemoveBook", json!({ "bookId": book_id })).expect("remove");
    let error = invoke(&app.window, "openBook", json!({ "bookId": book_id })).expect_err("open");
    assert_eq!(error_code(&error), "E_BOOK_NOT_FOUND");
}

#[test]
fn missing_or_changed_source_returns_stable_errors() {
    let missing = setup_app("missing-source");
    let book = import_book(&missing);
    let book_id = book["id"].as_str().expect("id").to_string();
    std::fs::remove_file(&missing.epub_path).expect("remove file");
    let error =
        invoke(&missing.window, "openBook", json!({ "bookId": book_id })).expect_err("open");
    assert_eq!(error_code(&error), "E_BOOK_UNAVAILABLE");

    let changed = setup_app("changed-source");
    let book = import_book(&changed);
    let book_id = book["id"].as_str().expect("id").to_string();
    let mut bytes = std::fs::read(&changed.epub_path).expect("read");
    bytes.extend_from_slice(b"changed");
    std::fs::write(&changed.epub_path, bytes).expect("write");
    let error =
        invoke(&changed.window, "openBook", json!({ "bookId": book_id })).expect_err("open");
    assert_eq!(error_code(&error), "E_SOURCE_CHANGED");
}
