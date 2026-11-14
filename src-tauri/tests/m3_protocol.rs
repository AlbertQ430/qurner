use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "qurner-m3-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

use epub_core::parse_epub;
use qurner_lib::protocol::{ProtocolResponse, handle};
use qurner_lib::session::SessionRegistry;

const CONTAINER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

const NAV: &str = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="chapter1.xhtml">第一章</a></li></ol></nav></body></html>"#;

struct ExtraEntry {
    name: &'static str,
    media_type: &'static str,
    data: Vec<u8>,
}

fn build_epub(chapter_body: &str, extras: &[ExtraEntry]) -> Vec<u8> {
    let mut manifest = String::new();
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", CONTAINER.as_bytes()),
        ("OEBPS/nav.xhtml", NAV.as_bytes()),
    ];
    let chapter = format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title></title></head><body><p>{chapter_body}</p></body></html>"
    );
    entries.push(("OEBPS/chapter1.xhtml", chapter.as_bytes()));

    for (index, extra) in extras.iter().enumerate() {
        let href = extra
            .name
            .strip_prefix("OEBPS/")
            .expect("extra entries live under OEBPS/");
        manifest.push_str(&format!(
            "    <item id=\"extra{index}\" href=\"{href}\" media-type=\"{}\"/>\n",
            extra.media_type
        ));
        entries.push((extra.name, &extra.data));
    }

    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>协议测试书</dc:title>
    <dc:creator>测试作者</dc:creator>
    <dc:identifier id="id">urn:uuid:m3-protocol</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="c1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
{manifest}  </manifest>
  <spine><itemref idref="c1"/></spine>
</package>
"#
    );
    entries.push(("OEBPS/content.opf", opf.as_bytes()));

    build_zip(&entries)
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

fn default_extras() -> Vec<ExtraEntry> {
    vec![
        ExtraEntry {
            name: "OEBPS/style.css",
            media_type: "text/css",
            data: b"body { color: black; }".to_vec(),
        },
        ExtraEntry {
            name: "OEBPS/image.png",
            media_type: "image/png",
            data: vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3],
        },
        ExtraEntry {
            name: "OEBPS/image.jpg",
            media_type: "image/jpeg",
            data: vec![0xff, 0xd8, 0xff, 0xe0, 1, 2, 3],
        },
        ExtraEntry {
            name: "OEBPS/font.ttf",
            media_type: "font/ttf",
            data: vec![0x00, 0x01, 0x00, 0x00, 9, 9],
        },
        ExtraEntry {
            name: "OEBPS/图片 名.png",
            media_type: "image/png",
            data: vec![0x89, b'P', b'N', b'G', 42],
        },
    ]
}

fn write_book(directory: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, bytes).expect("write book");
    path
}

fn create_session(registry: &SessionRegistry, path: &Path) -> String {
    let book = parse_epub(path).expect("parse book");
    registry.create(path, book).expect("create session").token
}

fn uri(token: &str, path: &str) -> String {
    format!("epub://{token}/{}", encode_path(path))
}

fn encode_path(path: &str) -> String {
    let mut output = String::new();
    for byte in path.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                output.push(*byte as char);
            }
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

fn body_text(response: &ProtocolResponse) -> String {
    String::from_utf8_lossy(&response.body).into_owned()
}

fn header<'a>(response: &'a ProtocolResponse, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(header_name, _)| header_name == name)
        .map(|(_, value)| value.as_str())
}

fn assert_no_leaks(response: &ProtocolResponse, secrets: &[&str]) {
    let body = body_text(response);
    for secret in secrets {
        assert!(!body.contains(secret), "错误响应泄漏 {secret}");
    }
    for needle in ["zip", "io", "rbook", "rusqlite", "SELECT", "/Users/"] {
        assert!(!body.contains(needle), "错误响应泄漏 {needle}");
    }
}

#[test]
fn serves_manifest_resources_with_mime_and_headers() {
    let directory = TempDir::new().expect("temp");
    let book_path = write_book(
        directory.path(),
        "book.epub",
        &build_epub("正文", &default_extras()),
    );
    let registry = SessionRegistry::new();
    let token = create_session(&registry, &book_path);

    let chapter = handle(&registry, "GET", &uri(&token, "OEBPS/chapter1.xhtml"));
    assert_eq!(chapter.status, 200);
    assert_eq!(
        chapter.content_type.as_deref(),
        Some("application/xhtml+xml")
    );
    assert_eq!(header(&chapter, "X-Content-Type-Options"), Some("nosniff"));
    assert_eq!(header(&chapter, "Cache-Control"), Some("no-store"));
    assert!(body_text(&chapter).contains("正文"));

    for (path, media_type) in [
        ("OEBPS/style.css", "text/css"),
        ("OEBPS/image.png", "image/png"),
        ("OEBPS/image.jpg", "image/jpeg"),
        ("OEBPS/font.ttf", "font/ttf"),
        ("OEBPS/图片 名.png", "image/png"),
    ] {
        let response = handle(&registry, "GET", &uri(&token, path));
        assert_eq!(response.status, 200, "{path}");
        assert_eq!(response.content_type.as_deref(), Some(media_type), "{path}");
        assert!(!response.body.is_empty(), "{path}");
    }
}

