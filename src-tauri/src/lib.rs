//! Qurner Tauri 应用入口。
//!
//! M2D 将 library-core 接入 Tauri：启动时初始化应用数据目录与数据库，暴露
//! startImport / listBooks / softRemoveBook 三个命令及导入事件。

pub mod commands;
pub mod dto;
pub mod error;
pub mod protocol;
pub mod session;
pub mod state;

use std::sync::Arc;

pub use error::{AppError, AppErrorCode};

use tauri::Manager;
use tracing_subscriber::EnvFilter;

/// 启动 Qurner 应用。
pub fn run() {
    if let Err(error) = init_tracing() {
        eprintln!("Qurner 日志初始化失败：{error}");
        std::process::exit(1);
    }

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "Qurner 启动");

    if let Err(error) = build_app().run(tauri::generate_context!()) {
        tracing::error!(%error, "Qurner 运行失败");
        std::process::exit(1);
    }
}

fn build_app() -> tauri::Builder<tauri::Wry> {
    let sessions = Arc::new(session::SessionRegistry::new());
    let cover_directory = protocol::cover::directory_handle();
    let builder = protocol::cover::register(
        protocol::register(
            tauri::Builder::default()
                .plugin(tauri_plugin_dialog::init())
                .invoke_handler(tauri::generate_handler![
                    commands::start_import,
                    commands::list_books,
                    commands::soft_remove_book,
                    commands::select_import_files,
                    commands::open_book,
                    commands::close_book,
                    commands::get_effective_settings,
                    commands::update_app_settings,
                    commands::update_book_settings,
                    commands::reset_book_settings,
                    commands::save_reading_state,
                    commands::list_bookmarks,
                    commands::create_bookmark,
                    commands::delete_bookmark,
                ]),
            Arc::clone(&sessions),
        ),
        Arc::clone(&cover_directory),
    );

    builder.setup(move |app| {
        let state = match app.path().app_data_dir() {
            Ok(directory) => {
                let (state, covers) =
                    state::initialize_state_with_covers(&directory, Arc::clone(&sessions));
                if let Some(directory) = covers {
                    protocol::cover::set_directory(&cover_directory, directory);
                }
                state
            }
            Err(_) => {
                tracing::error!(
                    stage = "app_data_dir",
                    code = AppErrorCode::Storage.as_str(),
                    "应用数据目录解析失败"
                );
                state::AppState::failed_with_sessions(AppError::storage(), Arc::clone(&sessions))
            }
        };
        app.manage(state);
        tracing::info!(
            epub_core = epub_core::CRATE_NAME,
            storage_core = storage_core::CRATE_NAME,
            reader_model = reader_model::CRATE_NAME,
            library_core = library_core::CRATE_NAME,
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
