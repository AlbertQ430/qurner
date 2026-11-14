//! EPUB 内容兼容与安全检测。
//!
//! 不执行书内脚本、不访问网络：只读取资源文本，检测固定版式、竖排、
//! 活动内容和远程资源引用。

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read as _;
use std::path::Path;
use std::rc::Rc;

use lol_html::{HtmlRewriter, Settings, element, text};
use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};

use crate::model::{EpubWarning, EpubWarningCode, ManifestItem};
use crate::{EpubError, EpubErrorCode, ResourcePath};

/// 单个待扫描资源允许的最大字节数。
const MAX_SCAN_BYTES: u64 = 16 * 1024 * 1024;

const DISPLAY_OPTIONS_PATH: &str = "META-INF/com.apple.ibooks.display-options.xml";

/// 在解析结果返回前执行固定版式、竖排、脚本和远程资源检查。
pub(crate) fn check_content(
    epub_path: &Path,
    package_path: &ResourcePath,
    manifest: &BTreeMap<String, ManifestItem>,
    warnings: &mut Vec<EpubWarning>,
) -> Result<(), EpubError> {
    let file = File::open(epub_path).map_err(|_| EpubError::io())?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|_| EpubError::new(EpubErrorCode::InvalidZip))?;

    let opf = read_entry(&mut archive, package_path.as_str())?
        .ok_or_else(|| EpubError::new(EpubErrorCode::InvalidOpf))?;
    if opf_is_fixed_layout(&opf)? {
        return Err(
            EpubError::new(EpubErrorCode::FixedLayout).with_resource_path(package_path.clone())
        );
    }

    if let Some(options) = read_entry(&mut archive, DISPLAY_OPTIONS_PATH)?
        && display_options_is_fixed_layout(&options)?
    {
        let display_options = ResourcePath::new(DISPLAY_OPTIONS_PATH)
            .map_err(|_| EpubError::new(EpubErrorCode::InvalidOpf))?;
        return Err(EpubError::new(EpubErrorCode::FixedLayout).with_resource_path(display_options));
    }

    for item in manifest.values() {
        let media_type = item.media_type.to_ascii_lowercase();
        if media_type == "text/css" {
            let content = read_entry(&mut archive, item.href.as_str())?
                .ok_or_else(|| missing_resource(&item.href))?;
            if css_has_vertical_writing(&content) {
                return Err(EpubError::new(EpubErrorCode::VerticalWriting)
                    .with_resource_path(item.href.clone()));
            }
            if css_has_remote_resource(&content) {
                warnings.push(remote_warning(&item.href));
            }
        } else if is_markup_media_type(&media_type) {
            let content = read_entry(&mut archive, item.href.as_str())?
                .ok_or_else(|| missing_resource(&item.href))?;
            scan_markup(&content, &item.href, warnings)?;
        }
    }

    Ok(())
}

fn missing_resource(path: &ResourcePath) -> EpubError {
    EpubError::new(EpubErrorCode::InvalidOpf).with_resource_path(path.clone())
}

fn remote_warning(path: &ResourcePath) -> EpubWarning {
    EpubWarning {
        code: EpubWarningCode::RemoteResourceBlocked,
        resource_href: Some(path.clone()),
    }
}

fn is_markup_media_type(media_type: &str) -> bool {
    matches!(
        media_type,
        "application/xhtml+xml" | "text/html" | "image/svg+xml"
    )
}

fn opf_is_fixed_layout(opf: &str) -> Result<bool, EpubError> {
    let mut reader = Reader::from_str(opf);
    reader.config_mut().trim_text(true);

    let mut in_meta = false;
    let mut property: Option<String> = None;
    let mut content_text = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let local = element.name().local_name().as_ref().to_ascii_lowercase();
                if local != "meta" {
                    continue;
                }
                in_meta = true;
                content_text.clear();
                let attributes = element_attributes(&element)?;
                property = attribute_value(&attributes, "property").map(str::to_string);

                let name = attribute_value(&attributes, "name");
                let content = attribute_value(&attributes, "content");
                if is_legacy_fixed_layout(name, content) {
                    return Ok(true);
                }
            }
            Ok(Event::Empty(element)) => {
                let local = element.name().local_name().as_ref().to_ascii_lowercase();
                if local != "meta" {
                    continue;
                }
                let attributes = element_attributes(&element)?;
                let name = attribute_value(&attributes, "name");
                let content = attribute_value(&attributes, "content");
                if is_legacy_fixed_layout(name, content) {
                    return Ok(true);
                }
            }
            Ok(Event::Text(event)) => {
                if in_meta {
                    content_text.push_str(event.into_inner().as_ref());
                }
            }
            Ok(Event::End(element)) => {
                if in_meta
                    && element
                        .name()
                        .local_name()
                        .as_ref()
                        .eq_ignore_ascii_case("meta")
                {
                    if property
                        .as_deref()
                        .is_some_and(|value| value.eq_ignore_ascii_case("rendition:layout"))
                        && content_text.trim().eq_ignore_ascii_case("pre-paginated")
                    {
                        return Ok(true);
                    }
                    in_meta = false;
                    content_text.clear();
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(xml_error()),
        }
    }
    Ok(false)
}

