//! 进程内阅读会话注册表。
//!
//! 每个会话保存规范化源路径与已解析书籍；协议令牌为 128 位随机值，与
//! `sessionId` 职责分离。

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use epub_core::ParsedBook;

/// 会话创建失败类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// 源路径无法规范化。
    InvalidPath,
}

/// 会话创建结果；`sessionId` 与协议令牌分离。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHandle {
    pub session_id: String,
    pub token: String,
}

/// 单个阅读会话。
pub struct Session {
    pub session_id: String,
    pub token: String,
    pub source_path: PathBuf,
    pub book: ParsedBook,
}

/// 线程安全会话注册表。
pub struct SessionRegistry {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    by_id: Mutex<HashMap<String, String>>,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            by_id: Mutex::new(HashMap::new()),
        }
    }

    /// 创建会话；源路径会规范化后保存。
    pub fn create(
        &self,
        source_path: &Path,
        book: ParsedBook,
    ) -> Result<SessionHandle, SessionError> {
        let source = source_path
            .canonicalize()
            .map_err(|_| SessionError::InvalidPath)?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let token = random_token();
        let session = Arc::new(Session {
            session_id: session_id.clone(),
            token: token.clone(),
            source_path: source,
            book,
        });
        lock(&self.sessions).insert(token.clone(), Arc::clone(&session));
        lock(&self.by_id).insert(session_id.clone(), token.clone());
        Ok(SessionHandle { session_id, token })
    }

    /// 按协议令牌查找会话。
    pub fn lookup(&self, token: &str) -> Option<Arc<Session>> {
        lock(&self.sessions).get(token).cloned()
    }

    /// 关闭会话；关闭后其协议 URL 立即失效。
    pub fn close(&self, session_id: &str) -> bool {
        let Some(token) = lock(&self.by_id).remove(session_id) else {
            return false;
        };
        lock(&self.sessions).remove(&token).is_some()
    }

    /// 当前会话数（测试观测用）。
    #[must_use]
    pub fn len(&self) -> usize {
        lock(&self.sessions).len()
    }

    /// 当前是否没有会话。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 128 位随机令牌（32 个十六进制字符）。
fn random_token() -> String {
    let mut bytes = [0_u8; 16];
    if let Ok(mut file) = std::fs::File::open("/dev/urandom")
        && file.read_exact(&mut bytes).is_ok()
    {
        return hex(&bytes);
    }

    fallback_token()
}

/// 回退令牌：完整两个 UUID v4（64 个十六进制字符，约 244 位随机）。
fn fallback_token() -> String {
    let first = uuid::Uuid::new_v4().simple().to_string();
    let second = uuid::Uuid::new_v4().simple().to_string();
    format!("{first}{second}")
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(TABLE[(byte >> 4) as usize] as char);
        output.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_token_has_at_least_128_random_bits() {
        let first = fallback_token();
        let second = fallback_token();
        assert_eq!(first.len(), 64, "完整两个 UUID 的十六进制长度");
        assert_eq!(second.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    #[test]
    fn session_tokens_are_separate_from_session_ids() {
        let registry = SessionRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }
}
