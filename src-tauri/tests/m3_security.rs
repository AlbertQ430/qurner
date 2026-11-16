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
            "qurner-m3-security-{}-{}",
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
        "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>章节</title></head><body>{chapter_body}</body></html>"
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
    <dc:title>安全测试书</dc:title>
    <dc:creator>测试作者</dc:creator>
    <dc:identifier id="id">urn:uuid:m3-security</dc:identifier>
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

fn write_file(directory: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, bytes).expect("write fixture");
    path
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

fn csp_nonce(response: &ProtocolResponse) -> String {
    let csp = header(response, "Content-Security-Policy").expect("CSP header");
    let start = csp.find("'nonce-").expect("nonce directive") + "'nonce-".len();
    let end = csp[start..].find('\'').expect("nonce end") + start;
    csp[start..end].to_string()
}

fn hostile_chapter() -> String {
    concat!(
        "<p>正文</p>\n",
        "<script>window.pwned = 1;</script>\n",
        "<iframe src=\"https://evil.test/frame\"></iframe>\n",
        "<object data=\"https://evil.test/object\"></object>\n",
        "<embed src=\"https://evil.test/embed\"/>\n",
        "<base href=\"https://evil.test/\"/>\n",
        "<meta http-equiv=\"refresh\" content=\"0;url=https://evil.test/\"/>\n",
        "<p onclick=\"window.pwned = 2\">点击</p>\n",
        "<a id=\"js\" href=\"javascript:alert(1)\">脚本链接</a>\n",
        "<a id=\"external\" href=\"https://evil.test/page\">外部链接</a>\n",
        "<a id=\"tab-scheme\" href=\"h\tttp://evil.test/tab\">tab</a>\n",
        "<a id=\"newline-scheme\" href=\"java\nscript:alert(1)\">nl</a>\n",
        "<a id=\"cr-scheme\" href=\"h\rttp://evil.test/cr\">cr</a>\n",
        "<svg><use xlink:href=\"h\tttp://evil.test/tab.svg#i\"></use></svg>\n",
        "<a id=\"mailto\" href=\"mailto:reader@example.com\">邮件</a>\n",
        "<a id=\"custom\" href=\"custom-app://open\">自定义</a>\n",
        "<area href=\"https://evil.test/area\" alt=\"area\"/>\n",
        "<svg><use xlink:href=\"https://evil.test/sprite.svg#icon\"></use></svg>\n",
        "<a id=\"ping\" href=\"relative.html\" ping=\"https://evil.test/track\">点击</a>\n",
        "<form action=\"//evil.test/form\"><button formaction=\"//evil.test/submit\">提交</button></form>\n",
        "<form action=\"h\tttp://evil.test/tab-form\"><button formaction=\"java\nscript:alert(1)\">提交</button></form>\n",
        "<img src=\"images/pic.png\" alt=\"本地图片\"/>\n",
        "<link rel=\"stylesheet\" href=\"style.css\"/>\n",
    )
    .to_string()
}

