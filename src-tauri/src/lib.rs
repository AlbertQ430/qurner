//! Qurner Tauri 应用入口。
//!
//! M0 只提供最小应用壳：初始化 tracing 日志、启动 Tauri 窗口。
//! 命令层、协议层和业务服务在后续任务按 SDS 增加。

use tracing_subscriber::EnvFilter;

/// 启动 Qurner 应用。
pub fn run() {
    if let Err(error) = init_tracing() {
        eprintln!("Qurner 日志初始化失败：{error}");
        std::process::exit(1);
    }

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "Qurner 启动",);

    if let Err(error) = build_app().run(tauri::generate_context!()) {
        tracing::error!(%error, "Qurner 运行失败");
        std::process::exit(1);
    }
}

fn build_app() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default().setup(|_app| {
        tracing::info!(
            epub_core = epub_core::CRATE_NAME,
            storage_core = storage_core::CRATE_NAME,
            reader_model = reader_model::CRATE_NAME,
            "领域 crate 已链接"
        );
        Ok(())
    })
}

fn init_tracing() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("qurner=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(true)
        .with_ansi(false)
        .try_init()?;
    Ok(())
}
