//! 存储层公开记录类型。

/// 新建书籍记录；由 `library-core` 在导入时构造。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewBookRecord {
    pub id: String,
    pub fingerprint: String,
    pub source_path: String,
    pub title: String,
    pub author: String,
    pub title_search: String,
    pub author_search: String,
    pub title_sort_key: Vec<u8>,
    pub author_sort_key: Vec<u8>,
    /// 封面缓存键（内容指纹）；无缓存时为 `None`。
    pub cover_cache_key: Option<String>,
    pub manifest_json: String,
    pub manifest_version: u32,
    pub imported_at: String,
}

/// 已存在书籍的摘要，用于判重与软移除恢复。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingBookRecord {
    pub id: String,
    pub source_path: String,
    pub lifecycle: String,
}

/// `reactivate_book` 的类型化结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactivateOutcome {
    /// 记录已从 removed 恢复为 active。
    Reactivated,
    /// 记录当前已经是 active（可能由并发导入先恢复）。
    AlreadyActive,
    /// 记录不存在。
    NotFound,
}