#[test]
fn html_responses_use_fresh_nonce_and_inject_single_engine() {
    let directory = TempDir::new().expect("temp");
    let clean = build_epub(
        "<p>正文</p>",
        &[ExtraEntry {
            name: "OEBPS/style.css",
            media_type: "text/css",
            data: b"body { color: black; }".to_vec(),
        }],
    );
    let path = write_file(directory.path(), "book.epub", &clean);
    let registry = SessionRegistry::new();
    let book = parse_epub(&path).expect("parse");
    let handle_value = registry.create("book-1", &path, book).expect("session");

    let first = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "OEBPS/chapter1.xhtml"),
    );
    let second = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "OEBPS/chapter1.xhtml"),
    );
    assert_eq!(first.status, 200);
    assert_eq!(first.content_type.as_deref(), Some("application/xhtml+xml"));
    let nonce_first = csp_nonce(&first);
    let nonce_second = csp_nonce(&second);
    assert!(
        matches!(nonce_first.len(), 32 | 64),
        "nonce 应为 32 或 64 位十六进制"
    );
    assert!(nonce_first.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_ne!(nonce_first, nonce_second, "每个响应应使用独立 nonce");

    let csp = header(&first, "Content-Security-Policy").expect("csp");
    assert!(csp.contains(&format!("'nonce-{nonce_first}'")));
    assert!(csp.contains("base-uri 'none'"));
    assert!(csp.contains("form-action 'none'"));
    assert!(csp.contains("connect-src 'none'"));
    assert!(!csp.contains("script-src 'unsafe-inline'"));
    assert!(!csp.contains("http:"));
    assert!(!csp.contains("https:"));

    let first_body = body_text(&first);
    assert_eq!(first_body.matches("<script").count(), 1, "只应注入一个脚本");
    assert!(first_body.contains(&format!("nonce=\"{nonce_first}\"")));
    assert!(first_body.contains(&format!(
        "src=\"epub://{}/__qurner/reader-engine.js\"",
        handle_value.token
    )));
    assert!(first_body.contains(&format!("data-session-id=\"{}\"", handle_value.session_id)));
    assert_eq!(header(&first, "X-Content-Type-Options"), Some("nosniff"));
    assert_eq!(header(&first, "Cache-Control"), Some("no-store"));

    // HEAD 返回相同状态/头但空正文。
    let head = handle(
        &registry,
        "HEAD",
        &uri(&handle_value.token, "OEBPS/chapter1.xhtml"),
    );
    assert_eq!(head.status, 200);
    assert_eq!(head.content_type, first.content_type);
    assert!(head.body.is_empty());
}

#[test]
fn strips_active_content_and_preserves_relative_urls() {
    let directory = TempDir::new().expect("temp");
    let clean = build_epub(
        "<p>正文</p>",
        &[
            ExtraEntry {
                name: "OEBPS/style.css",
                media_type: "text/css",
                data: b"body { color: black; }".to_vec(),
            },
            ExtraEntry {
                name: "OEBPS/images/pic.png",
                media_type: "image/png",
                data: vec![0x89, b'P', b'N', b'G', 1],
            },
        ],
    );
    let path = write_file(directory.path(), "book.epub", &clean);
    let registry = SessionRegistry::new();
    let book = parse_epub(&path).expect("parse");
    let handle_value = registry.create("book-1", &path, book).expect("session");

    // 替换为含主动内容的 HTML（解析阶段会拒绝 script，因此先解析干净版本）。
    let hostile = build_epub(
        &hostile_chapter(),
        &[
            ExtraEntry {
                name: "OEBPS/style.css",
                media_type: "text/css",
                data: b"body { color: black; }".to_vec(),
            },
            ExtraEntry {
                name: "OEBPS/images/pic.png",
                media_type: "image/png",
                data: vec![0x89, b'P', b'N', b'G', 1],
            },
        ],
    );
    fs::write(&path, hostile).expect("write hostile book");

    let response = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "OEBPS/chapter1.xhtml"),
    );
    assert_eq!(response.status, 200);
    let body = body_text(&response).to_ascii_lowercase();

    assert!(!body.contains("window.pwned"), "原书脚本应被移除");
    assert!(!body.contains("<iframe"));
    assert!(!body.contains("<object"));
    assert!(!body.contains("<embed"));
    assert!(!body.contains("<base"));
    assert!(!body.contains("http-equiv=\"refresh\""));
    assert!(!body.contains("onclick"));
    assert!(!body.contains("javascript:"));
    assert!(!body.contains("href=\"https://evil.test/page\""));
    assert!(!body.contains("mailto:"));
    assert!(!body.contains("custom-app://"));
    assert!(!body.contains("https://evil.test/area"));
    assert!(!body.contains("xlink:href=\"https"));
    assert!(
        !body.contains("evil.test/tab"),
        "制表符混淆的 scheme 应被移除"
    );
    assert!(!body.contains("evil.test/tab.svg"), "xlink 混淆应被移除");
    assert!(!body.contains("javascript:"), "换行混淆的 scheme 应被移除");
    assert!(!body.contains("h\tttp"));
    assert!(!body.contains("ping="));
    assert!(!body.contains("https://evil.test/form"));
    assert!(!body.contains("https://evil.test/submit"));
    assert!(body.contains("data-qurner-external=\"blocked\""));
    assert!(body.contains("relative.html"), "相对链接应保留");
    assert!(body.contains("images/pic.png"));
    assert!(body.contains("style.css"));
    assert!(body.contains("外部链接"), "外部链接文字应保留");
    assert_eq!(body.matches("<script").count(), 1);
}

