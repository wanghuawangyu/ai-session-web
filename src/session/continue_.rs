use std::path::Path;
use crate::error::{AppError, Result};
use super::{SessionMeta, SessionSource};

/// 解析 Continue session JSON 文件
pub fn parse_continue(path: &Path) -> Result<SessionMeta> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Io(e))?;

    let data: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Parse(format!("Continue parse error: {}", e)))?;

    let history = data.get("history").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);

    let mut user_count = 0usize;
    let mut ai_count = 0usize;

    if let Some(arr) = data.get("history").and_then(|v| v.as_array()) {
        for item in arr {
            let role = item
                .get("message")
                .and_then(|m| m.get("role"))
                .and_then(|r| r.as_str())
                .unwrap_or("");
            match role {
                "user" => user_count += 1,
                "assistant" => ai_count += 1,
                _ => {}
            }
        }
    }

    let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let wd = data.get("workspaceDirectory").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // session_id 从文件名获取
    let session_id = path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    // Continue: 单个文件，无额外关联文件
    let associated = vec![path.to_path_buf()];

    Ok(SessionMeta {
        source: SessionSource::Continue,
        session_id,
        title,
        name: String::new(),
        total_messages: history,
        user_messages: user_count,
        ai_messages: ai_count,
        created_at: String::new(),
        updated_at: String::new(), // Continue 无创建时间字段
        working_dir: wd,
        provider: "continue".to_string(),
        file_path: path.to_path_buf(),
        associated_files: associated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_continue() {
        let dir = std::env::temp_dir().join("ai-session-test-continue");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_session.json");

        let json = r#"{
            "title": "Continue Session",
            "workspaceDirectory": "/home/user/project",
            "history": [
                {"message": {"role": "user", "content": "hello"}},
                {"message": {"role": "assistant", "content": "hi"}},
                {"message": {"role": "user", "content": "again"}}
            ]
        }"#;

        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        drop(f);

        let meta = parse_continue(&path).unwrap();
        assert_eq!(meta.source, SessionSource::Continue);
        assert_eq!(meta.total_messages, 3);
        assert_eq!(meta.user_messages, 2);
        assert_eq!(meta.ai_messages, 1);
        assert_eq!(meta.title, "Continue Session");
        assert_eq!(meta.working_dir, "/home/user/project");
        assert_eq!(meta.provider, "continue");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