#[test]
fn head_returns_same_headers_and_empty_body() {
    let directory = TempDir::new().expect("temp");
    let book_path = write_book(
        directory.path(),
        "book.epub",
        &build_epub("正文", &default_extras()),
    );
    let registry = SessionRegistry::new();
    let token = create_session(&registry, &book_path);

    let get = handle(&registry, "GET", &uri(&token, "OEBPS/style.css"));
    let head = handle(&registry, "HEAD", &uri(&token, "OEBPS/style.css"));
    assert_eq!(head.status, get.status);
    assert_eq!(head.content_type, get.content_type);
    assert_eq!(head.headers, get.headers);
    assert!(head.body.is_empty());
    assert!(!get.body.is_empty());
}

#[test]
fn rejects_other_methods_with_allow_header() {
    let directory = TempDir::new().expect("temp");
    let book_path = write_book(directory.path(), "book.epub", &build_epub("正文", &[]));
    let registry = SessionRegistry::new();
    let token = create_session(&registry, &book_path);

    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        let response = handle(&registry, method, &uri(&token, "OEBPS/chapter1.xhtml"));
        assert_eq!(response.status, 405, "{method}");
        assert_eq!(header(&response, "Allow"), Some("GET, HEAD"));
    }
}

#[test]
fn unknown_closed_and_malformed_tokens_are_404() {
    let directory = TempDir::new().expect("temp");
    let book_path = write_book(directory.path(), "book.epub", &build_epub("正文", &[]));
    let registry = SessionRegistry::new();
    let book = parse_epub(&book_path).expect("parse");
    let handle_value = registry.create(&book_path, book).expect("session");
    let token = handle_value.token.clone();

    assert_eq!(
        handle(
            &registry,
            "GET",
            &format!("epub://{}/x.xhtml", "0".repeat(32))
        )
        .status,
        404
    );
    assert_eq!(handle(&registry, "GET", "epub://short/x.xhtml").status, 404);
    assert_eq!(handle(&registry, "GET", "https://x/y").status, 404);

    assert!(registry.close(&handle_value.session_id));
    assert_eq!(
        handle(&registry, "GET", &uri(&token, "OEBPS/chapter1.xhtml")).status,
        404
    );
}

#[test]
fn rejects_unsafe_unlisted_and_directory_paths() {
    let directory = TempDir::new().expect("temp");
    let book_path = write_book(
        directory.path(),
        "book.epub",
        &build_epub("正文", &default_extras()),
    );
    let registry = SessionRegistry::new();
    let token = create_session(&registry, &book_path);
    let secrets = [token.as_str(), "协议测试书"];

    let attacks = [
        "OEBPS/../content.opf",
        "OEBPS/%2e%2e/content.opf",
        "OEBPS%2Fstyle.css",
        "%2Fetc%2Fpasswd",
        "OEBPS%5Cstyle.css",
        "OEBPS/style.css%00.png",
        "META-INF/container.xml",
        "OEBPS/",
        "OEBPS",
        "OEBPS/missing.xhtml",
        "OEBPS/missing.xhtml?x=1",
        "css/style.css",
    ];
    for path in attacks {
        let raw_uri = format!("epub://{token}/{path}");
        let response = handle(&registry, "GET", &raw_uri);
        assert_eq!(response.status, 404, "{path}");
        assert_no_leaks(&response, &secrets);
    }
}

#[test]
fn decodes_percent_encoded_non_ascii_paths_once() {
    let directory = TempDir::new().expect("temp");
    let book_path = write_book(
        directory.path(),
        "book.epub",
        &build_epub("正文", &default_extras()),
    );
    let registry = SessionRegistry::new();
    let token = create_session(&registry, &book_path);

    let encoded = format!("epub://{token}/OEBPS/%E5%9B%BE%E7%89%87%20%E5%90%8D.png");
    let response = handle(&registry, "GET", &encoded);
    assert_eq!(response.status, 200);
    assert_eq!(response.content_type.as_deref(), Some("image/png"));

    // 二次编码（%25E5...）只解码一次，结果不匹配 manifest。
    let double =
        format!("epub://{token}/OEBPS/%25E5%259B%25BE%25E7%2589%2587%2520%25E5%2590%258D.png");
    assert_eq!(handle(&registry, "GET", &double).status, 404);
    assert_eq!(
        handle(
            &registry,
            "GET",
            &format!("epub://{token}/OEBPS/bad%zz.png")
        )
        .status,
        404
    );
}

