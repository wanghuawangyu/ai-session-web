use serde::Serialize;
use std::path::PathBuf;
use std::path::Path;

pub mod jcode;
pub mod jcode_journal;
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
    /// 是否有自定义标题（主会话 vs 临时会话）
    pub has_custom_title: bool,
    /// 排序用最终时间：max(内容updated_at, 所有数据文件mtime)，ISO 8601 格式
    pub effective_updated_at: String,
}

/// 获取文件的 mtime 并格式化为 ISO 8601 字符串
pub fn file_mtime_iso(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let dur = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();
    // 格式化为 ISO 8601（带毫秒）
    Some(iso_from_unix(secs, millis))
}

/// 从 Unix 时间戳生成 ISO 8601 字符串（UTC）
fn iso_from_unix(secs: u64, millis: u32) -> String {
    let utc = chrono::DateTime::from_timestamp(secs as i64, millis * 1_000_000)
        .unwrap_or_default();
    utc.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// 计算 effective_updated_at：max(内容updated_at, 文件mtime列表)
pub fn compute_effective_updated_at(content_updated: &str, file_paths: &[&Path]) -> String {
    let mut candidates: Vec<String> = Vec::new();
    if !content_updated.is_empty() {
        candidates.push(content_updated.to_string());
    }
    for fp in file_paths {
        if let Some(mtime) = file_mtime_iso(fp) {
            candidates.push(mtime);
        }
    }
    candidates.into_iter().max().unwrap_or_default()
}
