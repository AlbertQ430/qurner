//! M5E-A 阅读进度保存命令：会话绑定、Locator 校验与 reading_states 写入。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

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
            "qurner-m5e-{}-{name}-{}",
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
    <dc:title>进度测试书</dc:title>
    <dc:creator>作者</dc:creator>
    <dc:identifier id="id">urn:uuid:m5e-test</dc:identifier>
    <meta property="dcterms:modified">2026-11-17T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="c1"/><itemref idref="c2"/></spine>
</package>
"#;
    build_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", CONTAINER.as_bytes()),
        ("OEBPS/content.opf", opf.as_bytes()),
        (
            "OEBPS/nav.xhtml",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><nav epub:type=\"toc\"><ol><li><a href=\"chapter1.xhtml\">第一章</a></li><li><a href=\"chapter2.xhtml\">第二章</a></li></ol></nav></body></html>".as_bytes(),
        ),
        (
            "OEBPS/chapter1.xhtml",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>正文一</p></body></html>".as_bytes(),
        ),
        (
            "OEBPS/chapter2.xhtml",
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>正文二</p></body></html>".as_bytes(),
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
    temp: TempDir,
    _app: tauri::App<MockRuntime>,
    window: WebviewWindow<MockRuntime>,
    epub_path: PathBuf,
}

fn setup_app(name: &str) -> App {
    let temp = TempDir::new(name);
    let epub_path = temp.path.join("book.epub");
    std::fs::write(&epub_path, build_epub()).expect("epub");
    std::fs::create_dir_all(temp.app_data_dir()).expect("data");
    let sessions = Arc::new(SessionRegistry::new());
    let state: AppState =
        initialize_state_with_sessions(&temp.app_data_dir(), Arc::clone(&sessions));
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            qurner_lib::commands::start_import,
            qurner_lib::commands::list_books,
            qurner_lib::commands::open_book,
            qurner_lib::commands::close_book,
            qurner_lib::commands::save_reading_state,
        ])
        .build(mock_context(noop_assets()))
        .expect("build app");
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("window");
    App {
        temp,
        _app: app,
        window,
        epub_path,
    }
}

