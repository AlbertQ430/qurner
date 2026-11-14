//! 阅读 HTML 运行时安全改写。
//!
//! 使用 `lol_html` 流式移除主动内容，并注入唯一可信阅读脚本标签。

use std::cell::Cell;
use std::rc::Rc;

use lol_html::html_content::{ContentType, Element};
use lol_html::{HtmlRewriter, Settings, element, end_tag};

/// 注入脚本所需的会话信息。
pub struct RewriteConfig<'a> {
    pub nonce: &'a str,
    pub token: &'a str,
    pub session_id: &'a str,
    /// 当前文档在 spine 中的位置；`None` 表示非 spine 文档，不注入引擎。
    pub spine_index: Option<u32>,
    pub spine_href: Option<&'a str>,
}

/// 移除主动内容并注入可信阅读脚本；失败返回 `None`。
pub fn rewrite_document(input: &str, config: &RewriteConfig<'_>) -> Option<String> {
    let spine = config
        .spine_index
        .zip(config.spine_href.map(str::to_string));
    let inject = spine.is_some();
    let script = match spine {
        Some((index, href)) => format!(
            "<script nonce=\"{}\" src=\"epub://{}/__qurner/reader-engine.js\" data-session-id=\"{}\" data-spine-index=\"{}\" data-spine-href=\"{}\"></script>",
            escape_attribute(config.nonce),
            escape_attribute(config.token),
            escape_attribute(config.session_id),
            index,
            escape_attribute(&href),
        ),
        None => String::new(),
    };

    let mut output = Vec::new();
    let injected = Rc::new(Cell::new(false));
    let head_script = script.clone();
    let head_flag = Rc::clone(&injected);
    let body_script = script.clone();
    let body_flag = Rc::clone(&injected);

    let mut settings = Settings::new()
        .append_element_content_handler(element!("script", |element| {
            element.remove();
            Ok(())
        }))
        .append_element_content_handler(element!("iframe", |element| {
            element.remove();
            Ok(())
        }))
        .append_element_content_handler(element!("object", |element| {
            element.remove();
            Ok(())
        }))
        .append_element_content_handler(element!("embed", |element| {
            element.remove();
            Ok(())
        }))
        .append_element_content_handler(element!("base", |element| {
            element.remove();
            Ok(())
        }))
        .append_element_content_handler(element!("meta", |element| {
            if element
                .get_attribute("http-equiv")
                .map(|value| value.trim().eq_ignore_ascii_case("refresh"))
                .unwrap_or(false)
            {
                element.remove();
            }
            Ok(())
        }))
        .append_element_content_handler(element!("*", |element| {
            sanitize_attributes(element);
            Ok(())
        }));

    if inject {
        settings = settings
            .append_element_content_handler(element!("head", move |element| {
                let script = head_script.clone();
                let flag = Rc::clone(&head_flag);
                element.on_end_tag(end_tag!(move |end| {
                    if !flag.get() {
                        end.after(&script, ContentType::Html);
                        flag.set(true);
                    }
                    Ok(())
                }))?;
                Ok(())
            }))
            .append_element_content_handler(element!("body", move |element| {
                if !body_flag.get() {
                    element.prepend(&body_script, ContentType::Html);
                    body_flag.set(true);
                }
                Ok(())
            }));
    }

    let mut rewriter = HtmlRewriter::new(settings, |chunk: &[u8]| output.extend_from_slice(chunk));
    rewriter.write(input.as_bytes()).ok()?;
    rewriter.end().ok()?;

    let mut result = String::from_utf8(output).ok()?;
    // 既无 head 也无 body 的 spine 片段：把唯一脚本放在片段开头。
    if inject && !injected.get() {
        result = format!("{script}{result}");
    }
    Some(result)
}

fn sanitize_attributes(element: &mut Element<'_, '_>) {
    let tag = element.tag_name();
    let attributes: Vec<(String, String)> = element
        .attributes()
        .iter()
        .map(|attribute| (attribute.name().to_string(), attribute.value()))
        .collect();

    for (name, value) in attributes {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("on") && lower.len() > 2 {
            element.remove_attribute(&name);
            continue;
        }
        if lower == "ping" {
            element.remove_attribute(&name);
            continue;
        }
        if lower == "xlink:href" {
            let normalized = strip_control_whitespace(&value);
            if has_url_scheme(&value) || normalized.trim_start().starts_with("//") {
                element.remove_attribute(&name);
            }
            continue;
        }
        if lower == "href" {
            let normalized = strip_control_whitespace(&value);
            if has_url_scheme(&value) || normalized.trim_start().starts_with("//") {
                element.remove_attribute(&name);
                if tag == "a" || tag == "area" {
                    let _ = element.set_attribute("data-qurner-external", "blocked");
                }
            }
            continue;
        }
        if matches!(lower.as_str(), "action" | "formaction") {
            let normalized = strip_control_whitespace(&value);
            if has_url_scheme(&value) || normalized.trim_start().starts_with("//") {
                element.remove_attribute(&name);
            }
            continue;
        }
        if matches!(lower.as_str(), "src" | "poster") {
            let normalized = strip_control_whitespace(&value)
                .trim_start()
                .to_ascii_lowercase();
            if normalized.starts_with("javascript:")
                || normalized.starts_with("vbscript:")
                || normalized.starts_with("data:text/html")
            {
                element.remove_attribute(&name);
            }
        }
    }
}

/// 移除 URL 解析会忽略的嵌入制表符/换行/回车，避免 scheme 混淆绕过。
fn strip_control_whitespace(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '\t' | '\n' | '\r'))
        .collect()
}

/// 任何带 scheme 的绝对 URL（http、https、mailto、自定义 scheme 等）。
fn has_url_scheme(value: &str) -> bool {
    let normalized = strip_control_whitespace(value);
    let trimmed = normalized.trim_start();
    let Some(colon) = trimmed.find(':') else {
        return false;
    };
    let scheme = &trimmed[..colon];
    !scheme.is_empty()
        && scheme.starts_with(|character: char| character.is_ascii_alphabetic())
        && scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
