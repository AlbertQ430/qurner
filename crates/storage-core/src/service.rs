//! 专用写线程与线程安全存储入口。

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use reader_model::{BookSummary, ListBooksQuery};
use rusqlite::Connection;

use crate::StorageError;
use crate::books;
use crate::records::{ExistingBookRecord, NewBookRecord, ReactivateOutcome};
use crate::schema;

enum Request {
    SchemaVersion(Sender<Result<u32, StorageError>>),
    FindByFingerprint {
        fingerprint: String,
        reply: Sender<Result<Option<ExistingBookRecord>, StorageError>>,
    },
    InsertNewBook {
        record: NewBookRecord,
        reply: Sender<Result<(), StorageError>>,
    },
    ReactivateBook {
        id: String,
        source_path: String,
        imported_at: String,
        reply: Sender<Result<ReactivateOutcome, StorageError>>,
    },
    SoftRemoveBook {
        id: String,
        reply: Sender<Result<(), StorageError>>,
    },
    ListBooks {
        query: ListBooksQuery,
        reply: Sender<Result<Vec<BookSummary>, StorageError>>,
    },
    Shutdown(Sender<Result<(), StorageError>>),
    #[cfg(test)]
    TestWork {
        active: Arc<std::sync::atomic::AtomicUsize>,
        max: Arc<std::sync::atomic::AtomicUsize>,
        done: Sender<()>,
    },
}

struct Inner {
    sender: Mutex<Option<Sender<Request>>>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

/// 持有唯一写连接的存储服务；可克隆并在线程间安全传递。
#[derive(Clone)]
pub struct StorageService {
    inner: Arc<Inner>,
}

impl StorageService {
    /// 打开数据库、执行 PRAGMA 与迁移，并启动专用写线程。
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let (sender, receiver) = mpsc::channel::<Request>();
        let (init_sender, init_receiver) = mpsc::sync_channel::<Result<(), StorageError>>(0);
        let path: PathBuf = path.to_path_buf();

        let handle = thread::Builder::new()
            .name("qurner-storage".to_string())
            .spawn(move || match open_connection(&path) {
                Ok(mut connection) => {
                    if init_sender.send(Ok(())).is_ok() {
                        run_worker(&mut connection, receiver);
                    }
                }
                Err(error) => {
                    let _ = init_sender.send(Err(error));
                }
            })
            .map_err(|_| StorageError::OpenFailed)?;

        match init_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                inner: Arc::new(Inner {
                    sender: Mutex::new(Some(sender)),
                    handle: Mutex::new(Some(handle)),
                }),
            }),
            Ok(Err(error)) => {
                let _ = handle.join();
                Err(error)
            }
            Err(_) => {
                let _ = handle.join();
                Err(StorageError::OpenFailed)
            }
        }
    }

    /// 读取当前 schema 版本。
    pub fn schema_version(&self) -> Result<u32, StorageError> {
        let sender = self.sender()?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        sender
            .send(Request::SchemaVersion(reply_sender))
            .map_err(|_| StorageError::ThreadUnavailable)?;
        reply_receiver
            .recv()
            .map_err(|_| StorageError::ThreadUnavailable)?
    }

    /// 按指纹查询已存在书籍。
    pub fn find_by_fingerprint(
        &self,
        fingerprint: &str,
    ) -> Result<Option<ExistingBookRecord>, StorageError> {
        let sender = self.sender()?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        sender
            .send(Request::FindByFingerprint {
                fingerprint: fingerprint.to_string(),
                reply: reply_sender,
            })
            .map_err(|_| StorageError::ThreadUnavailable)?;
        reply_receiver
            .recv()
            .map_err(|_| StorageError::ThreadUnavailable)?
    }

    /// 在事务中插入新书；重复 fingerprint 返回 `Conflict`。
    pub fn insert_new_book(&self, record: NewBookRecord) -> Result<(), StorageError> {
        let sender = self.sender()?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        sender
            .send(Request::InsertNewBook {
                record,
                reply: reply_sender,
            })
            .map_err(|_| StorageError::ThreadUnavailable)?;
        reply_receiver
            .recv()
            .map_err(|_| StorageError::ThreadUnavailable)?
    }

    /// 恢复已软移除书籍；只更新 lifecycle、source_path 与 imported_at。
    pub fn reactivate_book(
        &self,
        id: &str,
        source_path: &str,
        imported_at: &str,
    ) -> Result<ReactivateOutcome, StorageError> {
        let sender = self.sender()?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        sender
            .send(Request::ReactivateBook {
                id: id.to_string(),
                source_path: source_path.to_string(),
                imported_at: imported_at.to_string(),
                reply: reply_sender,
            })
            .map_err(|_| StorageError::ThreadUnavailable)?;
        reply_receiver
            .recv()
            .map_err(|_| StorageError::ThreadUnavailable)?
    }

    /// 软移除书籍，不删除关联个人数据。
    pub fn soft_remove_book(&self, id: &str) -> Result<(), StorageError> {
        let sender = self.sender()?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        sender
            .send(Request::SoftRemoveBook {
                id: id.to_string(),
                reply: reply_sender,
            })
            .map_err(|_| StorageError::ThreadUnavailable)?;
        reply_receiver
            .recv()
            .map_err(|_| StorageError::ThreadUnavailable)?
    }

    /// 查询 active 书库列表；搜索与排序均在 SQL 中参数化完成。
    pub fn list_books(&self, query: ListBooksQuery) -> Result<Vec<BookSummary>, StorageError> {
        let sender = self.sender()?;
        let (reply_sender, reply_receiver) = mpsc::channel();
        sender
            .send(Request::ListBooks {
                query,
                reply: reply_sender,
            })
            .map_err(|_| StorageError::ThreadUnavailable)?;
        reply_receiver
            .recv()
            .map_err(|_| StorageError::ThreadUnavailable)?
    }

    /// 等待已排队请求完成、关闭连接并结束写线程。
    pub fn shutdown(self) -> Result<(), StorageError> {
        let sender = self
            .inner
            .sender
            .lock()
            .map_err(|_| StorageError::ThreadUnavailable)?
            .take();
        let Some(sender) = sender else {
            return Ok(());
        };

        let (reply_sender, reply_receiver) = mpsc::channel();
        sender
            .send(Request::Shutdown(reply_sender))
            .map_err(|_| StorageError::ThreadUnavailable)?;
        let result = reply_receiver
            .recv()
            .map_err(|_| StorageError::ThreadUnavailable)?;

        if let Some(handle) = self
            .inner
            .handle
            .lock()
            .map_err(|_| StorageError::ThreadUnavailable)?
            .take()
        {
            let _ = handle.join();
        }
        result
    }

    fn sender(&self) -> Result<Sender<Request>, StorageError> {
        self.inner
            .sender
            .lock()
            .map_err(|_| StorageError::ThreadUnavailable)?
            .clone()
            .ok_or(StorageError::ThreadUnavailable)
    }
}

