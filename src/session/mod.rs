use serde::Serialize;
use std::path::PathBuf;

pub mod jcode;
pub mod codex;
pub mod continue_;
pub mod registry;

/// 会话来源
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SessionSource {
    Jcode,
    Codex,
    Continue,
}

impl std::fmt::Display for SessionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionSource::Jcode => write!(f, "jcode"),
            SessionSource::Codex => write!(f, "codex"),
            SessionSource::Continue => write!(f, "continue"),
        }
    }
}

impl SessionSource {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "jcode" => Some(SessionSource::Jcode),
            "codex" => Some(SessionSource::Codex),
            "continue" => Some(SessionSource::Continue),
            _ => None,
        }
    }
}

/// 统一的会话条目
#[derive(Debug, Clone, Serialize)]
pub struct SessionMeta {
    pub source: SessionSource,
    pub session_id: String,
    pub title: String,
    pub name: String,
    pub total_messages: usize,
    pub user_messages: usize,
    pub ai_messages: usize,
    pub created_at: String,
    pub updated_at: String,
    pub working_dir: String,
    pub provider: String,
    /// 对应磁盘上的完整文件路径
    pub file_path: PathBuf,
    /// 该 session 关联的所有文件（删除时一并清除）
    pub associated_files: Vec<PathBuf>,
}