#[test]
fn isolates_concurrent_sessions_with_same_resource_path() {
    let directory = TempDir::new().expect("temp");
    let book_a = write_book(directory.path(), "a.epub", &build_epub("第一本书", &[]));
    let book_b = write_book(
        directory.path(),
        "b.epub",
        &build_epub(
            "第二本书",
            &[ExtraEntry {
                name: "OEBPS/b-only.xhtml",
                media_type: "application/xhtml+xml",
                data: b"<html><body><p>B only</p></body></html>".to_vec(),
            }],
        ),
    );
    let registry = Arc::new(SessionRegistry::new());
    let token_a = create_session(&registry, &book_a);
    let token_b = create_session(&registry, &book_b);

    let registry_a = Arc::clone(&registry);
    let token_a_clone = token_a.clone();
    let first = std::thread::spawn(move || {
        handle(
            &registry_a,
            "GET",
            &uri(&token_a_clone, "OEBPS/chapter1.xhtml"),
        )
    });
    let registry_b = Arc::clone(&registry);
    let token_b_clone = token_b.clone();
    let second = std::thread::spawn(move || {
        handle(
            &registry_b,
            "GET",
            &uri(&token_b_clone, "OEBPS/chapter1.xhtml"),
        )
    });
    let first = first.join().expect("join");
    let second = second.join().expect("join");
    assert!(body_text(&first).contains("第一本书"));
    assert!(body_text(&second).contains("第二本书"));
    assert!(!body_text(&first).contains("第二本书"));

    // 跨会话：A 的令牌请求 B 独有路径 → 404。
    let cross = handle(&registry, "GET", &uri(&token_a, "OEBPS/b-only.xhtml"));
    assert_eq!(cross.status, 404);
    let own = handle(&registry, "GET", &uri(&token_b, "OEBPS/b-only.xhtml"));
    assert_eq!(own.status, 200);
}

#[test]
fn rejects_oversized_declared_resource_and_corrupted_entry() {
    let directory = TempDir::new().expect("temp");
    let oversized = write_book(
        directory.path(),
        "oversized.epub",
        &build_epub(
            "正文",
            &[ExtraEntry {
                name: "OEBPS/huge.bin",
                media_type: "application/octet-stream",
                data: vec![3_u8; 256 * 1024],
            }],
        ),
    );
    let book = parse_epub(&oversized).expect("parse");
    let mut bytes = fs::read(&oversized).expect("read");
    forge_central_size(&mut bytes, "OEBPS/huge.bin", 300 * 1024 * 1024);
    fs::write(&oversized, &bytes).expect("write forged");
    let registry = SessionRegistry::new();
    let handle_value = registry.create(&oversized, book).expect("session");
    let response = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "OEBPS/huge.bin"),
    );
    assert_eq!(response.status, 413);
    assert_no_leaks(&response, &[handle_value.token.as_str()]);

    let corrupted = write_book(directory.path(), "corrupted.epub", &build_epub("正文", &[]));
    let book = parse_epub(&corrupted).expect("parse");
    let mut bytes = fs::read(&corrupted).expect("read");
    corrupt_entry_data(&mut bytes, "OEBPS/chapter1.xhtml");
    fs::write(&corrupted, &bytes).expect("write corrupted");
    let handle_value = registry.create(&corrupted, book).expect("session");
    let response = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "OEBPS/chapter1.xhtml"),
    );
    assert_eq!(response.status, 500);
    assert_no_leaks(&response, &[handle_value.token.as_str()]);
}

fn central_offset(bytes: &[u8], name: &str) -> usize {
    let eocd = (0..bytes.len() - 4)
        .rev()
        .find(|&index| bytes[index..index + 4] == [0x50, 0x4b, 0x05, 0x06])
        .expect("EOCD");
    let central_start = u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().unwrap());
    let mut position = central_start as usize;
    while position + 46 <= bytes.len() && bytes[position..position + 4] == [0x50, 0x4b, 0x01, 0x02]
    {
        let name_len =
            u16::from_le_bytes(bytes[position + 28..position + 30].try_into().unwrap()) as usize;
        let extra_len =
            u16::from_le_bytes(bytes[position + 30..position + 32].try_into().unwrap()) as usize;
        let comment_len =
            u16::from_le_bytes(bytes[position + 32..position + 34].try_into().unwrap()) as usize;
        if &bytes[position + 46..position + 46 + name_len] == name.as_bytes() {
            return position;
        }
        position += 46 + name_len + extra_len + comment_len;
    }
    panic!("central entry not found: {name}");
}

