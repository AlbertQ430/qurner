//! 阅读会话、设置与 TOC 模型。

use serde::{Deserialize, Serialize};

use crate::locator::{ReaderLocator, ReaderMode};

/// 阅读主题。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReaderTheme {
    Day,
    Night,
}

/// 生效阅读设置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderSettings {
    pub mode: ReaderMode,
    pub font_scale: f64,
    pub theme: ReaderTheme,
}

impl Default for ReaderSettings {
    fn default() -> Self {
        Self {
            mode: ReaderMode::Paged,
            font_scale: 1.0,
            theme: ReaderTheme::Day,
        }
    }
}

impl ReaderSettings {
    /// fontScale 的合法范围。
    pub const MIN_FONT_SCALE: f64 = 0.5;
    pub const MAX_FONT_SCALE: f64 = 3.0;

    /// 校验设置值；非法持久化值不得进入会话。
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.font_scale.is_finite()
            && (Self::MIN_FONT_SCALE..=Self::MAX_FONT_SCALE).contains(&self.font_scale)
    }
}

/// 会话的只读 spine 项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderSpineItem {
    pub index: usize,
    pub href: String,
}

/// 会话目录项。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderTocNode {
    pub id: String,
    pub label: String,
    pub spine_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment: Option<String>,
    pub children: Vec<ReaderTocNode>,
}

/// 打开书籍后返回的阅读会话。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderSession {
    pub session_id: String,
    pub book_id: String,
    pub title: String,
    pub frame_url: String,
    pub spine: Vec<ReaderSpineItem>,
    pub chapter_weights: Vec<u32>,
    pub toc: Vec<ReaderTocNode>,
    pub initial_locator: Option<ReaderLocator>,
    pub initial_progression: f64,
    pub settings: ReaderSettings,
}
