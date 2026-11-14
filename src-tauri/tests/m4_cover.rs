//! M4D cover:// 协议验证：只读、严格 key、固定错误与 CSP。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use qurner_lib::protocol::cover::handle;

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "qurner-cover-test-{}-{name}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("创建临时目录");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn write_cover(directory: &Path, bytes: &[u8]) {
    fs::write(directory.join(format!("{KEY}.jpg")), bytes).expect("写封面");
}

fn header<'a>(response: &'a tauri::http::Response<Vec<u8>>, name: &str) -> Option<&'a str> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
}

#[test]
fn serves_get_and_head_with_immutable_jpeg_headers() {
    let temp = TempDir::new("get");
    write_cover(&temp.path, b"jpeg-bytes");
    let uri = format!("cover://{KEY}");

    let response = handle(Some(&temp.path), "GET", &uri);
    assert_eq!(response.status(), tauri::http::StatusCode::OK);
    assert_eq!(header(&response, "Content-Type"), Some("image/jpeg"));
    assert_eq!(header(&response, "X-Content-Type-Options"), Some("nosniff"));
    assert_eq!(
        header(&response, "Cache-Control"),
        Some("public, max-age=31536000, immutable")
    );
    assert_eq!(header(&response, "Content-Length"), Some("10"));
    assert_eq!(response.body(), b"jpeg-bytes");

    let head = handle(Some(&temp.path), "HEAD", &uri);
    assert_eq!(head.status(), tauri::http::StatusCode::OK);
    assert_eq!(header(&head, "Content-Type"), Some("image/jpeg"));
    assert_eq!(header(&head, "Content-Length"), Some("10"));
    assert!(head.body().is_empty());
}

#[test]
fn rejects_methods_and_invalid_keys_with_fixed_errors() {
    let temp = TempDir::new("invalid");
    write_cover(&temp.path, b"jpeg-bytes");

    let post = handle(Some(&temp.path), "POST", &format!("cover://{KEY}"));
    assert_eq!(post.status(), tauri::http::StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(header(&post, "Allow"), Some("GET, HEAD"));

    // 浏览器会把 host 形式规范成尾部单个斜杠，需兼容。
    let trailing = handle(Some(&temp.path), "GET", &format!("cover://{KEY}/"));
    assert_eq!(trailing.status(), tauri::http::StatusCode::OK);

    for uri in [
        "cover://",
        "cover:///",
        "cover://short",
        "cover://0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcd",
        "cover://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef//",
        "cover://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/extra",
        "cover://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef?x=1",
        "cover://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef#frag",
        "cover://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef:80",
        "cover://%30%31%32%33%34%35%36%37%38%39abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "epub://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "cover://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde",
    ] {
        let response = handle(Some(&temp.path), "GET", uri);
        assert_eq!(
            response.status(),
            tauri::http::StatusCode::NOT_FOUND,
            "应拒绝 {uri}"
        );
        assert_eq!(response.body(), b"Not Found");
    }
}

#[test]
fn rejects_missing_non_file_symlink_and_oversized_covers() {
    let temp = TempDir::new("missing");
    let query = format!("cover://{KEY}");

    assert_eq!(
        handle(Some(&temp.path), "GET", &query).status(),
        tauri::http::StatusCode::NOT_FOUND
    );

    fs::create_dir_all(temp.path.join(format!("{KEY}.jpg"))).expect("目录");
    assert_eq!(
        handle(Some(&temp.path), "GET", &query).status(),
        tauri::http::StatusCode::NOT_FOUND
    );
    fs::remove_dir_all(temp.path.join(format!("{KEY}.jpg"))).expect("移除目录");

    fs::write(temp.path.join("target.jpg"), b"x").expect("目标");
    std::os::unix::fs::symlink(
        temp.path.join("target.jpg"),
        temp.path.join(format!("{KEY}.jpg")),
    )
    .expect("符号链接");
    assert_eq!(
        handle(Some(&temp.path), "GET", &query).status(),
        tauri::http::StatusCode::NOT_FOUND
    );
    fs::remove_file(temp.path.join(format!("{KEY}.jpg"))).expect("移除链接");

    let oversized = vec![0_u8; 5 * 1024 * 1024 + 1];
    write_cover(&temp.path, &oversized);
    assert_eq!(
        handle(Some(&temp.path), "GET", &query).status(),
        tauri::http::StatusCode::NOT_FOUND
    );

    assert_eq!(
        handle(None, "GET", &query).status(),
        tauri::http::StatusCode::NOT_FOUND
    );
}

#[test]
fn csp_allows_cover_and_capability_stays_core_default() {
    let config = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json"))
        .expect("read tauri.conf.json");
    let value: serde_json::Value = serde_json::from_str(&config).expect("tauri.conf.json");
    let csp = value["app"]["security"]["csp"]
        .as_str()
        .expect("csp string");
    let img_src = csp
        .split(';')
        .find(|directive| directive.trim_start().starts_with("img-src"))
        .expect("img-src");
    assert!(img_src.contains("cover:"));
    assert!(!img_src.contains("http") && !img_src.contains("https"));

    let capabilities = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/capabilities/default.json"
    ))
    .expect("read capabilities");
    let value: serde_json::Value = serde_json::from_str(&capabilities).expect("json");
    assert_eq!(value["permissions"], serde_json::json!(["core:default"]));
    assert!(!capabilities.contains("dialog:") && !capabilities.contains("fs:"));

    let lib = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read lib.rs");
    assert!(lib.contains("protocol::cover::register"));
    let cover = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/protocol/cover.rs"
    ))
    .expect("read cover protocol");
    assert!(cover.contains("register_asynchronous_uri_scheme_protocol"));
    assert!(cover.contains("\"cover\""));
    assert!(!cover.contains("reqwest") && !cover.contains("TcpStream"));
}
