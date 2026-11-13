//! Qurner 阅读模型。
//!
//! M0 只建立可独立编译和测试的 crate 边界。Locator、阅读进度和
//! 设置模型在后续任务实现，本 crate 目前不提供业务占位接口。

#![forbid(unsafe_code)]

/// crate 名称，用于冒烟测试确认 workspace 能编译并测试本包。
pub const CRATE_NAME: &str = "reader-model";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_is_wired_into_workspace() {
        assert_eq!(CRATE_NAME, "reader-model");
    }
}
