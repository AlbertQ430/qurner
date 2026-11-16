//! M5D 设置命令契约：继承、全局/单书更新、清除与错误。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use qurner_lib::state::AppState;
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
            "qurner-m5d-{}-{name}-{}",
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
    <dc:title>设置命令测试书</dc:title>
    <dc:creator>作者</dc:creator>
    <dc:identifier id="id">urn:uuid:m5d-test</dc:identifier>
    <meta property="dcterms:modified">2026-11-16T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="c1"/></spine>
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
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>正文</p></body></html>".as_bytes(),
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
    epub_path: PathBuf,
}

fn setup_app(name: &str) -> App {
    let temp = TempDir::new(name);
    let epub_path = temp.path.join("book.epub");
    std::fs::write(&epub_path, build_epub()).expect("epub");
    std::fs::create_dir_all(temp.app_data_dir()).expect("data");
    let state: AppState = qurner_lib::state::initialize_state(&temp.app_data_dir());
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            qurner_lib::commands::start_import,
            qurner_lib::commands::list_books,
            qurner_lib::commands::soft_remove_book,
            qurner_lib::commands::get_effective_settings,
            qurner_lib::commands::update_app_settings,
            qurner_lib::commands::update_book_settings,
            qurner_lib::commands::reset_book_settings,
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

fn settings(mode: &str, font_scale: f64, theme: &str) -> Value {
    json!({ "mode": mode, "fontScale": font_scale, "theme": theme })
}

#[test]
fn settings_commands_follow_inheritance_and_reset() {
    let app = setup_app("inherit");
    let book_id = import_book(&app);

    let defaults = invoke(
        &app.window,
        "getEffectiveSettings",
        json!({ "bookId": book_id }),
    )
    .expect("defaults");
    assert_eq!(defaults, settings("paged", 1.0, "day"));

    let global = invoke(
        &app.window,
        "updateAppSettings",
        json!({ "patch": { "fontScale": 1.5, "theme": "night" } }),
    )
    .expect("global");
    assert_eq!(global, settings("paged", 1.5, "night"));

    let book = invoke(
        &app.window,
        "updateBookSettings",
        json!({
            "input": {
                "bookId": book_id,
                "patch": { "mode": "scrolling", "fontScale": 2.0 }
            }
        }),
    )
    .expect("book");
    assert_eq!(book, settings("scrolling", 2.0, "night"));

    let merged = invoke(
        &app.window,
        "getEffectiveSettings",
        json!({ "bookId": book_id }),
    )
    .expect("merged");
    assert_eq!(merged, settings("scrolling", 2.0, "night"));

    let global2 = invoke(
        &app.window,
        "updateAppSettings",
        json!({ "patch": { "theme": "day" } }),
    )
    .expect("global2");
    assert_eq!(global2, settings("paged", 1.5, "day"));
    let merged2 = invoke(
        &app.window,
        "getEffectiveSettings",
        json!({ "bookId": book_id }),
    )
    .expect("merged2");
    assert_eq!(merged2, settings("scrolling", 2.0, "day"));

    let cleared = invoke(
        &app.window,
        "updateBookSettings",
        json!({
            "input": {
                "bookId": book_id,
                "patch": { "fontScale": null, "theme": null }
            }
        }),
    )
    .expect("cleared");
    assert_eq!(cleared, settings("scrolling", 1.5, "day"));

    let reset = invoke(
        &app.window,
        "resetBookSettings",
        json!({ "bookId": book_id }),
    )
    .expect("reset");
    assert_eq!(reset, settings("paged", 1.5, "day"));
}

#[test]
fn settings_commands_reject_invalid_input_and_removed_books() {
    let app = setup_app("invalid");
    let book_id = import_book(&app);

    let empty = invoke(
        &app.window,
        "updateBookSettings",
        json!({ "input": { "bookId": book_id, "patch": {} } }),
    )
    .expect_err("empty patch");
    assert_eq!(error_code(&empty), "E_INVALID_INPUT");

    let null_global = invoke(
        &app.window,
        "updateAppSettings",
        json!({ "patch": { "mode": null } }),
    )
    .expect_err("null global");
    assert_eq!(error_code(&null_global), "E_INVALID_INPUT");

    let bad_scale = invoke(
        &app.window,
        "updateAppSettings",
        json!({ "patch": { "fontScale": 4.0 } }),
    )
    .expect_err("bad scale");
    assert_eq!(error_code(&bad_scale), "E_INVALID_INPUT");

    let unknown = invoke(
        &app.window,
        "updateAppSettings",
        json!({ "patch": { "mode": "paged", "extra": true } }),
    )
    .expect_err("unknown");
    assert_eq!(error_code(&unknown), "E_INVALID_INPUT");

    let missing = invoke(
        &app.window,
        "getEffectiveSettings",
        json!({ "bookId": "missing" }),
    )
    .expect_err("missing");
    assert_eq!(error_code(&missing), "E_BOOK_NOT_FOUND");

    invoke(&app.window, "softRemoveBook", json!({ "bookId": book_id })).expect("remove");
    let removed = invoke(
        &app.window,
        "getEffectiveSettings",
        json!({ "bookId": book_id }),
    )
    .expect_err("removed");
    assert_eq!(error_code(&removed), "E_BOOK_NOT_FOUND");
}
