//! 安全解析 `META-INF/encryption.xml`。
//!
//! 使用 XML 解析器读取 `CipherReference` 的 `URI` 属性，正确忽略命名空间前缀、
//! 属性顺序和引号风格；不做字符串搜索。

use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::Event;

use crate::{EpubError, EpubErrorCode, ResourcePath};

/// 从 `encryption.xml` 提取并规范化为归档内相对路径的 `CipherReference` URI。
pub(crate) fn cipher_reference_uris(xml: &str) -> Result<Vec<ResourcePath>, EpubError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut uris = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                if element.name().local_name().as_ref() != "CipherReference" {
                    continue;
                }
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|_| xml_error())?;
                    if attribute.key.local_name().as_ref() != "URI" {
                        continue;
                    }
                    let value = attribute
                        .normalized_value(XmlVersion::Implicit1_0)
                        .map_err(|_| xml_error())?;
                    let relative = value.trim().strip_prefix('/').unwrap_or(value.trim());
                    uris.push(
                        ResourcePath::new(relative)
                            .map_err(|_| EpubError::new(EpubErrorCode::UnsafeResourcePath))?,
                    );
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(xml_error()),
        }
    }

    Ok(uris)
}

/// 判断加密资源是否为字体；字体加密允许回退到系统字体。
pub(crate) fn is_font_path(path: &str) -> bool {
    let lowercase = path.to_ascii_lowercase();
    [".otf", ".ttf", ".woff", ".woff2"]
        .iter()
        .any(|extension| lowercase.ends_with(extension))
}

fn xml_error() -> EpubError {
    EpubError::new(EpubErrorCode::XmlParseFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_attribute_orders_quotes_and_namespace_prefixes() {
        let documents = [
            r#"<?xml version="1.0"?>
<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container"
            xmlns:enc="http://www.w3.org/2001/04/xmlenc#">
  <enc:EncryptedData>
    <enc:CipherData>
      <enc:CipherReference URI="OEBPS/chapter1.xhtml"/>
    </enc:CipherData>
  </enc:EncryptedData>
</encryption>"#,
            r#"<?xml version="1.0"?>
<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container"
            xmlns:x="http://www.w3.org/2001/04/xmlenc#">
  <x:EncryptedData>
    <x:CipherData>
      <x:CipherReference xmlns:foo="urn:example" foo:name="value" URI='OEBPS/chapter1.xhtml'/>
    </x:CipherData>
  </x:EncryptedData>
</encryption>"#,
            r#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <EncryptedData xmlns="http://www.w3.org/2001/04/xmlenc#">
    <CipherData>
      <CipherReference URI = "OEBPS/chapter1.xhtml" />
    </CipherData>
  </EncryptedData>
</encryption>"#,
        ];

        for document in documents {
            let uris = cipher_reference_uris(document).expect("应可解析");
            assert_eq!(uris.len(), 1);
            assert_eq!(uris[0].as_str(), "OEBPS/chapter1.xhtml");
        }
    }

    #[test]
    fn normalizes_leading_slash_and_rejects_traversal() {
        let xml = r#"<encryption><CipherReference URI="/OEBPS/fonts/a.woff"/></encryption>"#;
        let uris = cipher_reference_uris(xml).expect("应可解析");
        assert_eq!(uris[0].as_str(), "OEBPS/fonts/a.woff");

        let xml = r#"<encryption><CipherReference URI="../outside.xhtml"/></encryption>"#;
        assert_eq!(
            cipher_reference_uris(xml)
                .expect_err("应拒绝安全路径")
                .code(),
            EpubErrorCode::UnsafeResourcePath
        );
    }

    #[test]
    fn detects_font_extensions() {
        assert!(is_font_path("OEBPS/fonts/a.OTF"));
        assert!(is_font_path("OEBPS/fonts/a.woff2"));
        assert!(!is_font_path("OEBPS/chapter1.xhtml"));
        assert!(!is_font_path("OEBPS/style.css"));
    }
}
