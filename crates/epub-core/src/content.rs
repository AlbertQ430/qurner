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

/// 单个小资源（OPF/CSS/display-options）允许读取的最大字节数。
const MAX_SCAN_BYTES: u64 = 16 * 1024 * 1024;
/// 单个 XHTML/SVG/HTML 正文允许流式扫描的最大字节数（与 precheck 单条目上限一致）。
const MAX_MARKUP_SCAN_BYTES: u64 = 200 * 1024 * 1024;
/// 流式扫描的分块大小；不把整章读入内存。
const MARKUP_SCAN_CHUNK_BYTES: usize = 64 * 1024;

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
            scan_markup_entry(&mut archive, &item.href, warnings)?;
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

/// 流式 CSS 检测状态：有界（O(1)）且可跨任意片段/任意长度空白。
#[derive(Debug, Default)]
struct CssStreamScanner {
    mode: CssMode,
    buffer: String,
    comment: bool,
    comment_return: CssMode,
    comment_previous: Option<char>,
    pending_slash: bool,
    slash_mode: CssMode,
    string_quote: Option<char>,
    string_escaped: bool,
    string_return: CssMode,
    string_scheme: bool,
    pub(crate) vertical: bool,
    pub(crate) remote: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum CssMode {
    #[default]
    Idle,
    AfterWritingMode,
    AfterColon,
    ValuePrefix,
    AfterUrlParen,
    SchemePrefix,
    AfterImport,
}

impl CssStreamScanner {
    fn feed(&mut self, text: &str) {
        for character in text.chars() {
            self.feed_char(character);
        }
    }

    fn enter_string(&mut self, quote: char, return_mode: CssMode, scheme: bool) {
        self.string_quote = Some(quote);
        self.string_escaped = false;
        self.string_return = return_mode;
        self.string_scheme = scheme;
        self.pending_slash = false;
        self.buffer.clear();
    }

    fn feed_scheme_in_string(&mut self, character: char) {
        let lower = character.to_ascii_lowercase();
        if character.is_whitespace() || !lower.is_ascii() {
            self.buffer.clear();
            return;
        }
        self.buffer.push(lower);
        let targets = ["http://", "https://"];
        if targets
            .iter()
            .any(|target| target.starts_with(self.buffer.as_str()))
        {
            if targets.iter().any(|target| *target == self.buffer) {
                self.remote = true;
                self.buffer.clear();
            }
        } else {
            self.buffer.clear();
        }
    }

    fn feed_char(&mut self, character: char) {
        if let Some(quote) = self.string_quote {
            if self.string_escaped {
                self.string_escaped = false;
                return;
            }
            if character == '\\' {
                self.string_escaped = true;
                return;
            }
            if character == quote {
                self.string_quote = None;
                self.mode = self.string_return;
                self.buffer.clear();
                return;
            }
            if self.string_scheme {
                self.feed_scheme_in_string(character);
            }
            return;
        }
        if self.comment {
            if self.comment_previous == Some('*') && character == '/' {
                self.comment = false;
                self.mode = self.comment_return;
            }
            self.comment_previous = Some(character);
            return;
        }
        if self.pending_slash {
            self.pending_slash = false;
            if character == '*' {
                self.comment = true;
                self.comment_return = self.slash_mode;
                self.comment_previous = Some(character);
                self.buffer.clear();
                return;
            }
            let mode = self.slash_mode;
            self.mode = mode;
            self.process_char('/', false);
        }
        if character == '/' {
            self.pending_slash = true;
            self.slash_mode = self.mode;
            return;
        }
        self.process_char(character, true);
    }