#[test]
fn serves_built_engine_and_rejects_manifest_shadowing() {
    let directory = TempDir::new().expect("temp");
    let clean = build_epub(
        "<p>正文</p>",
        &[
            ExtraEntry {
                name: "OEBPS/style.css",
                media_type: "text/css",
                data: b"body { color: black; }".to_vec(),
            },
            ExtraEntry {
                name: "OEBPS/__qurner/reader-engine.js",
                media_type: "application/javascript",
                data: b"EVIL-MANIFEST-SHADOW".to_vec(),
            },
        ],
    );
    let path = write_file(directory.path(), "book.epub", &clean);
    let registry = SessionRegistry::new();
    let book = parse_epub(&path).expect("parse");
    let handle_value = registry
        .create("book-1", &path, book.clone())
        .expect("session");

    // 即使 manifest 声明了保留路径，也只能返回内置脚本。
    let response = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "__qurner/reader-engine.js"),
    );
    assert_eq!(response.status, 200);
    assert_eq!(
        response.content_type.as_deref(),
        Some("text/javascript; charset=utf-8")
    );
    assert_eq!(header(&response, "X-Content-Type-Options"), Some("nosniff"));
    assert_eq!(header(&response, "Cache-Control"), Some("no-store"));
    let body = body_text(&response);
    assert!(body.len() > 1_000, "应返回构建后的 IIFE");
    assert!(!body.contains("EVIL-MANIFEST-SHADOW"));

    let head = handle(
        &registry,
        "HEAD",
        &uri(&handle_value.token, "__qurner/reader-engine.js"),
    );
    assert_eq!(head.status, 200);
    assert!(head.body.is_empty());

    // 关闭会话后保留路径也失效。
    assert!(registry.close(&handle_value.session_id));
    let closed = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "__qurner/reader-engine.js"),
    );
    assert_eq!(closed.status, 404);

    // 其他 __qurner/ 路径即使被 manifest 声明也拒绝。
    let other = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "__qurner/other.js"),
    );
    assert_eq!(other.status, 404);

    // 清单中的同路径条目不能覆盖内置脚本（重新创建会话验证）。
    let second = registry.create("book-1", &path, book).expect("session");
    let response = handle(
        &registry,
        "GET",
        &uri(&second.token, "__qurner/reader-engine.js"),
    );
    assert_eq!(response.status, 200);
    assert!(!body_text(&response).contains("EVIL-MANIFEST-SHADOW"));
}

#[test]
fn non_html_resources_are_not_rewritten() {
    let directory = TempDir::new().expect("temp");
    let clean = build_epub(
        "<p>正文</p>",
        &[
            ExtraEntry {
                name: "OEBPS/style.css",
                media_type: "text/css",
                data: b"body { color: black; }".to_vec(),
            },
            ExtraEntry {
                name: "OEBPS/logo.png",
                media_type: "image/png",
                data: vec![0x89, b'P', b'N', b'G', 7],
            },
        ],
    );
    let path = write_file(directory.path(), "book.epub", &clean);
    let registry = SessionRegistry::new();
    let handle_value = registry
        .create("book-1", &path, parse_epub(&path).expect("parse"))
        .expect("session");

    for (name, media_type, expected) in [
        (
            "OEBPS/style.css",
            "text/css",
            b"body { color: black; }".to_vec(),
        ),
        (
            "OEBPS/logo.png",
            "image/png",
            vec![0x89, b'P', b'N', b'G', 7],
        ),
    ] {
        let response = handle(&registry, "GET", &uri(&handle_value.token, name));
        assert_eq!(response.status, 200);
        assert_eq!(response.content_type.as_deref(), Some(media_type));
        assert_eq!(response.body, expected);
        assert_eq!(header(&response, "X-Content-Type-Options"), Some("nosniff"));
        assert_eq!(header(&response, "Cache-Control"), Some("no-store"));
        assert!(header(&response, "Content-Security-Policy").is_none());
    }
}

