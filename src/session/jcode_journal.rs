use std::path::Path;
use crate::error::{AppError, Result};

/// Journal 解析结果
pub struct JournalData {
    pub user_count: usize,
    pub ai_count: usize,
    pub latest_timestamp: String,
    pub cwd: Option<String>,
    /// journal 中所有 message 行的原始 JSON 字符串（用于合并查看）
    #[allow(dead_code)]
    pub message_lines: Vec<String>,
}

/// 解析 Jcode .journal.jsonl 文件
pub fn parse_jcode_journal(path: &Path) -> Result<JournalData> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Io(e))?;

    let mut user_count = 0usize;
    let mut ai_count = 0usize;
    let mut latest_timestamp = String::new();
    let mut cwd: Option<String> = None;
    let mut message_lines: Vec<String> = Vec::new();

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

        match t {
            "session_meta" => {
                if let Some(pl) = obj.get("payload") {
                    // 取最新的 cwd
                    if let Some(cwd_val) = pl.get("cwd").and_then(|v| v.as_str()) {
                        if !cwd_val.is_empty() {
                            cwd = Some(cwd_val.to_string());
                        }
                    }
                    // 取最新时间戳
                    if let Some(ts) = pl.get("timestamp").and_then(|v| v.as_str()) {
                        if ts > latest_timestamp.as_str() {
                            latest_timestamp = ts.to_string();
                        }
                    }
                }
            }
            "response_item" => {
                // 包含 env_snapshot 和其它事件，只关心 type=message
                if let Some(pl) = obj.get("payload") {
                    if pl.get("type").and_then(|v| v.as_str()) == Some("message") {
                        let role = pl.get("role").and_then(|v| v.as_str()).unwrap_or("");
                        match role {
                            "user" => user_count += 1,
                            "assistant" => ai_count += 1,
                            _ => {}
                        }
                        message_lines.push(line.to_string());
                    }
                    // 也有时间戳
                    if let Some(ts) = pl.get("timestamp").and_then(|v| v.as_str()) {
                        if ts > latest_timestamp.as_str() {
                            latest_timestamp = ts.to_string();
                        }
                    }
                }
            }
            "env_snapshot" => {
                // 环境快照，可能包含时间戳
                if let Some(pl) = obj.get("payload") {
                    if let Some(ts) = pl.get("captured_at").and_then(|v| v.as_str()) {
                        if ts > latest_timestamp.as_str() {
                            latest_timestamp = ts.to_string();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(JournalData {
        user_count,
        ai_count,
        latest_timestamp,
        cwd,
        message_lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_empty_journal() {
        let dir = std::env::temp_dir().join("ai-session-test-journal-empty");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.journal.jsonl");
        std::fs::write(&path, "").unwrap();

        let data = parse_jcode_journal(&path).unwrap();
        assert_eq!(data.user_count, 0);
        assert_eq!(data.ai_count, 0);
        assert!(data.latest_timestamp.is_empty());
        assert!(data.cwd.is_none());
        assert!(data.message_lines.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_journal_with_messages() {
        let dir = std::env::temp_dir().join("ai-session-test-journal-msgs");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.journal.jsonl");

        let content = r#"{"type":"session_meta","payload":{"id":"sess_1","timestamp":"2025-02-01T00:00:00Z","cwd":"/new/project","model_provider":"anthropic"}}
{"type":"response_item","payload":{"type":"message","role":"user","content":"hello again","timestamp":"2025-02-01T00:01:00Z"}}
{"type":"response_item","payload":{"type":"message","role":"assistant","content":"hi again","timestamp":"2025-02-01T00:02:00Z"}}
{"type":"env_snapshot","payload":{"captured_at":"2025-02-01T00:00:30Z"}}
"#;

        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        drop(f);

        let data = parse_jcode_journal(&path).unwrap();
        assert_eq!(data.user_count, 1);
        assert_eq!(data.ai_count, 1);
        assert_eq!(data.latest_timestamp, "2025-02-01T00:02:00Z");
        assert_eq!(data.cwd.unwrap(), "/new/project");
        assert_eq!(data.message_lines.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_journal_skips_invalid_lines() {
        let dir = std::env::temp_dir().join("ai-session-test-journal-bad");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.journal.jsonl");

        let content = "not json\n{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":\"hi\"}}\n";
        std::fs::write(&path, content).unwrap();

        let data = parse_jcode_journal(&path).unwrap();
        assert_eq!(data.user_count, 1);
        assert_eq!(data.message_lines.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