    fn process_char(&mut self, character: char, _allow_slash: bool) {
        let lower = character.to_ascii_lowercase();
        match self.mode {
            CssMode::Idle => {
                if character == '\'' || character == '"' {
                    self.enter_string(character, CssMode::Idle, false);
                    return;
                }
                if character.is_whitespace() || !lower.is_ascii() {
                    self.buffer.clear();
                } else {
                    self.buffer.push(lower);
                    if self.buffer.len() > 24 {
                        let excess = self.buffer.len() - 24;
                        self.buffer.drain(..excess);
                    }
                }
                if self.buffer.ends_with("writing-mode") {
                    self.mode = CssMode::AfterWritingMode;
                    self.buffer.clear();
                } else if self.buffer.ends_with("url(") {
                    self.mode = CssMode::AfterUrlParen;
                    self.buffer.clear();
                } else if self.buffer.ends_with("@import") {
                    self.mode = CssMode::AfterImport;
                    self.buffer.clear();
                }
            }
            CssMode::AfterWritingMode => {
                if character == '\'' || character == '"' {
                    self.enter_string(character, CssMode::AfterWritingMode, false);
                    return;
                }
                if character.is_whitespace() {
                    return;
                }
                if character == ':' {
                    self.mode = CssMode::AfterColon;
                    self.buffer.clear();
                } else {
                    self.mode = CssMode::Idle;
                    self.process_char(character, _allow_slash);
                }
            }
            CssMode::AfterColon | CssMode::AfterUrlParen | CssMode::AfterImport => {
                if character == '\'' || character == '"' {
                    let scheme = self.mode != CssMode::AfterColon;
                    self.enter_string(character, self.mode, scheme);
                    return;
                }
                if character.is_whitespace() {
                    return;
                }
                self.mode = if self.mode == CssMode::AfterColon {
                    CssMode::ValuePrefix
                } else {
                    CssMode::SchemePrefix
                };
                self.buffer.clear();
                self.process_char(character, _allow_slash);
            }
            CssMode::ValuePrefix => {
                if character == '\'' || character == '"' {
                    self.enter_string(character, CssMode::ValuePrefix, false);
                    return;
                }
                if character.is_whitespace() || matches!(character, ';' | '{' | '}' | ',') {
                    self.mode = CssMode::Idle;
                    if !character.is_whitespace() {
                        self.process_char(character, _allow_slash);
                    }
                    return;
                }
                if !lower.is_ascii() {
                    self.mode = CssMode::Idle;
                    self.buffer.clear();
                    return;
                }
                self.buffer.push(lower);
                let targets = ["vertical-rl", "vertical-lr", "sideways-rl", "sideways-lr"];
                if targets
                    .iter()
                    .any(|target| target.starts_with(self.buffer.as_str()))
                {
                    if targets.iter().any(|target| *target == self.buffer) {
                        self.vertical = true;
                        self.mode = CssMode::Idle;
                        self.buffer.clear();
                    }
                } else {
                    self.mode = CssMode::Idle;
                    self.process_char(character, _allow_slash);
                }
            }
            CssMode::SchemePrefix => {
                if character == '\'' || character == '"' {
                    self.enter_string(character, CssMode::SchemePrefix, true);
                    return;
                }
                if character.is_whitespace() || matches!(character, ';' | ')') {
                    self.mode = CssMode::Idle;
                    if !character.is_whitespace() && character != ')' {
                        self.process_char(character, _allow_slash);
                    }
                    return;
                }
                if !lower.is_ascii() {
                    self.mode = CssMode::Idle;
                    self.buffer.clear();
                    return;
                }
                self.buffer.push(lower);
                let targets = ["http://", "https://"];
                if targets
                    .iter()
                    .any(|target| target.starts_with(self.buffer.as_str()))
                {
                    if targets.iter().any(|target| *target == self.buffer) {
                        self.remote = true;
                        self.mode = CssMode::Idle;
                        self.buffer.clear();
                    }
                } else {
                    self.mode = CssMode::Idle;
                    self.process_char(character, _allow_slash);
                }
            }
        }
    }
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

fn scan_markup_entry(
    archive: &mut zip::ZipArchive<File>,
    path: &ResourcePath,
    warnings: &mut Vec<EpubWarning>,
) -> Result<(), EpubError> {
    let resolved =
        resolve_entry_name(archive, path.as_str())?.ok_or_else(|| missing_resource(path))?;
    let entry = archive
        .by_name(&resolved)
        .map_err(|_| EpubError::new(EpubErrorCode::InvalidZip))?;
    if entry.size() > MAX_MARKUP_SCAN_BYTES {
        return Err(EpubError::new(EpubErrorCode::LimitExceeded));
    }
    scan_markup_reader(entry.take(MAX_MARKUP_SCAN_BYTES + 1), path, warnings)
}

/// 流式扫描章节标记；CSS 状态按 `<style>` 元素隔离并跨片段保持。
fn scan_markup_reader<R: std::io::Read>(
    mut reader: R,
    path: &ResourcePath,
    warnings: &mut Vec<EpubWarning>,
) -> Result<(), EpubError> {
    #[derive(Default)]
    struct ScanState {
        script: bool,
        vertical: bool,
        remote: bool,
        css: CssStreamScanner,
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
                state.css.feed(chunk.as_str());
                if chunk.last_in_text_node() {
                    state.vertical |= state.css.vertical;
                    state.remote |= state.css.remote;
                    state.css = CssStreamScanner::default();
                }
                Ok(())
            })),
        |_: &[u8]| {},
    );

    let mut buffer = vec![0_u8; MARKUP_SCAN_CHUNK_BYTES];
    let mut total = 0_u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| EpubError::new(EpubErrorCode::InvalidOpf))?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > MAX_MARKUP_SCAN_BYTES {
            return Err(EpubError::new(EpubErrorCode::LimitExceeded));
        }
        rewriter
            .write(&buffer[..read])
            .map_err(|_| EpubError::new(EpubErrorCode::InvalidOpf))?;
    }
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

