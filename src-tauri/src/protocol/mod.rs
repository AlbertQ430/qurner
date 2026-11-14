//! `epub://` 自定义协议。
//!
//! 纯函数 [`handle`] 负责解析请求、会话查找、manifest 白名单校验与资源读取；
//! Tauri 注册层只负责转换请求和响应。

use std::io::Read;
use std::sync::Arc;

use epub_core::{MAX_RESOURCE_BYTES, ResourcePath, ResourceReadError, read_resource};

pub mod cover;
mod html;

use crate::session::SessionRegistry;

/// 内置阅读脚本路径；书籍 manifest 不能覆盖。
pub const READER_ENGINE_PATH: &str = "__qurner/reader-engine.js";
const READER_ENGINE: &str = include_str!("../../resources/reader-engine.js");
const CSP_TEMPLATE: &str = "default-src 'self' data: blob: epub:; script-src 'nonce-{nonce}'; style-src 'self' 'unsafe-inline' epub:; img-src 'self' data: blob: epub:; font-src 'self' data: epub:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";

/// 协议支持的 HTTP 方法。
pub const ALLOW_METHODS: &str = "GET, HEAD";
/// 禁止缓存。
pub const NO_STORE: &str = "no-store";
/// 禁止 MIME 嗅探。
pub const NOSNIFF: &str = "nosniff";

/// 协议处理结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl ProtocolResponse {
    /// 固定短文本错误响应。
    #[must_use]
    pub fn error(status: u16, message: &'static str) -> Self {
        let mut headers = vec![
            ("X-Content-Type-Options".to_string(), NOSNIFF.to_string()),
            ("Cache-Control".to_string(), NO_STORE.to_string()),
        ];
        if status == 405 {
            headers.push(("Allow".to_string(), ALLOW_METHODS.to_string()));
        }
        Self {
            status,
            content_type: Some("text/plain; charset=utf-8".to_string()),
            headers,
            body: message.as_bytes().to_vec(),
        }
    }
}

/// 处理一次协议请求。
#[must_use]
pub fn handle(registry: &SessionRegistry, method: &str, uri: &str) -> ProtocolResponse {
    if method != "GET" && method != "HEAD" {
        return ProtocolResponse::error(405, "Method Not Allowed");
    }

    let Ok((token, resource)) = parse_request(uri) else {
        return ProtocolResponse::error(404, "Not Found");
    };
    let Some(session) = registry.lookup(&token) else {
        return ProtocolResponse::error(404, "Not Found");
    };

    // 保留路径：会话有效时提供内置阅读脚本；书籍 manifest 不能覆盖。
    if resource.as_str() == READER_ENGINE_PATH {
        return reader_engine_response(method);
    }
    if resource.as_str().starts_with("__qurner/") {
        return ProtocolResponse::error(404, "Not Found");
    }

    let Some(item) = session
        .book
        .manifest
        .values()
        .find(|item| href_matches(item.href.as_str(), &resource))
    else {
        return ProtocolResponse::error(404, "Not Found");
    };
    let media_type = item.media_type.clone();
    if !is_valid_media_type(&media_type) {
        return ProtocolResponse::error(500, "Internal Server Error");
    }
    let spine = session
        .book
        .spine
        .iter()
        .find(|entry| entry.href == resource)
        .and_then(|entry| {
            u32::try_from(entry.index)
                .ok()
                .map(|index| (index, entry.href.as_str()))
        });

    match read_resource(&session.source_path, &resource, MAX_RESOURCE_BYTES) {
        Ok(bytes) => {
            if is_html_media_type(&media_type) {
                return html_response(method, &media_type, &bytes, &session, spine);
            }
            ProtocolResponse {
                status: 200,
                content_type: Some(media_type),
                headers: vec![
                    ("X-Content-Type-Options".to_string(), NOSNIFF.to_string()),
                    ("Cache-Control".to_string(), NO_STORE.to_string()),
                ],
                body: if method == "HEAD" { Vec::new() } else { bytes },
            }
        }
        Err(ResourceReadError::NotFound) => ProtocolResponse::error(404, "Not Found"),
        Err(ResourceReadError::LimitExceeded) => ProtocolResponse::error(413, "Payload Too Large"),
        Err(ResourceReadError::Corrupt) => ProtocolResponse::error(500, "Internal Server Error"),
    }
}

