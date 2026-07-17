use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use crate::error::{AppError, Result};
use crate::api::{AppState, ApiResponse};
use crate::session::SessionSource;
use crate::session::registry::SortedSessionGroup;

// ---------- Handlers ----------

/// GET /api/sessions — 获取排序后的会话列表
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<SortedSessionGroup>>>> {
    let registry = state.registry.lock().await;
    let sorted = registry.sorted_list();

    Ok(Json(ApiResponse::success(sorted)))
}

/// DELETE /api/sessions/{source}/{session_id} — 删除会话
pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path((source_str, session_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<String>>> {
    let source = SessionSource::from_str(&source_str)
        .ok_or_else(|| AppError::Config(format!("Invalid source: {}", source_str)))?;

    let mut registry = state.registry.lock().await;
    let deleted = registry.delete(&source, &session_id)?;

    Ok(Json(ApiResponse::success(format!(
        "已删除 {} 个文件（会话: {}:{}）",
        deleted.len(),
        source,
        session_id
    ))))
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

    let mut json_value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Parse(format!("JSON parse error: {}", e)))?;

    // Jcode session: 如果存在 .journal.jsonl，合并其中的消息到 messages[]
    if source == SessionSource::Jcode {
        let parent = meta.file_path.parent().unwrap_or(std::path::Path::new("."));
        let journal_path = parent.join(format!("{}.journal.jsonl", session_id));
        if journal_path.exists() {
            if let Ok(journal_content) = std::fs::read_to_string(&journal_path) {
                let mut journal_messages: Vec<serde_json::Value> = Vec::new();
                for line in journal_content.lines() {
                    let line = line.trim();
                    if line.is_empty() { continue; }
                    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                        if obj.get("type").and_then(|v| v.as_str()) == Some("response_item") {
                            if let Some(pl) = obj.get("payload") {
                                if pl.get("type").and_then(|v| v.as_str()) == Some("message") {
                                    let msg = serde_json::json!({
                                        "role": pl.get("role"),
                                        "content": pl.get("content"),
                                        "timestamp": pl.get("timestamp"),
                                        "source": "journal"
                                    });
                                    journal_messages.push(msg);
                                }
                            }
                        }
                    }
                }
                if !journal_messages.is_empty() {
                    if let Some(arr) = json_value.get_mut("messages").and_then(|v| v.as_array_mut()) {
                        arr.extend(journal_messages);
                    } else {
                        json_value["messages"] = serde_json::Value::Array(journal_messages);
                    }
                }
            }
        }
    }

    Ok(Json(json_value))
}