fn is_legacy_fixed_layout(name: Option<&str>, content: Option<&str>) -> bool {
    name.is_some_and(|value| value.eq_ignore_ascii_case("fixed-layout"))
        && content.is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

fn display_options_is_fixed_layout(xml: &str) -> Result<bool, EpubError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut option_is_fixed_layout = false;
    let mut text = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                let attributes = element_attributes(&element)?;
                for (name, value) in &attributes {
                    if name.eq_ignore_ascii_case("fixed-layout")
                        && value.eq_ignore_ascii_case("true")
                    {
                        return Ok(true);
                    }
                }
                option_is_fixed_layout = attribute_value(&attributes, "name")
                    .is_some_and(|value| value.eq_ignore_ascii_case("fixed-layout"));
                text.clear();
            }
            Ok(Event::Text(event)) => {
                if option_is_fixed_layout {
                    text.push_str(event.into_inner().as_ref());
                }
            }
            Ok(Event::End(_)) => {
                if option_is_fixed_layout && text.trim().eq_ignore_ascii_case("true") {
                    return Ok(true);
                }
                option_is_fixed_layout = false;
                text.clear();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(xml_error()),
        }
    }
    Ok(false)
}

fn element_attributes(element: &BytesStart<'_>) -> Result<Vec<(String, String)>, EpubError> {
    let mut attributes = Vec::new();
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| xml_error())?;
        let name = attribute.key.local_name().as_ref().to_ascii_lowercase();
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|_| xml_error())?
            .into_owned();
        attributes.push((name, value));
    }
    Ok(attributes)
}

fn attribute_value<'a>(attributes: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn css_has_vertical_writing(css: &str) -> bool {
    let lowercase = css.to_ascii_lowercase();
    let mut search_start = 0_usize;

    while let Some(index) = lowercase[search_start..].find("writing-mode") {
        let after_property = search_start + index + "writing-mode".len();
        let remainder = lowercase[after_property..].trim_start();
        if let Some(after_colon) = remainder.strip_prefix(':') {
            let value = after_colon.trim_start();
            for prefix in ["vertical-rl", "vertical-lr", "sideways-rl", "sideways-lr"] {
                if value.starts_with(prefix) {
                    return true;
                }
            }
        }
        search_start = after_property;
    }
    false
}

fn css_has_remote_resource(css: &str) -> bool {
    let lowercase = css.to_ascii_lowercase();

    let mut rest = lowercase.as_str();
    while let Some(index) = rest.find("url(") {
        let after = &rest[index + 4..];
        let end = after.find(')').unwrap_or(after.len());
        if contains_remote_url(&after[..end]) {
            return true;
        }
        rest = &after[end..];
    }

    let mut rest = lowercase.as_str();
    while let Some(index) = rest.find("@import") {
        let after = &rest[index + 7..];
        let end = after.find(';').unwrap_or(after.len());
        if contains_remote_url(&after[..end]) {
            return true;
        }
        rest = &after[end..];
    }

    false
}

fn contains_remote_url(value: &str) -> bool {
    let lowercase = value.to_ascii_lowercase();
    lowercase.contains("http://") || lowercase.contains("https://")
}

fn is_remote_resource_attribute(tag: &str, attribute: &str) -> bool {
    matches!(
        (tag, attribute),
        ("img", "src")
            | ("img", "srcset")
            | ("link", "href")
            | ("video", "src")
            | ("audio", "src")
            | ("source", "src")
            | ("track", "src")
            | ("video", "poster")
            | ("image", "href")
            | ("use", "href")
            | ("image", "xlink:href")
            | ("use", "xlink:href")
    )
}

fn scan_markup(
    html: &str,
    path: &ResourcePath,
    warnings: &mut Vec<EpubWarning>,
) -> Result<(), EpubError> {
    #[derive(Default)]
    struct ScanState {
        script: bool,
        vertical: bool,
        remote: bool,
    }

    let state = Rc::new(RefCell::new(ScanState::default()));
    let element_state = Rc::clone(&state);
    let text_state = Rc::clone(&state);

    let mut rewriter = HtmlRewriter::new(
        Settings::new()
            .append_element_content_handler(element!("*", move |element| {
                let mut state = element_state.borrow_mut();
                let tag = element.tag_name();
                if matches!(tag.as_str(), "script" | "iframe" | "object") {
                    state.script = true;
                }

                for attribute in element.attributes() {
                    let name = attribute.name().to_ascii_lowercase();
                    let value = attribute.value();
                    if name.starts_with("on") && name.len() > 2 {
                        state.script = true;
                    }
                    if value
                        .trim_start()
                        .to_ascii_lowercase()
                        .starts_with("javascript:")
                    {
                        state.script = true;
                    }
                    if is_remote_resource_attribute(&tag, &name) && contains_remote_url(&value) {
                        state.remote = true;
                    }
                }

                if let Some(style) = element.get_attribute("style") {
                    if css_has_vertical_writing(&style) {
                        state.vertical = true;
                    }
                    if css_has_remote_resource(&style) {
                        state.remote = true;
                    }
                }
                Ok(())
            }))
            .append_element_content_handler(text!("style", move |chunk| {
                let mut state = text_state.borrow_mut();
                let text = chunk.as_str();
                if css_has_vertical_writing(text) {
                    state.vertical = true;
                }
                if css_has_remote_resource(text) {
                    state.remote = true;
                }
                Ok(())
            })),
        |_: &[u8]| {},
    );

    rewriter
        .write(html.as_bytes())
        .map_err(|_| EpubError::new(EpubErrorCode::InvalidOpf))?;
    rewriter
        .end()
        .map_err(|_| EpubError::new(EpubErrorCode::InvalidOpf))?;

    let state = state.borrow();
    if state.script {
        return Err(
            EpubError::new(EpubErrorCode::ScriptDependency).with_resource_path(path.clone())
        );
    }
    if state.vertical {
        return Err(EpubError::new(EpubErrorCode::VerticalWriting).with_resource_path(path.clone()));
    }
    if state.remote {
        warnings.push(remote_warning(path));
    }
    Ok(())
}

