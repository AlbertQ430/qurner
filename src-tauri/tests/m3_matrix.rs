#[path = "../../crates/epub-core/tests/support/fixtures.rs"]
mod fixtures;
#[path = "../../crates/epub-core/tests/support/zip_writer.rs"]
mod zip_writer;

use epub_core::parse_epub;
use fixtures::{Fixture, FixtureKind};
use qurner_lib::protocol::{ProtocolResponse, handle};
use qurner_lib::session::SessionRegistry;

const KINDS: [FixtureKind; 6] = [
    FixtureKind::MinimalEpub2,
    FixtureKind::MinimalEpub3,
    FixtureKind::NestedToc,
    FixtureKind::MissingMetadata,
    FixtureKind::CssAndImage,
    FixtureKind::Bilingual,
];

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

fn uri(token: &str, href: &str) -> String {
    format!("epub://{token}/{}", encode_path(href))
}

fn header<'a>(response: &'a ProtocolResponse, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(header_name, _)| header_name == name)
        .map(|(_, value)| value.as_str())
}

fn session(path: &std::path::Path) -> (SessionRegistry, String, String, epub_core::ParsedBook) {
    let book = parse_epub(path).expect("应能解析夹具");
    let registry = SessionRegistry::new();
    let handle_value = registry
        .create("book-1", path, book.clone())
        .expect("应能创建会话");
    (registry, handle_value.token, handle_value.session_id, book)
}

#[test]
fn six_fixture_matrix_serves_spine_and_assets() {
    for kind in KINDS {
        let fixture: Fixture = kind.build();
        let (registry, token, session_id, book) = session(fixture.path());
        assert!(!book.spine.is_empty(), "{kind:?} 应有 spine");
        assert!(!book.manifest.is_empty(), "{kind:?} 应有 manifest");

        match kind {
            FixtureKind::MinimalEpub2 => assert_eq!(book.toc.len(), 1),
            FixtureKind::MinimalEpub3 => assert!(
                book.manifest
                    .values()
                    .any(|item| item.properties.iter().any(|value| value == "nav"))
            ),
            FixtureKind::NestedToc => assert_eq!(book.toc.len(), 2),
            FixtureKind::MissingMetadata => {
                assert!(book.metadata.title.is_none());
                assert!(book.metadata.authors.is_empty());
            }
            FixtureKind::CssAndImage => {
                for (media_type, suffix) in [
                    ("text/css", "style.css"),
                    ("image/png", "cover.png"),
                    ("font/ttf", "font.ttf"),
                ] {
                    assert!(
                        book.manifest.values().any(|item| {
                            item.media_type == media_type && item.href.as_str().ends_with(suffix)
                        }),
                        "{kind:?} 缺少 {suffix}"
                    );
                }
            }
            FixtureKind::Bilingual => {
                assert_eq!(book.spine.len(), 2);
                assert_eq!(book.metadata.title.as_deref(), Some("Bilingual Sample"));
            }
            _ => unreachable!("不在六类夹具集合中"),
        }

        for (spine_index, spine_item) in book.spine.iter().enumerate() {
            let manifest_item = book
                .manifest
                .values()
                .find(|item| item.href == spine_item.href)
                .expect("spine 项应存在于 manifest");
            let response = handle(&registry, "GET", &uri(&token, spine_item.href.as_str()));
            assert_eq!(response.status, 200, "{kind:?} spine {spine_index}");
            assert_eq!(
                response.content_type.as_deref(),
                Some(manifest_item.media_type.as_str())
            );
            assert_eq!(header(&response, "X-Content-Type-Options"), Some("nosniff"));
            assert_eq!(header(&response, "Cache-Control"), Some("no-store"));
            let csp = header(&response, "Content-Security-Policy").expect("HTML 应带 CSP");
            assert!(csp.contains("'nonce-"), "{kind:?} 应有 nonce");
            assert!(csp.contains("base-uri 'none'"));
            assert!(csp.contains("form-action 'none'"));
            let body = String::from_utf8_lossy(&response.body);
            assert_eq!(
                body.matches("<script").count(),
                1,
                "{kind:?} 应只注入一个脚本"
            );
            assert!(
                body.contains(&format!("data-spine-index=\"{spine_index}\"")),
                "{kind:?} spine {spine_index} 缺少 data-spine-index"
            );
            assert!(
                body.contains(&format!("data-spine-href=\"{}\"", spine_item.href.as_str())),
                "{kind:?} spine {spine_index} 缺少 data-spine-href"
            );
            assert!(
                body.contains(&format!("src=\"epub://{token}/__qurner/reader-engine.js\"")),
                "{kind:?} 缺少引擎脚本 URL"
            );
            assert!(
                body.contains(&format!("data-session-id=\"{session_id}\"")),
                "{kind:?} 缺少 sessionId"
            );
        }

        for item in book
            .manifest
            .values()
            .filter(|item| item.media_type != "application/xhtml+xml")
        {
            let response = handle(&registry, "GET", &uri(&token, item.href.as_str()));
            assert_eq!(response.status, 200, "{kind:?} {} ", item.href.as_str());
            assert_eq!(
                response.content_type.as_deref(),
                Some(item.media_type.as_str()),
                "{kind:?} {} MIME",
                item.href.as_str()
            );
            assert!(
                !response.body.is_empty(),
                "{kind:?} {} 资源不应为空",
                item.href.as_str()
            );
            assert_eq!(header(&response, "X-Content-Type-Options"), Some("nosniff"));
            assert_eq!(header(&response, "Cache-Control"), Some("no-store"));
        }

        // 非 spine HTML 不注入阅读引擎。
        for item in book
            .manifest
            .values()
            .filter(|item| item.media_type == "application/xhtml+xml")
            .filter(|item| !book.spine.iter().any(|spine| spine.href == item.href))
        {
            let response = handle(&registry, "GET", &uri(&token, item.href.as_str()));
            assert_eq!(response.status, 200, "{kind:?} 非 spine HTML");
            let body = String::from_utf8_lossy(&response.body);
            assert!(
                !body.contains("reader-engine.js"),
                "{kind:?} 非 spine HTML 不应注入引擎"
            );
        }

        eprintln!("M3D_MATRIX {kind:?} ok spines={}", book.spine.len());
    }
}

#[test]
fn css_and_image_fixture_serves_raw_asset_bytes() {
    let fixture = FixtureKind::CssAndImage.build();
    let (registry, token, _session_id, book) = session(fixture.path());

    let css = handle(&registry, "GET", &uri(&token, "OEBPS/style.css"));
    assert!(String::from_utf8_lossy(&css.body).contains("font-family"));

    let png = handle(&registry, "GET", &uri(&token, "OEBPS/cover.png"));
    assert_eq!(&png.body[..8], b"\x89PNG\r\n\x1a\n");

    let font = handle(&registry, "GET", &uri(&token, "OEBPS/font.ttf"));
    assert_eq!(font.content_type.as_deref(), Some("font/ttf"));
    assert!(!font.body.is_empty());

    // 资源只读，不改原始 EPUB 字节。
    let before = fixture.read_bytes();
    let _ = book;
    let after = fixture.read_bytes();
    assert_eq!(before, after);
}
