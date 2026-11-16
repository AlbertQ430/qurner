//! 设置继承、全局/单书更新与重置用例。
//!
//! 合并顺序：内置默认 < `app_settings[reader_defaults]` < `book_settings` 非 NULL。

use reader_model::{ReaderMode, ReaderSettings, ReaderTheme};
use storage_core::{BookSettingsRecord, StorageError, StorageService};

use crate::error::{BookError, StorageErrorKind};
use crate::open::GLOBAL_SETTINGS_KEY;

/// 全局设置 patch；不接受 `null` 清除。
#[derive(Debug, Clone, Default)]
pub struct AppSettingsPatch {
    pub mode: Option<ReaderMode>,
    pub font_scale: Option<f64>,
    pub theme: Option<ReaderTheme>,
}

impl AppSettingsPatch {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mode.is_none() && self.font_scale.is_none() && self.theme.is_none()
    }
}

/// 单书设置 patch；外层 `None` 表示字段未提供，内层 `None` 表示清除覆盖。
#[derive(Debug, Clone, Default)]
pub struct BookSettingsPatch {
    pub mode: Option<Option<ReaderMode>>,
    pub font_scale: Option<Option<f64>>,
    pub theme: Option<Option<ReaderTheme>>,
}

impl BookSettingsPatch {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mode.is_none() && self.font_scale.is_none() && self.theme.is_none()
    }
}

pub(crate) fn load_settings(
    storage: &StorageService,
    book_id: &str,
) -> Result<ReaderSettings, BookError> {
    let global = parse_global(
        storage
            .app_setting(GLOBAL_SETTINGS_KEY)
            .map_err(map_storage)?,
    )?;
    let book = storage.book_settings(book_id).map_err(map_storage)?;
    merge_book(global, book.as_ref())
}

/// 读取单书 effective settings；active book 不存在 → BookNotFound。
pub(crate) fn get_effective(
    storage: &StorageService,
    book_id: &str,
) -> Result<ReaderSettings, BookError> {
    let id = book_id.trim();
    if id.is_empty() {
        return Err(BookError::InvalidInput);
    }
    ensure_active(storage, id)?;
    load_settings(storage, id)
}

/// 更新全局默认并返回新的全局设置。
pub(crate) fn update_app(
    storage: &StorageService,
    patch: AppSettingsPatch,
) -> Result<ReaderSettings, BookError> {
    if patch.is_empty() {
        return Err(BookError::InvalidInput);
    }
    let font_scale = patch.font_scale.map(validate_font_scale).transpose()?;
    storage
        .update_app_settings(
            GLOBAL_SETTINGS_KEY,
            patch.mode.map(mode_str),
            font_scale,
            patch.theme.map(theme_str),
            &now(),
        )
        .map_err(map_storage)?;
    parse_global(
        storage
            .app_setting(GLOBAL_SETTINGS_KEY)
            .map_err(map_storage)?,
    )
}

/// 更新单书覆盖并返回合并后的 effective settings。
pub(crate) fn update_book(
    storage: &StorageService,
    book_id: &str,
    patch: BookSettingsPatch,
) -> Result<ReaderSettings, BookError> {
    let id = book_id.trim();
    if id.is_empty() {
        return Err(BookError::InvalidInput);
    }
    if patch.is_empty() {
        return Err(BookError::InvalidInput);
    }
    ensure_active(storage, id)?;
    let font_scale = match patch.font_scale {
        Some(Some(value)) => Some(Some(validate_font_scale(value)?)),
        Some(None) => Some(None),
        None => None,
    };
    storage
        .update_book_settings(
            id,
            patch.mode.map(|value| value.map(mode_str)),
            font_scale,
            patch.theme.map(|value| value.map(theme_str)),
            &now(),
        )
        .map_err(map_storage)?;
    load_settings(storage, id)
}