fn reader_engine_response(method: &str) -> ProtocolResponse {
    ProtocolResponse {
        status: 200,
        content_type: Some("text/javascript; charset=utf-8".to_string()),
        headers: vec![
            ("X-Content-Type-Options".to_string(), NOSNIFF.to_string()),
            ("Cache-Control".to_string(), NO_STORE.to_string()),
        ],
        body: if method == "HEAD" {
            Vec::new()
        } else {
            READER_ENGINE.as_bytes().to_vec()
        },
    }
}

fn html_response(
    method: &str,
    media_type: &str,
    bytes: &[u8],
    session: &crate::session::Session,
    spine: Option<(u32, &str)>,
) -> ProtocolResponse {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return ProtocolResponse::error(500, "Internal Server Error");
    };
    let Some(nonce) = random_nonce() else {
        return ProtocolResponse::error(500, "Internal Server Error");
    };
    let config = html::RewriteConfig {
        nonce: &nonce,
        token: &session.token,
        session_id: &session.session_id,
        spine_index: spine.map(|(index, _)| index),
        spine_href: spine.map(|(_, href)| href),
    };
    let Some(rewritten) = html::rewrite_document(text, &config) else {
        return ProtocolResponse::error(500, "Internal Server Error");
    };
    let csp = content_security_policy(&nonce);
    ProtocolResponse {
        status: 200,
        content_type: Some(media_type.to_string()),
        headers: vec![
            ("X-Content-Type-Options".to_string(), NOSNIFF.to_string()),
            ("Cache-Control".to_string(), NO_STORE.to_string()),
            ("Content-Security-Policy".to_string(), csp),
        ],
        body: if method == "HEAD" {
            Vec::new()
        } else {
            rewritten.into_bytes()
        },
    }
}

fn is_html_media_type(media_type: &str) -> bool {
    let essence = media_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    essence == "application/xhtml+xml" || essence == "text/html"
}

fn content_security_policy(nonce: &str) -> String {
    CSP_TEMPLATE.replace("{nonce}", nonce)
}

fn random_nonce() -> Option<String> {
    random_nonce_with_reader(read_urandom)
}

fn random_nonce_with_reader<F: FnOnce() -> Option<[u8; 16]>>(reader: F) -> Option<String> {
    match reader() {
        Some(bytes) => Some(hex(&bytes)),
        None => Some(fallback_nonce()),
    }
}

fn read_urandom() -> Option<[u8; 16]> {
    let mut bytes = [0_u8; 16];
    if let Ok(mut file) = std::fs::File::open("/dev/urandom")
        && file.read_exact(&mut bytes).is_ok()
    {
        return Some(bytes);
    }
    None
}

/// CSPRNG 回退（uuid 底层 getrandom）：两个 UUID v4 拼接为 64 位十六进制，
/// 约 244 位随机，满足至少 128 位要求。
fn fallback_nonce() -> String {
    let first = uuid::Uuid::new_v4().simple().to_string();
    let second = uuid::Uuid::new_v4().simple().to_string();
    format!("{first}{second}")
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(TABLE[(byte >> 4) as usize] as char);
        output.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    output
}

/// 在 Tauri builder 上注册 `epub` 协议。
#[must_use]
pub fn register<R: tauri::Runtime>(
    builder: tauri::Builder<R>,
    registry: Arc<SessionRegistry>,
) -> tauri::Builder<R> {
    builder.register_asynchronous_uri_scheme_protocol(
        "epub",
        move |_context, request, responder| {
            let method = request.method().as_str().to_string();
            let uri = request.uri().to_string();
            let registry = Arc::clone(&registry);
            // ZIP 打开、解压与资源读取放入阻塞任务，避免占用 WebView 回调。
            tauri::async_runtime::spawn_blocking(move || {
                let response = handle(&registry, &method, &uri);
                responder.respond(to_http_response(response));
            });
        },
    )
}

