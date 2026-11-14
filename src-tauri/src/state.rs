//! Tauri 应用状态、启动初始化与导入 job 并发限制。

use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};

use library_core::LibraryService;
use storage_core::StorageService;

use crate::error::AppError;

/// 限制同时执行的导入 job 数；超出的 job 在阻塞任务中排队。
pub struct JobScheduler {
    limit: usize,
    active: Mutex<usize>,
    condvar: Condvar,
}

impl JobScheduler {
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            active: Mutex::new(0),
            condvar: Condvar::new(),
        }
    }

    /// 当前正在执行的 job 数（测试观测用）。
    #[must_use]
    pub fn active_count(&self) -> usize {
        *self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 取得一个执行槽位；返回的 guard 在 drop 时释放。
    pub fn acquire(self: &Arc<Self>) -> JobSchedulerGuard {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *active >= self.limit {
            active = self
                .condvar
                .wait(active)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        *active += 1;
        JobSchedulerGuard {
            scheduler: Arc::clone(self),
        }
    }
}

/// 导入 job 的执行槽位。
pub struct JobSchedulerGuard {
    scheduler: Arc<JobScheduler>,
}

impl Drop for JobSchedulerGuard {
    fn drop(&mut self) {
        let mut active = self
            .scheduler
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *active = active.saturating_sub(1);
        self.scheduler.condvar.notify_one();
    }
}

/// 全局应用状态；初始化失败时保存稳定错误，三个业务命令统一返回该错误。
pub struct AppState {
    library: Result<Arc<LibraryService>, AppError>,
    scheduler: Arc<JobScheduler>,
}

impl AppState {
    #[must_use]
    pub fn ready(library: LibraryService) -> Self {
        Self {
            library: Ok(Arc::new(library)),
            scheduler: Arc::new(JobScheduler::new(2)),
        }
    }

    #[must_use]
    pub fn failed(error: AppError) -> Self {
        Self {
            library: Err(error),
            scheduler: Arc::new(JobScheduler::new(2)),
        }
    }

    /// 获取共享书库服务；初始化失败时返回同一稳定错误。
    pub fn library(&self) -> Result<Arc<LibraryService>, AppError> {
        self.library.clone()
    }

    #[must_use]
    pub fn scheduler(&self) -> Arc<JobScheduler> {
        Arc::clone(&self.scheduler)
    }
}

/// 在给定应用数据目录创建数据库并初始化服务。
///
/// 日志只记录阶段与错误类别，不记录绝对路径。
#[must_use]
pub fn initialize_state(app_data_dir: &Path) -> AppState {
    if let Err(error) = std::fs::create_dir_all(app_data_dir) {
        tracing::error!(
            stage = "create_app_data_dir",
            kind = ?error.kind(),
            code = AppError::storage().code.as_str(),
            "应用数据目录创建失败"
        );
        return AppState::failed(AppError::storage());
    }

    let database_path = app_data_dir.join("reader.sqlite3");
    match StorageService::open(&database_path) {
        Ok(storage) => AppState::ready(LibraryService::new(storage)),
        Err(_) => {
            tracing::error!(
                stage = "open_storage",
                code = AppError::storage().code.as_str(),
                "存储初始化失败"
            );
            AppState::failed(AppError::storage())
        }
    }
}
