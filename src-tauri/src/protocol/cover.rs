//! 只读 `cover://` 协议：`cover://<64 位小写十六进制>` → `covers/<key>.jpg`。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const MAX_COVER_BYTES: u64 = 5 * 1024 * 1024;
const CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
const NOSNIFF: &str = "nosniff";

/// 协议共享的封面缓存目录；在 setup 中设置。
pub type CoverDirectory = Arc<Mutex<Option<PathBuf>>>;

/// 创建共享目录句柄。
#[must_use]
pub fn directory_handle() -> CoverDirectory {
    Arc::new(Mutex::new(None))
}

/// 设置缓存目录。
pub fn set_directory(directory: &CoverDirectory, path: PathBuf) {
    if let Ok(mut slot) = directory.lock() {
        *slot = Some(path);
    }
}

/// 注册异步 `cover` 协议。
pub fn register<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    directory: CoverDirectory,
) -> tauri::Builder<R> {
    builder.register_asynchronous_uri_scheme_protocol(
        "cover",
        move |_context, request, responder| {
            let directory = directory.lock().ok().and_then(|guard| guard.clone());
            let method = request.method().as_str().to_string();
            let uri = request.uri().to_string();
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(handle(directory.as_deref(), &method, &uri));
            });
        },
    )
}

/// 纯函数响应处理，便于协议单元测试。
#[must_use]
pub fn handle(directory: Option<&Path>, method: &str, uri: &str) -> tauri::http::Response<Vec<u8>> {
    if method != "GET" && method != "HEAD" {
        return response(
            tauri::http::StatusCode::METHOD_NOT_ALLOWED,
            &[("Allow", "GET, HEAD")],
            Some(b"Method Not Allowed".to_vec()),
            None,
        );
    }
    let Some(key) = parse_key(uri) else {
        return not_found(method == "HEAD");
    };
    let Some(directory) = directory else {
        return not_found(method == "HEAD");
    };
    let path = directory.join(format!("{key}.jpg"));
    let Ok(metadata) = std::fs::symlink_metadata(&path) else {
        return not_found(method == "HEAD");
    };
    if !metadata.file_type().is_file() || metadata.len() > MAX_COVER_BYTES {
        return not_found(method == "HEAD");
    }
    let Ok(bytes) = std::fs::read(&path) else {
        return not_found(method == "HEAD");
    };
    response(
        tauri::http::StatusCode::OK,
        &[
            ("Content-Type", "image/jpeg"),
            ("X-Content-Type-Options", NOSNIFF),
            ("Cache-Control", CACHE_CONTROL),
        ],
        if method == "HEAD" { None } else { Some(bytes) },
        Some(metadata.len() as usize),
    )
}

/// 严格解析 `cover://<key>`（允许一个尾部斜杠）；拒绝端口、额外路径、查询、片段和编码。
fn parse_key(uri: &str) -> Option<String> {
    let rest = uri
        .strip_prefix("cover://")
        .or_else(|| uri.strip_prefix("COVER://"))?;
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    if rest.len() != 64
        || !rest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    Some(rest.to_string())
}

fn not_found(head: bool) -> tauri::http::Response<Vec<u8>> {
    response(
        tauri::http::StatusCode::NOT_FOUND,
        &[],
        if head {
            None
        } else {
            Some(b"Not Found".to_vec())
        },
        None,
    )
}

fn response(
    status: tauri::http::StatusCode,
    headers: &[(&str, &str)],
    body: Option<Vec<u8>>,
    content_length: Option<usize>,
) -> tauri::http::Response<Vec<u8>> {
    let payload = body.unwrap_or_default();
    let mut builder = tauri::http::Response::builder().status(status);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    if let Some(length) = content_length {
        builder = builder.header("Content-Length", length.to_string());
    }
    builder.body(payload).unwrap_or_else(|_| {
        let mut fallback = tauri::http::Response::new(Vec::new());
        *fallback.status_mut() = tauri::http::StatusCode::INTERNAL_SERVER_ERROR;
        fallback
    })
}
