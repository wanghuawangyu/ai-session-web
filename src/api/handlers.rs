use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::api::{AppState, ApiResponse};
use crate::session::SessionSource;

#[derive(Debug, Serialize)]
pub struct SessionGroup {
    pub source: String,
    pub sessions: Vec<SessionEntry>,
}

#[derive(Debug, Serialize)]
pub struct SessionEntry {
    pub session_id: String,
    pub title: String,
    pub name: String,
    pub total_messages: usize,
    pub user_messages: usize,
    pub ai_messages: usize,
    pub created_at: String,
    pub working_dir: String,
    pub provider: String,
}

impl From<&crate::session::SessionMeta> for SessionEntry {
    fn from(meta: &crate::session::SessionMeta) -> Self {
        SessionEntry {
            session_id: meta.session_id.clone(),
            title: meta.title.clone(),
            name: meta.name.clone(),
            total_messages: meta.total_messages,
            user_messages: meta.user_messages,
            ai_messages: meta.ai_messages,
            created_at: meta.created_at.clone(),
            working_dir: meta.working_dir.clone(),
            provider: meta.provider.clone(),
        }
    }
}

// ---------- Handlers ----------

/// GET /api/sessions — 获取所有会话，按来源分组
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<SessionGroup>>>> {
    let registry = state.registry.lock().await;
    let grouped = registry.list_grouped();

    let mut groups: Vec<SessionGroup> = grouped
        .into_iter()
        .map(|(source, sessions)| {
            let entries: Vec<SessionEntry> = sessions
                .into_iter()
                .map(|s| SessionEntry::from(s))
                .collect();
            SessionGroup {
                source: source.to_string(),
                sessions: entries,
            }
        })
        .collect();

    // 固定排序: jcode, codex, continue
    groups.sort_by(|a, b| {
        let order = |s: &str| match s {
            "jcode" => 0,
            "codex" => 1,
            "continue" => 2,
            _ => 3,
        };
        order(&a.source).cmp(&order(&b.source))
    });

    Ok(Json(ApiResponse::success(groups)))
}

/// DELETE /api/sessions/{source}/{session_id} — 删除会话
pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path((source_str, session_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<String>>> {
    let source = SessionSource::from_str(&source_str)
        .ok_or_else(|| AppError::Config(format!("Invalid source: {}", source_str)))?;

    let mut registry = state.registry.lock().await;
    registry.delete(&source, &session_id)?;

    Ok(Json(ApiResponse::success(format!("Deleted {}:{}", source, session_id))))
}

/// GET /api/sessions/{source}/{session_id}/json — 获取会话原始 JSON 内容
pub async fn get_session_json(
    State(state): State<Arc<AppState>>,
    Path((source_str, session_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>> {
    let source = SessionSource::from_str(&source_str)
        .ok_or_else(|| AppError::Config(format!("Invalid source: {}", source_str)))?;

    let registry = state.registry.lock().await;
    let meta = registry.get(&source, &session_id)
        .ok_or_else(|| AppError::SessionNotFound(format!("{}:{}", source, session_id)))?;

    let content = std::fs::read_to_string(&meta.file_path)
        .map_err(|e| AppError::Io(e))?;

    let json_value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Parse(format!("JSON parse error: {}", e)))?;

    Ok(Json(json_value))
}
