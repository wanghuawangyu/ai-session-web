use std::path::Path;
use crate::error::{AppError, Result};
use super::{SessionMeta, SessionSource};

/// 解析 Codex session JSONL 文件
pub fn parse_codex(path: &Path) -> Result<SessionMeta> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Io(e))?;

    let mut session_id = String::new();
    let mut created_at = String::new();
    let mut working_dir = String::new();
    let mut provider = String::new();
    let mut user_count = 0usize;
    let mut ai_count = 0usize;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let obj: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue, // 容忍无效行
        };

        let t = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let pl = obj.get("payload");

        match t {
            "session_meta" => {
                if let Some(p) = pl {
                    session_id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    created_at = p.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    working_dir = p.get("cwd").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    provider = p.get("model_provider").and_then(|v| v.as_str()).unwrap_or("codex").to_string();
                }
            }
            "response_item" => {
                if let Some(p) = pl {
                    if p.get("type").and_then(|v| v.as_str()) == Some("message") {
                        let role = p.get("role").and_then(|v| v.as_str()).unwrap_or("");
                        match role {
                            "user" => user_count += 1,
                            "assistant" => ai_count += 1,
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if session_id.is_empty() {
        // 回退：用文件名作为 session_id
        session_id = path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
    }

    let total = user_count + ai_count;

    // Codex: 单个文件，无额外关联文件
    let associated = vec![path.to_path_buf()];

    // content_updated 取 created_at（Codex 无独立 updated_at 字段）
    let content_updated = created_at.clone();
    let effective_updated_at = super::compute_effective_updated_at(&content_updated, &[path]);

    Ok(SessionMeta {
        source: SessionSource::Codex,
        session_id,
        title: String::new(),
        name: String::new(),
        total_messages: total,
        user_messages: user_count,
        ai_messages: ai_count,
        created_at: created_at.clone(),
        updated_at: created_at,
        working_dir,
        provider: if provider.is_empty() { "codex".to_string() } else { provider },
        file_path: path.to_path_buf(),
        associated_files: associated,
        has_custom_title: false,
        effective_updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_codex() {
        let dir = std::env::temp_dir().join("ai-session-test-codex");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_session.jsonl");

        let content = r#"{"type":"session_meta","payload":{"id":"sess_abc123","timestamp":"2025-01-01T00:00:00Z","cwd":"/home/user/project","model_provider":"openai"}}
{"type":"response_item","payload":{"type":"message","role":"user","content":"hello"}}
{"type":"response_item","payload":{"type":"message","role":"assistant","content":"hi"}}
{"type":"response_item","payload":{"type":"message","role":"user","content":"again"}}
"#;

        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        drop(f);

        let meta = parse_codex(&path).unwrap();
        assert_eq!(meta.source, SessionSource::Codex);
        assert_eq!(meta.session_id, "sess_abc123");
        assert_eq!(meta.total_messages, 3);
        assert_eq!(meta.user_messages, 2);
        assert_eq!(meta.ai_messages, 1);
        assert_eq!(meta.working_dir, "/home/user/project");
        assert_eq!(meta.provider, "openai");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_codex_tolerates_invalid_lines() {
        let dir = std::env::temp_dir().join("ai-session-test-codex2");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_bad.jsonl");

        let content = r#"{"type":"session_meta","payload":{"id":"sess_123"}}
this is not json
{"type":"response_item","payload":{"type":"message","role":"user","content":"hi"}}
"#;

        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        drop(f);

        let meta = parse_codex(&path).unwrap();
        assert_eq!(meta.session_id, "sess_123");
        assert_eq!(meta.total_messages, 1);
        assert_eq!(meta.user_messages, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
