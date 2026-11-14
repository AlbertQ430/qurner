//! M4B dialog 注册验证。
//!
//! 只检查注册源码与 capability，不启动真实系统对话框（由 M4D 冒烟覆盖）。

use std::path::Path;

fn read(relative: &str) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(manifest_dir.join(relative)).expect("read source")
}

#[test]
fn dialog_plugin_and_select_command_are_registered() {
    let lib = read("src/lib.rs");
    assert!(
        lib.contains("tauri_plugin_dialog::init()"),
        "应注册 dialog 插件"
    );
    assert!(
        lib.contains("commands::select_import_files"),
        "应在 invoke_handler 注册 selectImportFiles"
    );

    let commands = read("src/commands.rs");
    assert!(
        commands.contains("rename = \"selectImportFiles\""),
        "命令名必须是 selectImportFiles"
    );
    assert!(
        commands.contains("add_filter(\"EPUB\", &[\"epub\"])"),
        "只显示 epub 文件过滤项"
    );
    assert!(
        commands.contains("blocking_pick_files()"),
        "应使用原生多选文件对话框"
    );
    assert!(
        commands.contains("spawn_blocking"),
        "阻塞式 dialog 必须放到 blocking 任务"
    );
    assert!(
        commands.contains("validate_selected_files"),
        "选择结果必须校验"
    );
}

#[test]
fn dialog_capability_stays_core_default_only() {
    let capabilities = read("capabilities/default.json");
    let value: serde_json::Value = serde_json::from_str(&capabilities).expect("capabilities JSON");
    assert_eq!(value["permissions"], serde_json::json!(["core:default"]));
    assert!(
        !capabilities.contains("dialog:"),
        "capability 不得放开 dialog:*"
    );
}

#[test]
fn dialog_command_does_not_scan_directories_or_log_paths() {
    let commands = read("src/commands.rs");
    for needle in [
        "read_dir",
        "WalkDir",
        "fs::read",
        "std::fs::metadata",
        "tracing::info!(path",
        "tracing::error!(path",
    ] {
        assert!(!commands.contains(needle), "不得出现 {needle}");
    }
}