fn import_book(app: &App) -> String {
    let path = app.epub_path.to_string_lossy().into_owned();
    invoke(&app.window, "startImport", json!({ "paths": [path] })).expect("start import");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let books = invoke(
            &app.window,
            "listBooks",
            json!({ "input": { "sort": "importedAt", "direction": "desc" } }),
        )
        .expect("list books");
        if let Some(book) = books.as_array().and_then(|items| items.first()) {
            return book["id"].as_str().expect("id").to_string();
        }
        assert!(Instant::now() < deadline, "导入超时");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn open_book(app: &App, book_id: &str) -> Value {
    invoke(&app.window, "openBook", json!({ "bookId": book_id })).expect("open")
}

fn locator(spine_index: u32, spine_href: &str, text_offset: u64, intra: f64) -> Value {
    json!({
        "version": 1,
        "spineIndex": spine_index,
        "spineHref": spine_href,
        "textOffset": text_offset,
        "intraSpineProgression": intra,
        "quote": { "exact": "正文", "prefix": "", "suffix": "" }
    })
}

fn save(app: &App, session_id: &str, locator: &Value) -> Result<Value, Value> {
    invoke(
        &app.window,
        "saveReadingState",
        json!({ "sessionId": session_id, "locator": locator }),
    )
}

fn db(app: &App) -> rusqlite::Connection {
    let connection = rusqlite::Connection::open(app.temp.db_path()).expect("sqlite");
    connection
        .execute_batch("PRAGMA busy_timeout = 5000;")
        .expect("busy");
    connection
}

#[test]
fn saves_reading_state_and_upserts_single_row() {
    let app = setup_app("save");
    let book_id = import_book(&app);
    let before = std::fs::read(&app.epub_path).expect("epub before");
    let session = open_book(&app, &book_id);
    let session_id = session["sessionId"].as_str().expect("session").to_string();
    let spine = session["spine"].as_array().expect("spine");
    let weights = session["chapterWeights"].as_array().expect("weights");
    assert_eq!(weights.len(), spine.len());
    assert_eq!(spine[0]["href"], "OEBPS/chapter1.xhtml");

    let first = save(
        &app,
        &session_id,
        &locator(0, "OEBPS/chapter1.xhtml", 4, 0.5),
    )
    .expect("save");
    let progression = first["globalProgression"].as_f64().expect("progress");
    assert!((0.0..=1.0).contains(&progression));
    assert!(progression > 0.0);
    assert!(first["savedAt"].as_str().expect("savedAt").ends_with('Z'));

    let connection = db(&app);
    let (json, version, stored, updated): (String, u32, f64, String) = connection
        .query_row(
            "SELECT locator_json, locator_version, global_progression, updated_at
             FROM reading_states WHERE book_id=?1",
            [&book_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("reading state");
    assert_eq!(version, 1);
    assert!((stored - progression).abs() < 1e-9);
    assert!(updated.ends_with('Z'));
    let parsed: Value = serde_json::from_str(&json).expect("locator json");
    assert_eq!(parsed["spineHref"], "OEBPS/chapter1.xhtml");
    assert_eq!(parsed["spineIndex"], 0);

    let second = save(
        &app,
        &session_id,
        &locator(1, "OEBPS/chapter2.xhtml", 2, 0.9),
    )
    .expect("second save");
    let second_progress = second["globalProgression"].as_f64().expect("progress");
    let (count, stored2): (i64, f64) = connection
        .query_row(
            "SELECT COUNT(*), MAX(global_progression) FROM reading_states WHERE book_id=?1",
            [&book_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("count");
    assert_eq!(count, 1, "UPSERT 只保留一行");
    assert!((stored2 - second_progress).abs() < 1e-9);
    let after = std::fs::read(&app.epub_path).expect("epub after");
    assert_eq!(before, after);
    let book_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM books", [], |row| row.get(0))
        .expect("books");
    assert_eq!(book_count, 1);
    let bookmark_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))
        .expect("bookmarks");
    assert_eq!(bookmark_count, 0);
}

#[test]
fn rejects_invalid_sessions_and_locators_without_writing() {
    let app = setup_app("invalid");
    let book_id = import_book(&app);
    let session = open_book(&app, &book_id);
    let session_id = session["sessionId"].as_str().expect("session").to_string();

    let unknown = save(
        &app,
        "123e4567-e89b-42d3-a456-426614174000",
        &locator(0, "OEBPS/chapter1.xhtml", 0, 0.1),
    )
    .expect_err("unknown session");
    assert_eq!(error_code(&unknown), "E_SESSION_INVALID");

    let mismatch = save(
        &app,
        &session_id,
        &locator(0, "OEBPS/chapter2.xhtml", 0, 0.1),
    )
    .expect_err("spine mismatch");
    assert_eq!(error_code(&mismatch), "E_INVALID_INPUT");

    let out_of_range = save(
        &app,
        &session_id,
        &locator(9, "OEBPS/chapter1.xhtml", 0, 0.1),
    )
    .expect_err("out of range");
    assert_eq!(error_code(&out_of_range), "E_INVALID_INPUT");

    let mut too_long = locator(0, "OEBPS/chapter1.xhtml", 0, 0.1);
    too_long["quote"]["exact"] = json!("x".repeat(129));
    let quote = save(&app, &session_id, &too_long).expect_err("quote");
    assert_eq!(error_code(&quote), "E_INVALID_INPUT");

    let missing = invoke(
        &app.window,
        "saveReadingState",
        json!({ "sessionId": session_id }),
    )
    .expect_err("missing locator");
    assert_eq!(error_code(&missing), "E_INVALID_INPUT");

    let connection = db(&app);
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM reading_states", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 0, "失败不得写入 reading_states");

    invoke(&app.window, "closeBook", json!({ "sessionId": session_id })).expect("close");
    let closed = save(
        &app,
        &session_id,
        &locator(0, "OEBPS/chapter1.xhtml", 0, 0.1),
    )
    .expect_err("closed session");
    assert_eq!(error_code(&closed), "E_SESSION_INVALID");
}

#[allow(dead_code)]
fn _assert_path(_path: &Path) {}