fn forge_central_size(bytes: &mut [u8], name: &str, size: u32) {
    let offset = central_offset(bytes, name);
    bytes[offset + 20..offset + 24].copy_from_slice(&size.to_le_bytes());
    bytes[offset + 24..offset + 28].copy_from_slice(&size.to_le_bytes());
}

fn corrupt_entry_data(bytes: &mut [u8], name: &str) {
    let central = central_offset(bytes, name);
    let local_offset =
        u32::from_le_bytes(bytes[central + 42..central + 46].try_into().unwrap()) as usize;
    let name_len = u16::from_le_bytes(
        bytes[local_offset + 26..local_offset + 28]
            .try_into()
            .unwrap(),
    ) as usize;
    let extra_len = u16::from_le_bytes(
        bytes[local_offset + 28..local_offset + 30]
            .try_into()
            .unwrap(),
    ) as usize;
    let data_offset = local_offset + 30 + name_len + extra_len;
    bytes[data_offset] ^= 0xff;
}

#[test]
fn registers_epub_protocol_in_mock_runtime() {
    let registry = Arc::new(SessionRegistry::new());
    let _app = qurner_lib::protocol::register(tauri::test::mock_builder(), Arc::clone(&registry))
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("注册 epub 协议的 mock app 应可构建");

    // Tauri 2 不公开已注册协议表（manager 为 sealed）；以注册调用源码与
    // 纯函数 404 作为可验证证据。
    let protocol_source =
        fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/protocol/mod.rs"))
            .expect("read protocol source");
    assert!(
        protocol_source.contains("register_asynchronous_uri_scheme_protocol"),
        "异步协议注册调用应存在"
    );
    assert!(protocol_source.contains("\"epub\""), "应注册 epub scheme");

    let response = handle(&registry, "GET", "epub://unknown/x.xhtml");
    assert_eq!(response.status, 404);
}

#[test]
fn has_no_network_or_plugin_dependencies() {
    let manifest = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("read Cargo.toml");
    for needle in [
        "tauri-plugin-",
        "plugin-http",
        "plugin-shell",
        "plugin-fs",
        "plugin-dialog",
        "reqwest",
        "hyper",
    ] {
        assert!(!manifest.contains(needle), "不应包含 {needle}");
    }

    for source in [
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/protocol/mod.rs"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/session/mod.rs"),
    ] {
        let source = fs::read_to_string(source).expect("read source");
        for needle in ["reqwest", "hyper", "TcpStream", "TcpListener", "std::net"] {
            assert!(!source.contains(needle), "不应包含网络调用 {needle}");
        }
    }
}

#[test]
fn rejects_invalid_manifest_mime_with_fixed_500() {
    let directory = TempDir::new().expect("temp");
    let book_path = write_book(directory.path(), "book.epub", &build_epub("正文", &[]));
    let mut book = parse_epub(&book_path).expect("parse");
    let item = book
        .manifest
        .values_mut()
        .find(|item| item.href.as_str().ends_with("chapter1.xhtml"))
        .expect("chapter item");
    item.media_type = "not-a-mime".to_string();

    let registry = SessionRegistry::new();
    let handle_value = registry.create(&book_path, book).expect("session");
    let response = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "OEBPS/chapter1.xhtml"),
    );
    assert_eq!(response.status, 500);
    assert_eq!(body_text(&response), "Internal Server Error");
    assert_no_leaks(&response, &[handle_value.token.as_str()]);
}

#[test]
fn response_construction_failure_returns_fixed_500() {
    let response = qurner_lib::protocol::to_http_response(qurner_lib::protocol::ProtocolResponse {
        status: 200,
        content_type: Some("text/plain\r\nX".to_string()),
        headers: Vec::new(),
        body: b"x".to_vec(),
    });
    assert_eq!(
        response.status(),
        tauri::http::StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(response.body().as_slice(), b"Internal Server Error");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
}

#[test]
fn accepts_64_hex_tokens_as_unknown_sessions() {
    let registry = SessionRegistry::new();
    let token = "a".repeat(64);
    let response = handle(
        &registry,
        "GET",
        &format!("epub://{token}/OEBPS/chapter1.xhtml"),
    );
    assert_eq!(response.status, 404);
}
