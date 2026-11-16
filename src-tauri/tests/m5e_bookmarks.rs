//! M5E-B 书签命令：会话绑定、列表顺序、删除隔离与不变量。

use std::path::PathBuf;
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
            "qurner-m5eb-{}-{name}-{}",
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

fn build_epub(title: &str) -> Vec<u8> {
    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>{title}</dc:title>
    <dc:creator>作者</dc:creator>
    <dc:identifier id="id">urn:uuid:m5eb-{title}</dc:identifier>
    <meta property="dcterms:modified">2026-11-18T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="c1"/><itemref idref="c2"/></spine>
</package>
"#
    );
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
    epub_one: PathBuf,
    epub_two: PathBuf,
}

fn setup_app(name: &str) -> App {
    let temp = TempDir::new(name);
    let epub_one = temp.path.join("one.epub");
    let epub_two = temp.path.join("two.epub");
    std::fs::write(&epub_one, build_epub("书一")).expect("epub one");
    std::fs::write(&epub_two, build_epub("书二")).expect("epub two");
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
            qurner_lib::commands::list_bookmarks,
            qurner_lib::commands::create_bookmark,
            qurner_lib::commands::delete_bookmark,
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
        epub_one,
        epub_two,
    }
}

fn import_books(app: &App) -> (String, String) {
    for path in [&app.epub_one, &app.epub_two] {
        let path = path.to_string_lossy().into_owned();
        invoke(&app.window, "startImport", json!({ "paths": [path] })).expect("import");
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let books = invoke(
            &app.window,
            "listBooks",
            json!({ "input": { "sort": "importedAt", "direction": "desc" } }),
        )
        .expect("list books");
        if let Some(items) = books.as_array()
            && items.len() == 2
        {
            let mut ids: Vec<String> = items
                .iter()
                .map(|item| item["id"].as_str().expect("id").to_string())
                .collect();
            ids.sort();
            return (ids[0].clone(), ids[1].clone());
        }
        assert!(Instant::now() < deadline, "导入超时");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn open_book(app: &App, book_id: &str) -> Value {
    invoke(&app.window, "openBook", json!({ "bookId": book_id })).expect("open")
}

fn locator(spine_index: u32, spine_href: &str, offset: u64, exact: &str) -> Value {
    json!({
        "version": 1,
        "spineIndex": spine_index,
        "spineHref": spine_href,
        "textOffset": offset,
        "intraSpineProgression": 0.2,
        "quote": { "exact": exact, "prefix": "", "suffix": "" }
    })
}

fn create(app: &App, session_id: &str, locator: &Value, excerpt: Option<&str>) -> Value {
    let mut body = json!({ "sessionId": session_id, "locator": locator });
    if let Some(excerpt) = excerpt {
        body["excerpt"] = json!(excerpt);
    }
    invoke(&app.window, "createBookmark", body).expect("create")
}

fn list(app: &App, book_id: &str) -> Vec<Value> {
    invoke(&app.window, "listBookmarks", json!({ "bookId": book_id }))
        .expect("list")
        .as_array()
        .expect("array")
        .clone()
}

fn db(app: &App) -> rusqlite::Connection {
    let connection = rusqlite::Connection::open(app.temp.db_path()).expect("sqlite");
    connection
        .execute_batch("PRAGMA busy_timeout = 5000;")
        .expect("busy");
    connection
}

#[test]
fn creates_lists_and_deletes_bookmarks_per_book() {
    let app = setup_app("crud");
    let (book_a, book_b) = import_books(&app);
    let session_a = open_book(&app, &book_a);
    let session_id = session_a["sessionId"]
        .as_str()
        .expect("session")
        .to_string();
    assert_eq!(list(&app, &book_a), Vec::<Value>::new());

    let first = create(
        &app,
        &session_id,
        &locator(0, "OEBPS/chapter1.xhtml", 3, "摘录一"),
        None,
    );
    assert_eq!(first["bookId"], book_a);
    assert_eq!(first["excerpt"], "摘录一");
    assert!(first["locator"]["spineHref"].as_str().unwrap() == "OEBPS/chapter1.xhtml");
    let first_id = first["id"].as_str().expect("id").to_string();

    let second = create(
        &app,
        &session_id,
        &locator(1, "OEBPS/chapter2.xhtml", 5, "摘录二"),
        Some("显式摘录"),
    );
    assert_eq!(second["excerpt"], "显式摘录");

    let long_excerpt = "字".repeat(200);
    let truncated = create(
        &app,
        &session_id,
        &locator(0, "OEBPS/chapter1.xhtml", 8, "长摘录"),
        Some(&long_excerpt),
    );
    let excerpt = truncated["excerpt"].as_str().expect("excerpt");
    assert_eq!(excerpt.chars().count(), 128);

    let items = list(&app, &book_a);
    assert_eq!(items.len(), 3);
    for item in &items {
        assert_eq!(item["bookId"], book_a);
        assert!(item.get("sourcePath").is_none());
        assert!(item.get("token").is_none());
    }

    invoke(
        &app.window,
        "deleteBookmark",
        json!({ "bookId": book_a, "bookmarkId": first_id }),
    )
    .expect("delete");
    let remaining = list(&app, &book_a);
    assert_eq!(remaining.len(), 2);
    assert!(remaining.iter().all(|item| item["id"] != first_id.as_str()));

    // 跨书删除：用书 B 的 bookId 删除书 A 的书签必须失败且不影响书 A。
    let cross = invoke(
        &app.window,
        "deleteBookmark",
        json!({ "bookId": book_b, "bookmarkId": remaining[0]["id"] }),
    )
    .expect_err("cross delete");
    assert_eq!(error_code(&cross), "E_INVALID_INPUT");
    assert_eq!(list(&app, &book_a).len(), 2);
    assert_eq!(list(&app, &book_b).len(), 0);

    let missing = invoke(
        &app.window,
        "deleteBookmark",
        json!({ "bookId": book_a, "bookmarkId": "123e4567-e89b-42d3-a456-426614174000" }),
    )
    .expect_err("missing bookmark");
    assert_eq!(error_code(&missing), "E_INVALID_INPUT");

    let connection = db(&app);
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 2);
    let book_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM books", [], |row| row.get(0))
        .expect("books");
    assert_eq!(book_count, 2);
}

#[test]
fn rejects_invalid_sessions_and_locators() {
    let app = setup_app("invalid");
    let (book_a, _) = import_books(&app);
    let session = open_book(&app, &book_a);
    let session_id = session["sessionId"].as_str().expect("session").to_string();

    let unknown = invoke(
        &app.window,
        "createBookmark",
        json!({
            "sessionId": "123e4567-e89b-42d3-a456-426614174000",
            "locator": locator(0, "OEBPS/chapter1.xhtml", 0, "x"),
        }),
    )
    .expect_err("unknown session");
    assert_eq!(error_code(&unknown), "E_SESSION_INVALID");

    let mismatch = invoke(
        &app.window,
        "createBookmark",
        json!({
            "sessionId": session_id,
            "locator": locator(0, "OEBPS/chapter2.xhtml", 0, "x"),
        }),
    )
    .expect_err("spine mismatch");
    assert_eq!(error_code(&mismatch), "E_INVALID_INPUT");

    let too_long = locator(0, "OEBPS/chapter1.xhtml", 0, &"x".repeat(129));
    let quote = invoke(
        &app.window,
        "createBookmark",
        json!({ "sessionId": session_id, "locator": too_long }),
    )
    .expect_err("quote");
    assert_eq!(error_code(&quote), "E_INVALID_INPUT");

    let missing = invoke(
        &app.window,
        "createBookmark",
        json!({ "sessionId": session_id }),
    )
    .expect_err("missing locator");
    assert_eq!(error_code(&missing), "E_INVALID_INPUT");

    invoke(&app.window, "closeBook", json!({ "sessionId": session_id })).expect("close");
    let closed = invoke(
        &app.window,
        "createBookmark",
        json!({
            "sessionId": session_id,
            "locator": locator(0, "OEBPS/chapter1.xhtml", 0, "x"),
        }),
    )
    .expect_err("closed");
    assert_eq!(error_code(&closed), "E_SESSION_INVALID");

    let connection = db(&app);
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 0);
}
