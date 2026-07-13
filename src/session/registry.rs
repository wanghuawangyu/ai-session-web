use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;
use walkdir::WalkDir;

use crate::error::{AppError, Result};
use super::{SessionMeta, SessionSource, jcode, codex, continue_};

/// Session 注册表，管理所有来源的 session
#[derive(Debug, Default)]
pub struct SessionRegistry {
    /// key: "source:session_id" -> SessionMeta
    sessions: HashMap<String, SessionMeta>,
    /// 按来源分组
    by_source: HashMap<SessionSource, Vec<String>>,
}

impl SessionRegistry {
    /// 扫描所有目录，构建注册表
    pub fn scan(
        jcode_dir: Option<&PathBuf>,
        codex_dir: Option<&PathBuf>,
        continue_dir: Option<&PathBuf>,
    ) -> Result<Self> {
        let mut registry = SessionRegistry::default();

        if let Some(dir) = jcode_dir {
            registry.scan_dir(dir, SessionSource::Jcode, |p| {
                // Jcode: *.json, 跳过 .bak 和 .journal.jsonl
                if let Some(ext) = p.extension() {
                    if ext == "json" {
                        let name = p.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
                        if !name.ends_with(".journal") {
                            return true;
                        }
                    }
                }
                false
            });
        }

        if let Some(dir) = codex_dir {
            registry.scan_dir(dir, SessionSource::Codex, |p| {
                p.extension().map(|e| e == "jsonl").unwrap_or(false)
            });
        }

        if let Some(dir) = continue_dir {
            registry.scan_dir(dir, SessionSource::Continue, |p| {
                if let Some(ext) = p.extension() {
                    if ext == "json" {
                        let name = p.file_name().map(|s| s.to_string_lossy()).unwrap_or_default();
                        // 跳过 sessions.json（Continue 的索引文件）
                        return name != "sessions.json";
                    }
                }
                false
            });
        }

        Ok(registry)
    }

    fn scan_dir<F>(&mut self, dir: &Path, source: SessionSource, filter: F)
    where
        F: Fn(&Path) -> bool,
    {
        if !dir.exists() {
            info!("Directory not found, skipping: {}", dir.display());
            return;
        }

        for entry in WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() || !filter(path) {
                continue;
            }

            let meta = match source {
                SessionSource::Jcode => jcode::parse_jcode(path),
                SessionSource::Codex => codex::parse_codex(path),
                SessionSource::Continue => continue_::parse_continue(path),
            };

            match meta {
                Ok(m) => {
                    let key = format!("{}:{}", m.source, m.session_id);
                    self.by_source.entry(m.source.clone()).or_default().push(key.clone());
                    self.sessions.insert(key, m);
                }
                Err(e) => {
                    info!("Failed to parse {}: {} ({})", source, path.display(), e);
                }
            }
        }
    }

    /// 列出所有 session
    pub fn list_all(&self) -> Vec<&SessionMeta> {
        self.sessions.values().collect()
    }

    /// 按来源列出 session
    pub fn list_by_source(&self, source: &SessionSource) -> Vec<&SessionMeta> {
        self.by_source
            .get(source)
            .map(|keys| {
                keys.iter()
                    .filter_map(|k| self.sessions.get(k))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 获取所有来源的分组列表
    pub fn list_grouped(&self) -> HashMap<SessionSource, Vec<&SessionMeta>> {
        let mut map = HashMap::new();
        for source in &[SessionSource::Jcode, SessionSource::Codex, SessionSource::Continue] {
            let sessions = self.list_by_source(source);
            if !sessions.is_empty() {
                map.insert(source.clone(), sessions);
            }
        }
        map
    }

    /// 获取单个 session
    pub fn get(&self, source: &SessionSource, session_id: &str) -> Option<&SessionMeta> {
        let key = format!("{}:{}", source, session_id);
        self.sessions.get(&key)
    }

    /// 删除 session（删除关联的所有文件）
    pub fn delete(&mut self, source: &SessionSource, session_id: &str) -> Result<()> {
        let key = format!("{}:{}", source, session_id);
        let meta = self.sessions.remove(&key)
            .ok_or_else(|| AppError::SessionNotFound(format!("{}:{}", source, session_id)))?;

        // 从分组中移除
        if let Some(keys) = self.by_source.get_mut(source) {
            keys.retain(|k| k != &key);
        }

        // 删除所有关联文件
        for file in &meta.associated_files {
            if file.exists() {
                std::fs::remove_file(file)
                    .map_err(|e| AppError::Io(e))?;
                info!("Deleted file: {}", file.display());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_mock_jcode(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(format!("{}.json", name));
        let json = r#"{"title":"Test","messages":[{"role":"user","content":"hi"},{"role":"assistant","content":"hello"}],"created_at":"2025-01-01T00:00:00Z","working_dir":"/test","provider_key":"anthropic"}"#;
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        path
    }

    fn create_mock_codex(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(format!("{}.jsonl", name));
        let content = format!(r#"{{"type":"session_meta","payload":{{"id":"{}","timestamp":"2025-01-01T00:00:00Z","cwd":"/test","model_provider":"openai"}}}}
{{"type":"response_item","payload":{{"type":"message","role":"user","content":"hi"}}}}
"#, name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    fn create_mock_continue(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(format!("{}.json", name));
        let json = r#"{"title":"Continue Test","workspaceDirectory":"/test","history":[{"message":{"role":"user","content":"hi"}}]}"#;
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_registry_scan() {
        let base = std::env::temp_dir().join("ai-session-test-registry");
        let jdir = base.join("jcode");
        let cdir = base.join("codex");
        let ndir = base.join("continue");
        std::fs::create_dir_all(&jdir).unwrap();
        std::fs::create_dir_all(&cdir).unwrap();
        std::fs::create_dir_all(&ndir).unwrap();

        create_mock_jcode(&jdir, "session_a");
        create_mock_jcode(&jdir, "session_b");
        create_mock_codex(&cdir, "codex_sess_1");
        create_mock_continue(&ndir, "cont_sess_1");

        let registry = SessionRegistry::scan(
            Some(&jdir),
            Some(&cdir),
            Some(&ndir),
        ).unwrap();

        assert_eq!(registry.list_all().len(), 4);
        assert_eq!(registry.list_by_source(&SessionSource::Jcode).len(), 2);
        assert_eq!(registry.list_by_source(&SessionSource::Codex).len(), 1);
        assert_eq!(registry.list_by_source(&SessionSource::Continue).len(), 1);

        let grouped = registry.list_grouped();
        assert_eq!(grouped.len(), 3);

        let _ = std::fs::remove_dir_all(&base);
    }
}