/// 清除单书全部覆盖并返回全局设置。
pub(crate) fn reset_book(
    storage: &StorageService,
    book_id: &str,
) -> Result<ReaderSettings, BookError> {
    let id = book_id.trim();
    if id.is_empty() {
        return Err(BookError::InvalidInput);
    }
    ensure_active(storage, id)?;
    let global = parse_global(
        storage
            .app_setting(GLOBAL_SETTINGS_KEY)
            .map_err(map_storage)?,
    )?;
    storage.delete_book_settings(id).map_err(map_storage)?;
    Ok(global)
}

pub(crate) fn ensure_active(storage: &StorageService, book_id: &str) -> Result<(), BookError> {
    if storage
        .open_book_record(book_id)
        .map_err(map_storage)?
        .is_none()
    {
        return Err(BookError::BookNotFound);
    }
    Ok(())
}

fn parse_global(json: Option<String>) -> Result<ReaderSettings, BookError> {
    let mut settings = ReaderSettings::default();
    let Some(json) = json else {
        return Ok(settings);
    };
    let value: serde_json::Value = serde_json::from_str(&json).map_err(|_| storage_error())?;
    let object = value.as_object().ok_or_else(storage_error)?;
    for key in object.keys() {
        if key != "mode" && key != "fontScale" && key != "theme" {
            return Err(storage_error());
        }
    }
    if let Some(value) = object.get("mode") {
        let text = value.as_str().ok_or_else(storage_error)?;
        settings.mode = parse_mode(text)?;
    }
    if let Some(value) = object.get("fontScale") {
        let number = value.as_f64().ok_or_else(storage_error)?;
        settings.font_scale = parse_stored_font_scale(number)?;
    }
    if let Some(value) = object.get("theme") {
        let text = value.as_str().ok_or_else(storage_error)?;
        settings.theme = parse_theme(text)?;
    }
    if !settings.is_valid() {
        return Err(storage_error());
    }
    Ok(settings)
}

fn merge_book(
    mut settings: ReaderSettings,
    record: Option<&BookSettingsRecord>,
) -> Result<ReaderSettings, BookError> {
    if let Some(record) = record {
        if let Some(mode) = &record.mode {
            settings.mode = parse_mode(mode)?;
        }
        if let Some(font_scale) = record.font_scale {
            settings.font_scale = parse_stored_font_scale(font_scale)?;
        }
        if let Some(theme) = &record.theme {
            settings.theme = parse_theme(theme)?;
        }
    }
    if !settings.is_valid() {
        return Err(storage_error());
    }
    Ok(settings)
}

fn parse_mode(value: &str) -> Result<ReaderMode, BookError> {
    match value {
        "paged" => Ok(ReaderMode::Paged),
        "scrolling" => Ok(ReaderMode::Scrolling),
        _ => Err(storage_error()),
    }
}

fn parse_theme(value: &str) -> Result<ReaderTheme, BookError> {
    match value {
        "day" => Ok(ReaderTheme::Day),
        "night" => Ok(ReaderTheme::Night),
        _ => Err(storage_error()),
    }
}

fn parse_stored_font_scale(value: f64) -> Result<f64, BookError> {
    if value.is_finite()
        && (ReaderSettings::MIN_FONT_SCALE..=ReaderSettings::MAX_FONT_SCALE).contains(&value)
    {
        Ok(value)
    } else {
        Err(storage_error())
    }
}

fn validate_font_scale(value: f64) -> Result<f64, BookError> {
    if value.is_finite()
        && (ReaderSettings::MIN_FONT_SCALE..=ReaderSettings::MAX_FONT_SCALE).contains(&value)
    {
        Ok(value)
    } else {
        Err(BookError::InvalidInput)
    }
}

fn mode_str(mode: ReaderMode) -> &'static str {
    match mode {
        ReaderMode::Paged => "paged",
        ReaderMode::Scrolling => "scrolling",
    }
}

fn theme_str(theme: ReaderTheme) -> &'static str {
    match theme {
        ReaderTheme::Day => "day",
        ReaderTheme::Night => "night",
    }
}

fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn map_storage(error: StorageError) -> BookError {
    BookError::Storage(StorageErrorKind::from(error))
}

fn storage_error() -> BookError {
    BookError::Storage(StorageErrorKind::QueryFailed)
}
