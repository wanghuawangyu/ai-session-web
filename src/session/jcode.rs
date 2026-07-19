use std::path::Path;
use crate::error::{AppError, Result};
use super::{SessionMeta, SessionSource, jcode_journal};

/// 解析 Jcode session JSON 文件，并合并同名的 .journal.jsonl 数据
pub fn parse_jcode(path: &Path) -> Result<SessionMeta> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Io(e))?;

    let data: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Parse(format!("Jcode parse error: {}", e)))?;

    // --- 从 .json 中提取基本字段 ---
    let json_user = data.get("messages").and_then(|v| v.as_array()).map(|a| {
        a.iter().filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("user")).count()
    }).unwrap_or(0);
    let json_ai = data.get("messages").and_then(|v| v.as_array()).map(|a| {
        a.iter().filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant")).count()
    }).unwrap_or(0);

    let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let custom_title = data.get("custom_title").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let name = if !custom_title.is_empty() {
        custom_title.clone()
    } else {
        data.get("short_name")
            .or_else(|| data.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let has_custom_title = !custom_title.is_empty();
    let created = data.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let json_updated = data.get("updated_at").and_then(|v| v.as_str()).unwrap_or(&created).to_string();
    let json_wd = data.get("working_dir").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let provider = data.get("provider_key").and_then(|v| v.as_str()).unwrap_or("jcode").to_string();

    // 获取 session id (文件名不含扩展名)
    let session_id = path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    // --- 尝试解析同名的 .journal.jsonl ---
    let parent = path.parent().unwrap_or(Path::new("."));
    let journal_path = parent.join(format!("{}.journal.jsonl", session_id));

    let (user, ai, updated_at, wd_from_journal) = if journal_path.exists() {
        match jcode_journal::parse_jcode_journal(&journal_path) {
            Ok(jd) => {
                let new_user = json_user + jd.user_count;
                let new_ai = json_ai + jd.ai_count;
                // updated_at 取两者中的较大值
                let new_updated = if jd.latest_timestamp > json_updated {
                    jd.latest_timestamp.clone()
                } else {
                    json_updated.clone()
                };
                // working_dir: journal 中的 cwd 覆盖（如果存在）
                let new_wd = jd.cwd.clone().unwrap_or(json_wd.clone());
                (new_user, new_ai, new_updated, new_wd)
            }
            Err(_) => (json_user, json_ai, json_updated.clone(), json_wd.clone())
        }
    } else {
        (json_user, json_ai, json_updated.clone(), json_wd.clone())
    };

    let total = user + ai;

    // --- 关联文件: .json, .bak, .journal.jsonl ---
    let mut associated = Vec::new();
    associated.push(path.to_path_buf());
    let bak_path = parent.join(format!("{}.bak", session_id));
    if bak_path.exists() {
        associated.push(bak_path);
    }
    // journal 文件在目录中存在就关联（即使解析失败也要能删除）
    let journal_exists = journal_path.exists();
    if journal_exists {
        associated.push(journal_path.clone());
    }

    // --- 计算 effective_updated_at（内容时间 + 文件mtime 取大值）---
    let mut eff_candidate_files: Vec<&Path> = vec![path];
    if journal_exists {
        eff_candidate_files.push(&journal_path);
    }
    let effective_updated_at = super::compute_effective_updated_at(&updated_at, &eff_candidate_files);

    Ok(SessionMeta {
        source: SessionSource::Jcode,
        session_id,
        title,
        name,
        total_messages: total,
        user_messages: user,
        ai_messages: ai,
        created_at: created,
        updated_at,
        working_dir: wd_from_journal,
        provider,
        file_path: path.to_path_buf(),
        associated_files: associated,
        has_custom_title,
        effective_updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_json(dir: &std::path::Path, name: &str, user_msgs: usize, ai_msgs: usize) -> std::path::PathBuf {
        let path = dir.join(format!("{}.json", name));
        let mut msgs = Vec::new();
        for i in 0..user_msgs {
            msgs.push(format!(r#"{{"role":"user","content":"u{}"}}"#, i));
        }
        for i in 0..ai_msgs {
            msgs.push(format!(r#"{{"role":"assistant","content":"a{}"}}"#, i));
        }
        let json = format!(
            r#"{{
                "title": "Test",
                "custom_title": "TestName",
                "messages": [{}],
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T01:00:00Z",
                "working_dir": "/test/project",
                "provider_key": "test"
            }}"#,
            msgs.join(",")
        );
        std::fs::write(&path, json).unwrap();
        path
    }

    fn create_journal(dir: &std::path::Path, name: &str, user_msgs: usize, ai_msgs: usize) -> std::path::PathBuf {
        let path = dir.join(format!("{}.journal.jsonl", name));
        let mut lines = vec![
            r#"{"type":"session_meta","payload":{"id":"s","timestamp":"2025-02-01T00:00:00Z","cwd":"/new/project","model_provider":"test"}}"#.to_string()
        ];
        for i in 0..user_msgs {
            lines.push(format!(r#"{{"type":"response_item","payload":{{"type":"message","role":"user","content":"ju{}","timestamp":"2025-02-01T00:01:00Z"}}}}"#, i));
        }
        for i in 0..ai_msgs {
            lines.push(format!(r#"{{"type":"response_item","payload":{{"type":"message","role":"assistant","content":"ja{}","timestamp":"2025-02-01T00:02:00Z"}}}}"#, i));
        }
        std::fs::write(&path, lines.join("\n")).unwrap();
        path
    }

    #[test]
    fn test_parse_jcode_no_journal() {
        let dir = std::env::temp_dir().join("ai-session-test-jcode-only");
        let _ = std::fs::create_dir_all(&dir);
        create_json(&dir, "test_sess", 2, 1);

        let path = dir.join("test_sess.json");
        let meta = parse_jcode(&path).unwrap();
        assert_eq!(meta.user_messages, 2);
        assert_eq!(meta.ai_messages, 1);
        assert_eq!(meta.total_messages, 3);
        assert_eq!(meta.working_dir, "/test/project");
        assert_eq!(meta.updated_at, "2025-01-01T01:00:00Z");
        assert_eq!(meta.associated_files.len(), 1); // just .json

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_jcode_with_journal() {
        let dir = std::env::temp_dir().join("ai-session-test-dual");
        let _ = std::fs::create_dir_all(&dir);
        create_json(&dir, "sess_dual", 2, 1);
        create_journal(&dir, "sess_dual", 3, 2);

        let path = dir.join("sess_dual.json");
        let meta = parse_jcode(&path).unwrap();
        // json: 2 user + 1 ai = 3; journal: 3 user + 2 ai = 5; total = 8
        assert_eq!(meta.user_messages, 5);  // 2 + 3
        assert_eq!(meta.ai_messages, 3);    // 1 + 2
        assert_eq!(meta.total_messages, 8); // 3 + 5
        // journal cwd covers
        assert_eq!(meta.working_dir, "/new/project");
        // journal timestamp > json updated_at
        assert_eq!(meta.updated_at, "2025-02-01T00:02:00Z");
        // associated: .json + .journal.jsonl
        assert!(meta.associated_files.len() >= 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_jcode_with_bak() {
        let dir = std::env::temp_dir().join("ai-session-test-bak");
        let _ = std::fs::create_dir_all(&dir);
        create_json(&dir, "sess_bak", 1, 0);
        // Create a .bak file
        let bak_path = dir.join("sess_bak.bak");
        std::fs::write(&bak_path, "bak content").unwrap();

        let path = dir.join("sess_bak.json");
        let meta = parse_jcode(&path).unwrap();
        assert_eq!(meta.associated_files.len(), 2); // .json + .bak
        assert!(meta.associated_files.iter().any(|p| p.extension().map(|e| e == "bak").unwrap_or(false)));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