/// 测试/兼容入口：对完整文本执行同一套标记检查。
#[cfg(test)]
fn scan_markup(
    html: &str,
    path: &ResourcePath,
    warnings: &mut Vec<EpubWarning>,
) -> Result<(), EpubError> {
    scan_markup_reader(html.as_bytes(), path, warnings)
}

fn resolve_entry_name(
    archive: &mut zip::ZipArchive<File>,
    name: &str,
) -> Result<Option<String>, EpubError> {
    let found = archive.by_name(name).is_ok();
    if found {
        return Ok(Some(name.to_string()));
    }
    match archive.by_name(name) {
        Err(zip::result::ZipError::FileNotFound) => {}
        Err(_) => return Err(EpubError::new(EpubErrorCode::InvalidZip)),
        Ok(_) => unreachable!(),
    }
    let decoded = decode_percent(name);
    if decoded == name {
        return Ok(None);
    }
    let found_decoded = archive.by_name(&decoded).is_ok();
    if found_decoded {
        return Ok(Some(decoded));
    }
    match archive.by_name(&decoded) {
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(_) => Err(EpubError::new(EpubErrorCode::InvalidZip)),
        Ok(_) => unreachable!(),
    }
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

    fn markup_path() -> ResourcePath {
        ResourcePath::new("OEBPS/chapter1.xhtml").expect("path")
    }

    #[test]
    fn scans_100mb_single_xhtml_epub() {
        use std::io::Write as _;

        let root = std::env::temp_dir().join(format!(
            "qurner-m5r4-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("dir");
        let path = root.join("book-100mb.epub");
        let text = "a".repeat(100 * 1024 * 1024);
        let chapter = format!(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>{text}</p></body></html>"
        );
        {
            let file = std::fs::File::create(&path).expect("create");
            let mut writer = zip::ZipWriter::new(file);
            let stored = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            let deflated = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            writer.start_file("mimetype", stored).expect("mimetype");
            writer
                .write_all(b"application/epub+zip")
                .expect("mimetype data");
            writer
                .start_file("META-INF/container.xml", deflated)
                .expect("container");
            writer
                .write_all(
                    b"<?xml version=\"1.0\"?><container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>",
                )
                .expect("container data");
            writer
                .start_file("OEBPS/content.opf", deflated)
                .expect("opf");
            writer
                .write_all(
                    b"<?xml version=\"1.0\"?><package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"id\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>100MB Text</dc:title><dc:identifier id=\"id\">urn:uuid:m5r4</dc:identifier><meta property=\"dcterms:modified\">2026-11-16T00:00:00Z</meta></metadata><manifest><item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/><item id=\"c1\" href=\"chapter1.xhtml\" media-type=\"application/xhtml+xml\"/></manifest><spine><itemref idref=\"c1\"/></spine></package>",
                )
                .expect("opf data");
            writer.start_file("OEBPS/nav.xhtml", deflated).expect("nav");
            writer
                .write_all(
                    b"<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><body><nav epub:type=\"toc\"><ol><li><a href=\"chapter1.xhtml\">Big Chapter</a></li></ol></nav></body></html>",
                )
                .expect("nav data");
            writer
                .start_file("OEBPS/chapter1.xhtml", stored)
                .expect("chapter");
            writer.write_all(chapter.as_bytes()).expect("chapter data");
            writer.finish().expect("finish");
        }

        let before = crate::fingerprint_file(&path).expect("hash before");
        let book = crate::parse_epub(&path).expect("100MB 单 XHTML 应可解析");
        assert_eq!(book.spine.len(), 1);
        let after = crate::fingerprint_file(&path).expect("hash after");
        assert_eq!(before, after, "导入前后原文 SHA-256 不变");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_markup_beyond_scan_limit() {
        let path = markup_path();
        let mut warnings = Vec::new();
        let reader = std::io::repeat(b'a').take(MAX_MARKUP_SCAN_BYTES + 1);
        let error = scan_markup_reader(reader, &path, &mut warnings).expect_err("应拒绝超大标记");
        assert_eq!(error.code(), EpubErrorCode::LimitExceeded);
    }

    struct ChunkedReader {
        data: Vec<u8>,
        position: usize,
    }

    impl std::io::Read for ChunkedReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.position >= self.data.len() {
                return Ok(0);
            }
            let remaining = self.data.len() - self.position;
            let count = remaining.min(MARKUP_SCAN_CHUNK_BYTES).min(buffer.len());
            buffer[..count].copy_from_slice(&self.data[self.position..self.position + count]);
            self.position += count;
            Ok(count)
        }
    }

    fn chunked(html: &str) -> ChunkedReader {
        ChunkedReader {
            data: html.as_bytes().to_vec(),
            position: 0,
        }
    }

    #[test]
    fn detects_vertical_writing_across_chunk_boundary() {
        let fill = MARKUP_SCAN_CHUNK_BYTES - "<style>".len() - 15;
        let html = format!(
            "<style>{}writing-mode:vertical-rl</style>",
            "a".repeat(fill)
        );
        let mut warnings = Vec::new();
        let error = scan_markup_reader(chunked(&html), &markup_path(), &mut warnings)
            .expect_err("跨片段竖排应被拒绝");
        assert_eq!(error.code(), EpubErrorCode::VerticalWriting);
    }

    #[test]
    fn detects_vertical_writing_after_long_whitespace() {
        let fill = MARKUP_SCAN_CHUNK_BYTES - "<style>".len() - 12;
        let html = format!(
            "<style>{}writing-mode:{}vertical-rl</style>",
            "a".repeat(fill),
            " ".repeat(2_000)
        );
        let mut warnings = Vec::new();
        let error = scan_markup_reader(chunked(&html), &markup_path(), &mut warnings)
            .expect_err("超长空白后的竖排应被拒绝");
        assert_eq!(error.code(), EpubErrorCode::VerticalWriting);
    }

    #[test]
    fn detects_remote_after_long_whitespace() {
        let fill = MARKUP_SCAN_CHUNK_BYTES - "<style>".len() - 6;
        let html = format!(
            "<style>{}url({}https://example.com/a.png)</style>",
            "a".repeat(fill),
            " ".repeat(2_000)
        );
        let mut warnings = Vec::new();
        scan_markup_reader(chunked(&html), &markup_path(), &mut warnings)
            .expect("超长空白后的远程资源应只警告");
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
        );
    }

    #[test]
    fn handles_non_ascii_css_identifiers_without_panic() {
        let html = "<style>a中aaaaaaaaaaaaaaaaaaaa{}中{color:red}</style>";
        let mut warnings = Vec::new();
        scan_markup(html, &markup_path(), &mut warnings)
            .expect("合法中文 CSS 标识符不得 panic 或误报");
        assert!(warnings.is_empty());
    }

    #[test]
    fn detects_vertical_writing_after_css_string_comment_marker() {
        let html = "<style>p:before{content:\"/*\";}p{writing-mode:vertical-rl}</style>";
        let mut warnings = Vec::new();
        let error = scan_markup(html, &markup_path(), &mut warnings)
            .expect_err("字符串中的 /* 不得遮蔽竖排检测");
        assert_eq!(error.code(), EpubErrorCode::VerticalWriting);
    }

    #[test]
    fn detects_vertical_writing_after_css_string_close_marker() {
        let html = "<style>p:before{content:\"*/\";}p{writing-mode:vertical-rl}</style>";
        let mut warnings = Vec::new();
        let error = scan_markup(html, &markup_path(), &mut warnings)
            .expect_err("字符串中的 */ 不得遮蔽竖排检测");
        assert_eq!(error.code(), EpubErrorCode::VerticalWriting);
    }

    #[test]
    fn handles_escaped_quote_in_css_string() {
        let html = "<style>p:before{content:\"a\\\"/*\";}p{writing-mode:vertical-rl}</style>";
        let mut warnings = Vec::new();
        let error =
            scan_markup(html, &markup_path(), &mut warnings).expect_err("转义引号后的竖排应被拒绝");
        assert_eq!(error.code(), EpubErrorCode::VerticalWriting);
    }

    #[test]
    fn detects_vertical_writing_after_string_across_chunk_boundary() {
        let prefix = "<style>p:before{content:\"";
        let fill = MARKUP_SCAN_CHUNK_BYTES - prefix.len() - 2;
        let html = format!(
            "{}${}*/\";}}p{{writing-mode:vertical-rl}}</style>",
            prefix,
            "a".repeat(fill)
        );
        let mut warnings = Vec::new();
        let error = scan_markup_reader(chunked(&html), &markup_path(), &mut warnings)
            .expect_err("跨片段字符串后的竖排应被拒绝");
        assert_eq!(error.code(), EpubErrorCode::VerticalWriting);
    }

    #[test]
    fn detects_quoted_remote_urls() {
        for html in [
            "<style>p{background:url(\"https://example.com/a.png\")}</style>",
            "<style>p{background:url('https://example.com/a.png')}</style>",
            "<style>@import \"https://example.com/a.css\";</style>",
            "<style>@import 'https://example.com/a.css';</style>",
        ] {
            let mut warnings = Vec::new();
            scan_markup(html, &markup_path(), &mut warnings).expect("引号内远程应只警告");
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked),
                "{html}"
            );
        }
    }

    #[test]
    fn detects_remote_url_after_css_comment() {
        let html = "<style>p{background:url(/**/https://example.com/x.png)}</style>";
        let mut warnings = Vec::new();
        scan_markup(html, &markup_path(), &mut warnings).expect("注释后的远程资源应只警告");
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
        );
    }

    #[test]
    fn detects_import_remote_after_css_comment() {
        let html = "<style>@import/**/\"https://example.com/a.css\";</style>";
        let mut warnings = Vec::new();
        scan_markup(html, &markup_path(), &mut warnings).expect("@import 注释后的远程资源应只警告");
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
        );
    }

    #[test]
    fn detects_vertical_writing_after_css_comment() {
        let html = "<style>p{writing-mode:/**/vertical-rl}</style>";
        let mut warnings = Vec::new();
        let error =
            scan_markup(html, &markup_path(), &mut warnings).expect_err("注释后的竖排应被拒绝");
        assert_eq!(error.code(), EpubErrorCode::VerticalWriting);
    }

    #[test]
    fn resets_css_state_between_style_elements() {
        let html = "<style>writing-mode:</style><style> vertical-rl </style>";
        let mut warnings = Vec::new();
        scan_markup(html, &markup_path(), &mut warnings).expect("相邻 style 元素不得串状态");
        assert!(warnings.is_empty());
    }

    #[test]
    fn detects_remote_resource_across_chunk_boundary() {
        let fill = MARKUP_SCAN_CHUNK_BYTES - "<style>".len() - 20;
        let html = format!(
            "<style>{}url(https://example.com/a.png)</style>",
            "a".repeat(fill)
        );
        let mut warnings = Vec::new();
        scan_markup_reader(chunked(&html), &markup_path(), &mut warnings)
            .expect("跨片段远程资源应只警告");
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
        );
    }

    #[test]
    fn keeps_markup_security_checks() {
        let path = markup_path();
        for (html, code) in [
            ("<script>alert(1)</script>", EpubErrorCode::ScriptDependency),
            (
                "<iframe src=\"x\"></iframe>",
                EpubErrorCode::ScriptDependency,
            ),
            (
                "<object data=\"x\"></object>",
                EpubErrorCode::ScriptDependency,
            ),
            ("<p onclick=\"x()\">a</p>", EpubErrorCode::ScriptDependency),
            (
                "<a href=\"javascript:alert(1)\">a</a>",
                EpubErrorCode::ScriptDependency,
            ),
            (
                "<p style=\"writing-mode: vertical-rl\">a</p>",
                EpubErrorCode::VerticalWriting,
            ),
            (
                "<style>p { writing-mode: vertical-lr }</style>",
                EpubErrorCode::VerticalWriting,
            ),
        ] {
            let mut warnings = Vec::new();
            let error = scan_markup(html, &path, &mut warnings).expect_err(html);
            assert_eq!(error.code(), code, "{html}");
        }

        let mut warnings = Vec::new();
        scan_markup(
            "<img src=\"https://example.com/a.png\"/>",
            &path,
            &mut warnings,
        )
        .expect("远程资源只警告");
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == EpubWarningCode::RemoteResourceBlocked)
        );

        let mut warnings = Vec::new();
        scan_markup("<p>正常正文</p>", &path, &mut warnings).expect("普通正文通过");
        assert!(warnings.is_empty());
    }
}