/// 把纯函数结果转换为 Tauri HTTP 响应。
#[must_use]
pub fn to_http_response(response: ProtocolResponse) -> tauri::http::Response<Vec<u8>> {
    let mut builder = tauri::http::Response::builder().status(response.status);
    if let Some(content_type) = response.content_type {
        builder = builder.header("Content-Type", content_type);
    }
    for (name, value) in response.headers {
        builder = builder.header(name, value);
    }
    match builder.body(response.body) {
        Ok(response) => response,
        // 任何 header/body 构造失败都返回固定 500，不返回空 200。
        Err(_) => fixed_internal_error(),
    }
}

fn fixed_internal_error() -> tauri::http::Response<Vec<u8>> {
    let mut response = tauri::http::Response::new(b"Internal Server Error".to_vec());
    *response.status_mut() = tauri::http::StatusCode::INTERNAL_SERVER_ERROR;
    let headers = response.headers_mut();
    headers.insert(
        tauri::http::header::CONTENT_TYPE,
        tauri::http::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    headers.insert(
        tauri::http::HeaderName::from_static("x-content-type-options"),
        tauri::http::HeaderValue::from_static(NOSNIFF),
    );
    headers.insert(
        tauri::http::header::CACHE_CONTROL,
        tauri::http::HeaderValue::from_static(NO_STORE),
    );
    response
}

fn is_valid_media_type(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 255 {
        return false;
    }
    let Some((type_part, subtype_part)) = trimmed.split_once('/') else {
        return false;
    };
    !type_part.is_empty()
        && !subtype_part.is_empty()
        && trimmed
            .chars()
            .all(|character| character.is_ascii() && !character.is_ascii_control())
}

fn parse_request(uri: &str) -> Result<(String, ResourcePath), ()> {
    let rest = uri.strip_prefix("epub://").ok_or(())?;
    // 查询参数与 fragment 不改变资源身份。
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let (token, raw_path) = rest.split_once('/').ok_or(())?;
    if !is_token(token) {
        return Err(());
    }

    let decoded = percent_decode(raw_path)?;
    // 编码斜杠不得改变路径层级（%2F/%2f）。
    if decoded.matches('/').count() != raw_path.matches('/').count() {
        return Err(());
    }

    let resource = ResourcePath::new(decoded).map_err(|_| ())?;
    if resource.as_str() == "META-INF" || resource.as_str().starts_with("META-INF/") {
        return Err(());
    }
    Ok((token.to_string(), resource))
}

/// manifest href 可能是 URL 百分号编码（OPF 中的 UTF-8 href 经 rbook 保留为
/// 编码形式）；严格解码一次后与请求资源路径精确比较，不做二次编码回退。
fn href_matches(href: &str, resource: &ResourcePath) -> bool {
    if href == resource.as_str() {
        return true;
    }
    match percent_decode(href) {
        Ok(decoded) => decoded == resource.as_str(),
        Err(()) => false,
    }
}

fn is_token(token: &str) -> bool {
    (token.len() == 32 || token.len() == 64) && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn percent_decode(input: &str) -> Result<String, ()> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%' {
            let high = *bytes.get(index + 1).ok_or(())?;
            let low = *bytes.get(index + 2).ok_or(())?;
            decoded.push(hex_value(high)? << 4 | hex_value(low)?);
            index += 3;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| ())
}

fn hex_value(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_primary_uses_urandom_bytes() {
        let nonce = random_nonce_with_reader(|| Some([0xab; 16])).expect("nonce");
        assert_eq!(nonce, "ab".repeat(16));
        assert_eq!(nonce.len(), 32);
    }

    #[test]
    fn nonce_fallback_uses_uuid_v4_csprng() {
        let nonce = random_nonce_with_reader(|| None).expect("nonce");
        assert_eq!(nonce.len(), 64);
        assert!(nonce.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let second = random_nonce_with_reader(|| None).expect("nonce");
        assert_ne!(nonce, second);
    }
}