fn open_connection(path: &Path) -> Result<Connection, StorageError> {
    let mut connection = Connection::open(path).map_err(|_| StorageError::OpenFailed)?;
    schema::configure(&connection)?;
    schema::migrate(&mut connection)?;
    Ok(connection)
}

fn run_worker(connection: &mut Connection, receiver: mpsc::Receiver<Request>) {
    while let Ok(request) = receiver.recv() {
        match request {
            Request::SchemaVersion(reply) => {
                let _ = reply.send(schema::current_version(connection));
            }
            Request::FindByFingerprint { fingerprint, reply } => {
                let _ = reply.send(books::find_by_fingerprint(connection, &fingerprint));
            }
            Request::InsertNewBook { record, reply } => {
                let _ = reply.send(books::insert_new_book(connection, record));
            }
            Request::ReactivateBook {
                id,
                source_path,
                imported_at,
                reply,
            } => {
                let _ = reply.send(books::reactivate_book(
                    connection,
                    &id,
                    &source_path,
                    &imported_at,
                ));
            }
            Request::SoftRemoveBook { id, reply } => {
                let _ = reply.send(books::soft_remove_book(connection, &id));
            }
            Request::ListBooks { query, reply } => {
                let _ = reply.send(books::list_books(connection, &query));
            }
            Request::Shutdown(reply) => {
                let _ = reply.send(Ok(()));
                break;
            }
            #[cfg(test)]
            Request::TestWork { active, max, done } => {
                let current = active.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                max.fetch_max(current, std::sync::atomic::Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(25));
                active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                let _ = done.send(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    use super::*;

    fn temp_db(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "qurner-storage-service-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("应能创建测试目录");
        directory.join("reader.sqlite3")
    }

    fn cleanup(path: &Path) {
        if let Some(directory) = path.parent() {
            let _ = std::fs::remove_dir_all(directory);
        }
    }

    #[test]
    fn opens_reports_version_and_shuts_down() {
        let path = temp_db("open");
        let service = StorageService::open(&path).expect("应能打开");
        assert_eq!(service.schema_version().expect("version"), 1);
        service.shutdown().expect("shutdown");
        cleanup(&path);
    }

    #[test]
    fn requests_after_shutdown_return_thread_unavailable() {
        let path = temp_db("shutdown");
        let service = StorageService::open(&path).expect("应能打开");
        let remaining = service.clone();
        service.shutdown().expect("shutdown");

        assert_eq!(
            remaining.schema_version().expect_err("应不可用"),
            StorageError::ThreadUnavailable
        );
        let _ = remaining.shutdown();
        cleanup(&path);
    }

    #[test]
    fn serializes_concurrent_requests_on_worker_thread() {
        let path = temp_db("serial");
        let service = StorageService::open(&path).expect("应能打开");

        let active = Arc::new(AtomicUsize::new(0));
        let max = Arc::new(AtomicUsize::new(0));
        let sender = service.sender().expect("sender");

        let mut completions = Vec::new();
        for _ in 0..2 {
            let (done_sender, done_receiver) = mpsc::channel();
            sender
                .send(Request::TestWork {
                    active: Arc::clone(&active),
                    max: Arc::clone(&max),
                    done: done_sender,
                })
                .expect("send");
            completions.push(done_receiver);
        }

        for completion in completions {
            completion.recv().expect("work done");
        }
        assert_eq!(max.load(Ordering::SeqCst), 1, "写线程应串行处理请求");

        service.shutdown().expect("shutdown");
        cleanup(&path);
    }

    #[test]
    fn shutdown_waits_for_in_flight_work() {
        let path = temp_db("wait");
        let service = StorageService::open(&path).expect("应能打开");
        let shutdown_service = service.clone();

        let (done_sender, done_receiver) = mpsc::channel();
        let active = Arc::new(AtomicUsize::new(0));
        let max = Arc::new(AtomicUsize::new(0));
        service
            .sender()
            .expect("sender")
            .send(Request::TestWork {
                active,
                max,
                done: done_sender,
            })
            .expect("send");

        let shutdown = std::thread::spawn(move || shutdown_service.shutdown());
        shutdown.join().expect("join").expect("shutdown");

        assert!(
            done_receiver.try_recv().is_ok(),
            "shutdown 应等待已入队工作完成"
        );
        cleanup(&path);
    }

    #[test]
    fn service_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StorageService>();
    }
}