fn read_entry(
    archive: &mut zip::ZipArchive<File>,
    name: &str,
) -> Result<Option<String>, EpubError> {
    let found = match archive.by_name(name) {
        Ok(_) => true,
        Err(zip::result::ZipError::FileNotFound) => false,
        Err(_) => return Err(EpubError::new(EpubErrorCode::InvalidZip)),
    };

    let resolved = if found {
        name.to_string()
    } else {
        let decoded = decode_percent(name);
        if decoded == name {
            return Ok(None);
        }
        let found_decoded = match archive.by_name(&decoded) {
            Ok(_) => true,
            Err(zip::result::ZipError::FileNotFound) => false,
            Err(_) => return Err(EpubError::new(EpubErrorCode::InvalidZip)),
        };
        if !found_decoded {
            return Ok(None);
        }
        decoded
    };

    let entry = archive
        .by_name(&resolved)
        .map_err(|_| EpubError::new(EpubErrorCode::InvalidZip))?;
    if entry.size() > MAX_SCAN_BYTES {
        return Err(EpubError::new(EpubErrorCode::LimitExceeded));
    }

    let mut content = String::new();
    entry
        .take(MAX_SCAN_BYTES + 1)
        .read_to_string(&mut content)
        .map_err(|_| EpubError::new(EpubErrorCode::InvalidOpf))?;
    Ok(Some(content))
}

fn decode_percent(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0_usize;

    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            output.push((high << 4) | low);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn xml_error() -> EpubError {
    EpubError::new(EpubErrorCode::XmlParseFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_vertical_writing_variants() {
        assert!(css_has_vertical_writing("writing-mode: vertical-rl;"));
        assert!(css_has_vertical_writing("WRITING-MODE : VERTICAL-LR"));
        assert!(css_has_vertical_writing("-webkit-writing-mode:sideways-rl"));
        assert!(css_has_vertical_writing(
            "p { writing-mode:\n  sideways-lr !important }"
        ));
        assert!(!css_has_vertical_writing("writing-mode: horizontal-tb;"));
        assert!(!css_has_vertical_writing("font-family: serif;"));
    }

    #[test]
    fn detects_css_remote_resources() {
        assert!(css_has_remote_resource(
            "@import url(https://example.com/a.css);"
        ));
        assert!(css_has_remote_resource(
            "@import \"http://example.com/a.css\";"
        ));
        assert!(css_has_remote_resource(
            "body { background: url(HTTPS://example.com/a.png); }"
        ));
        assert!(!css_has_remote_resource(
            "body { background: url(images/a.png); }"
        ));
        assert!(!css_has_remote_resource(
            "body { background: url(data:image/png;base64,AAAA); }"
        ));
    }

    #[test]
    fn recognizes_legacy_fixed_layout_meta() {
        let opf = r#"<package><metadata>
            <meta name="fixed-layout" content="TRUE"/>
        </metadata></package>"#;
        assert!(opf_is_fixed_layout(opf).expect("应解析"));
    }

    #[test]
    fn recognizes_modern_fixed_layout_meta() {
        let opf = r#"<package><metadata>
            <meta property="rendition:layout">pre-paginated</meta>
        </metadata></package>"#;
        assert!(opf_is_fixed_layout(opf).expect("应解析"));
    }

    #[test]
    fn ignores_regular_reflowable_opf() {
        let opf = r#"<package><metadata>
            <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
        </metadata></package>"#;
        assert!(!opf_is_fixed_layout(opf).expect("应解析"));
    }

    #[test]
    fn recognizes_ibooks_display_options() {
        let xml = r#"<display_options>
            <platform name="*">
                <option name="fixed-layout">true</option>
            </platform>
        </display_options>"#;
        assert!(display_options_is_fixed_layout(xml).expect("应解析"));

        let attribute_form = r#"<display_options><option fixed-layout="true"/></display_options>"#;
        assert!(display_options_is_fixed_layout(attribute_form).expect("应解析"));
    }
}
