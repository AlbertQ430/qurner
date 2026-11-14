use epub_core::{EpubError, EpubErrorCode, ResourcePath};

fn path(value: &str) -> ResourcePath {
    ResourcePath::new(value).expect("测试资源路径应有效")
}

#[test]
fn stable_codes_are_exposed() {
    assert_eq!(EpubErrorCode::InvalidZip.as_str(), "invalidZip");
    assert_eq!(EpubErrorCode::MissingContainer.as_str(), "missingContainer");
    assert_eq!(EpubErrorCode::MissingOpf.as_str(), "missingOpf");
    assert_eq!(EpubErrorCode::InvalidOpf.as_str(), "invalidOpf");
    assert_eq!(EpubErrorCode::XmlParseFailed.as_str(), "xmlParseFailed");
    assert_eq!(EpubErrorCode::EncryptedContent.as_str(), "encryptedContent");
    assert_eq!(EpubErrorCode::FixedLayout.as_str(), "fixedLayout");
    assert_eq!(EpubErrorCode::VerticalWriting.as_str(), "verticalWriting");
    assert_eq!(EpubErrorCode::ScriptDependency.as_str(), "scriptDependency");
    assert_eq!(
        EpubErrorCode::UnsafeResourcePath.as_str(),
        "unsafeResourcePath"
    );
    assert_eq!(EpubErrorCode::LimitExceeded.as_str(), "limitExceeded");
    assert_eq!(EpubErrorCode::IoError.as_str(), "ioError");
}

#[test]
fn error_codes_serialize_as_camel_case() {
    assert_eq!(
        serde_json::to_string(&EpubErrorCode::MissingContainer).expect("序列化应成功"),
        "\"missingContainer\""
    );
    assert_eq!(
        serde_json::to_string(&EpubErrorCode::UnsafeResourcePath).expect("序列化应成功"),
        "\"unsafeResourcePath\""
    );
}

#[test]
fn display_contains_relative_structure_path_and_message() {
    let error = EpubError::new(EpubErrorCode::MissingContainer)
        .with_resource_path(path("META-INF/container.xml"));

    let message = error.to_string();
    assert!(message.contains("missingContainer"));
    assert!(message.contains("缺少 META-INF/container.xml"));
    assert!(message.contains("META-INF/container.xml"));
    assert!(!message.contains("/Users/"));
}

#[test]
fn display_for_unsupported_format_variants() {
    let fixed_layout = EpubError::new(EpubErrorCode::FixedLayout);
    assert!(fixed_layout.to_string().contains("固定版式"));

    let encrypted = EpubError::new(EpubErrorCode::EncryptedContent);
    assert!(encrypted.to_string().contains("加密"));

    let vertical = EpubError::new(EpubErrorCode::VerticalWriting);
    assert!(vertical.to_string().contains("竖排"));

    let script = EpubError::new(EpubErrorCode::ScriptDependency);
    assert!(script.to_string().contains("脚本"));
}

#[test]
fn io_error_does_not_carry_paths() {
    let error = EpubError::io();
    assert_eq!(error.code(), EpubErrorCode::IoError);
    assert!(error.resource_path().is_none());
    assert!(!error.to_string().contains("/Users/"));
}
