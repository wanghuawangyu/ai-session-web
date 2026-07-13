use std::path::Path;
use crate::error::{AppError, Result};
use super::{SessionMeta, SessionSource};

/// 解析 Jcode session JSON 文件
pub fn parse_jcode(path: &Path) -> Result<SessionMeta> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Io(e))?;

    let data: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Parse(format!("Jcode parse error: {}", e)))?;

    let msgs = data.get("messages").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    let user = data.get("messages").and_then(|v| v.as_array()).map(|a| {
        a.iter().filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("user")).count()
    }).unwrap_or(0);
    let ai = data.get("messages").and_then(|v| v.as_array()).map(|a| {
        a.iter().filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant")).count()
    }).unwrap_or(0);

    let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let name = data.get("custom_title")
        .or_else(|| data.get("short_name"))
        .or_else(|| data.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let created = data.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let wd = data.get("working_dir").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let provider = data.get("provider_key").and_then(|v| v.as_str()).unwrap_or("jcode").to_string();

    // 获取 session id (文件名不含扩展名)
    let session_id = path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    // Jcode 关联的文件: .json, .bak, .journal.jsonl
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut associated = Vec::new();
    associated.push(path.to_path_buf());
    let bak_path = parent.join(format!("{}.bak", session_id));
    if bak_path.exists() {
        associated.push(bak_path);
    }
    let journal_path = parent.join(format!("{}.journal.jsonl", session_id));
    if journal_path.exists() {
        associated.push(journal_path);
    }

    Ok(SessionMeta {
        source: SessionSource::Jcode,
        session_id,
        title,
        name,
        total_messages: msgs,
        user_messages: user,
        ai_messages: ai,
        created_at: created,
        working_dir: wd,
        provider,
        file_path: path.to_path_buf(),
        associated_files: associated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_jcode() {
        let dir = std::env::temp_dir().join("ai-session-test-jcode");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_session.json");

        let json = r#"{
            "title": "Test Session",
            "custom_title": "MySession",
            "messages": [
                {"role": "user", "content": "hello"},
                {"role": "assistant", "content": "hi"},
                {"role": "user", "content": "again"}
            ],
            "created_at": "2025-01-01T00:00:00Z",
            "working_dir": "/home/user/project",
            "provider_key": "anthropic"
        }"#;

        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        drop(f);

        let meta = parse_jcode(&path).unwrap();
        assert_eq!(meta.source, SessionSource::Jcode);
        assert_eq!(meta.session_id, "test_session");
        assert_eq!(meta.title, "Test Session");
        assert_eq!(meta.name, "MySession");
        assert_eq!(meta.total_messages, 3);
        assert_eq!(meta.user_messages, 2);
        assert_eq!(meta.ai_messages, 1);
        assert_eq!(meta.working_dir, "/home/user/project");
        assert_eq!(meta.provider, "anthropic");
        assert_eq!(meta.associated_files.len(), 1); // just the json

        let _ = std::fs::remove_dir_all(&dir);
    }
}