#[test]
fn invalid_utf8_html_returns_fixed_500() {
    let directory = TempDir::new().expect("temp");
    let clean = build_epub("<p>正文</p>", &[]);
    let path = write_file(directory.path(), "book.epub", &clean);
    let registry = SessionRegistry::new();
    let handle_value = registry
        .create("book-1", &path, parse_epub(&path).expect("parse"))
        .expect("session");

    fs::write(&path, build_epub_bytes_with_invalid_chapter()).expect("write invalid book");

    let response = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "OEBPS/chapter1.xhtml"),
    );
    assert_eq!(response.status, 500);
    assert_eq!(body_text(&response), "Internal Server Error");
    assert!(!body_text(&response).contains(&handle_value.token));
    assert!(!body_text(&response).contains("/Users/"));
}

fn build_epub_bytes_with_invalid_chapter() -> Vec<u8> {
    let chapter: &[u8] = b"<html><body><p>\xff\xfe</p></body></html>";
    let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>安全测试书</dc:title>
    <dc:creator>测试作者</dc:creator>
    <dc:identifier id="id">urn:uuid:m3-security</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
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
        ("OEBPS/nav.xhtml", NAV.as_bytes()),
        ("OEBPS/chapter1.xhtml", chapter),
        ("OEBPS/content.opf", opf.as_bytes()),
    ])
}

#[test]
fn orphan_engine_requests_are_404() {
    let registry = Arc::new(SessionRegistry::new());
    let token = "0".repeat(32);
    let response = handle(
        &registry,
        "GET",
        &format!("epub://{token}/__qurner/reader-engine.js"),
    );
    assert_eq!(response.status, 404);
}

fn build_epub_with_chapter_raw(chapter: &str) -> Vec<u8> {
    let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>安全测试书</dc:title>
    <dc:creator>测试作者</dc:creator>
    <dc:identifier id="id">urn:uuid:m3-security</dc:identifier>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
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
        ("OEBPS/nav.xhtml", NAV.as_bytes()),
        ("OEBPS/chapter1.xhtml", chapter.as_bytes()),
        ("OEBPS/content.opf", opf.as_bytes()),
    ])
}

#[test]
fn injects_single_engine_when_head_is_only_in_comment_or_heading() {
    let directory = TempDir::new().expect("temp");
    let clean = build_epub("<p>正文</p>", &[]);
    let path = write_file(directory.path(), "book.epub", &clean);
    let registry = SessionRegistry::new();
    let handle_value = registry
        .create("book-1", &path, parse_epub(&path).expect("parse"))
        .expect("session");

    let fragment = "<!-- <head> --><heading>标题</heading><p>正文</p>";
    fs::write(&path, build_epub_with_chapter_raw(fragment)).expect("write fragment book");

    let response = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "OEBPS/chapter1.xhtml"),
    );
    assert_eq!(response.status, 200);
    let body = body_text(&response);
    assert_eq!(body.matches("<script").count(), 1, "必须注入唯一脚本");
    assert!(
        body.starts_with("<script"),
        "无 head/body 时脚本应在片段开头"
    );
    assert!(body.contains("标题"));
}

#[test]
fn non_spine_html_does_not_inject_engine() {
    let directory = TempDir::new().expect("temp");
    let clean = build_epub("<p>正文</p>", &[]);
    let path = write_file(directory.path(), "book.epub", &clean);
    let registry = SessionRegistry::new();
    let handle_value = registry
        .create("book-1", &path, parse_epub(&path).expect("parse"))
        .expect("session");

    let response = handle(
        &registry,
        "GET",
        &uri(&handle_value.token, "OEBPS/nav.xhtml"),
    );
    assert_eq!(response.status, 200);
    let body = body_text(&response);
    assert!(!body.contains("<script"), "非 spine HTML 不注入阅读引擎");
    assert!(body.contains("toc") || body.contains("ol"));
}
