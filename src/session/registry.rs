use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use serde::Serialize;
use tracing::info;
use walkdir::WalkDir;

use crate::config::CliDir;
use crate::error::{AppError, Result};
use super::{SessionMeta, SessionSource, jcode, codex, continue_, file_mtime_iso};

// ============================================================
// API 响应类型
// ============================================================

#[derive(Debug, Clone, Serialize)]
pub struct SessionEntry {
    pub session_id: String,
    pub title: String,
    pub name: String,
    pub total_messages: usize,
    pub user_messages: usize,
    pub ai_messages: usize,
    pub created_at: String,
    pub updated_at: String,
    #[serde(rename = "effective_updated_at")]
    pub effective_updated_at: String,
    pub working_dir: String,
    pub provider: String,
    pub has_custom_title: bool,
}

impl From<&SessionMeta> for SessionEntry {
    fn from(meta: &SessionMeta) -> Self {
        SessionEntry {
            session_id: meta.session_id.clone(),
            title: meta.title.clone(),
            name: meta.name.clone(),
            total_messages: meta.total_messages,
            user_messages: meta.user_messages,
            ai_messages: meta.ai_messages,
            created_at: meta.created_at.clone(),
            updated_at: meta.updated_at.clone(),
            effective_updated_at: meta.effective_updated_at.clone(),
            working_dir: meta.working_dir.clone(),
            provider: meta.provider.clone(),
            has_custom_title: meta.has_custom_title,
        }
    }
}

/// 残缺会话条目（文件存在但无法解析）
#[derive(Debug, Clone, Serialize)]
pub struct BrokenEntry {
    pub session_id: String,
    pub file_path: String,
    pub effective_updated_at: String,
}

/// 一个排序分组段（主会话 / 关联临时 / 其他临时 / 残缺）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSection {
    pub section_type: String,
    pub title: String,
    pub parent_session_id: Option<String>,
    pub parent_session_name: Option<String>,
    pub sessions: Vec<SessionEntry>,
    pub broken: Vec<BrokenEntry>,
}

/// 每个 CLI 来源的排序分组
#[derive(Debug, Clone, Serialize)]
pub struct SortedSessionGroup {
    pub source: String,
    pub sections: Vec<SessionSection>,
}

// ============================================================
// SessionRegistry
// ============================================================

/// Session 注册表
#[derive(Debug, Default)]
pub struct SessionRegistry {
    /// key: "source:session_id" -> SessionMeta
    sessions: HashMap<String, SessionMeta>,
    /// 按来源分组
    by_source: HashMap<SessionSource, Vec<String>>,
    /// CLI 目录配置（用于排序和残缺检测）
    cli_dirs: Vec<CliDir>,
}

impl SessionRegistry {
    /// 扫描所有 CLI 目录，构建注册表
    pub fn scan(cli_dirs: &[CliDir]) -> Result<Self> {
        let mut registry = SessionRegistry::default();
        registry.cli_dirs = cli_dirs.to_vec();

        for cli_dir in cli_dirs {
            if !cli_dir.path.exists() {
                info!("Directory not found, skipping: {}", cli_dir.path.display());
                continue;
            }

            match cli_dir.cli_type {
                SessionSource::Jcode => {
                    registry.scan_dir(&cli_dir.path, SessionSource::Jcode, |p| {
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
                SessionSource::Codex => {
                    registry.scan_dir(&cli_dir.path, SessionSource::Codex, |p| {
                        p.extension().map(|e| e == "jsonl").unwrap_or(false)
                    });
                }
                SessionSource::Continue => {
                    registry.scan_dir(&cli_dir.path, SessionSource::Continue, |p| {
                        if let Some(ext) = p.extension() {
                            if ext == "json" {
                                let name = p.file_name().map(|s| s.to_string_lossy()).unwrap_or_default();
                                return name != "sessions.json";
                            }
                        }
                        false
                    });
                }
            }
        }

        Ok(registry)
    }

    fn scan_dir<F>(&mut self, dir: &Path, source: SessionSource, filter: F)
    where
        F: Fn(&Path) -> bool,
    {
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

    // ============================================================
    // 排序输出（核心逻辑）
    // ============================================================

    /// 获取排序后的分组列表，按 --cli-dirs 顺序、主会话→关联临时→其他临时→残缺
    pub fn sorted_list(&self) -> Vec<SortedSessionGroup> {
        let mut groups: Vec<SortedSessionGroup> = Vec::new();

        for cli_dir in &self.cli_dirs {
            let source = &cli_dir.cli_type;
            let all_sessions = self.list_by_source(source);

            // --- 按 has_custom_title 和 working_dir 分桶 ---
            let main_sessions: Vec<&SessionMeta> = all_sessions.iter()
                .filter(|s| s.has_custom_title)
                .cloned()
                .collect();

            let temp_sessions: Vec<&SessionMeta> = all_sessions.iter()
                .filter(|s| !s.has_custom_title)
                .cloned()
                .collect();

            // 收集所有主会话的 working_dir 集合
            let main_wds: HashSet<&str> = main_sessions.iter()
                .map(|s| s.working_dir.as_str())
                .filter(|wd| !wd.is_empty())
                .collect();

            // 分 A 类（关联临时）和 B 类（其他临时）
            let mut linked_temps: Vec<&SessionMeta> = Vec::new();
            let mut unlinked_temps: Vec<&SessionMeta> = Vec::new();

            for s in &temp_sessions {
                if !s.working_dir.is_empty() && main_wds.contains(s.working_dir.as_str()) {
                    linked_temps.push(*s);
                } else {
                    unlinked_temps.push(*s);
                }
            }

            // --- 按 effective_updated_at 降序排列 ---
            let sort_desc = |a: &&SessionMeta, b: &&SessionMeta| {
                b.effective_updated_at.cmp(&a.effective_updated_at)
            };

            let mut sorted_main = main_sessions.clone();
            sorted_main.sort_by(sort_desc);

            let mut sorted_linked = linked_temps.clone();
            sorted_linked.sort_by(sort_desc);

            let mut sorted_unlinked = unlinked_temps.clone();
            sorted_unlinked.sort_by(sort_desc);

            // --- 构建 sections ---
            let mut sections: Vec<SessionSection> = Vec::new();

            // 1. 主会话
            if !sorted_main.is_empty() {
                sections.push(SessionSection {
                    section_type: "main".to_string(),
                    title: "主会话".to_string(),
                    parent_session_id: None,
                    parent_session_name: None,
                    sessions: sorted_main.iter().map(|s| SessionEntry::from(*s)).collect(),
                    broken: Vec::new(),
                });
            }

            // 2. 关联临时（按父会话分组）
            if !sorted_linked.is_empty() {
                // 找出匹配的主会话
                for main_s in &sorted_main {
                    let linked: Vec<&SessionMeta> = sorted_linked.iter()
                        .filter(|s| s.working_dir == main_s.working_dir)
                        .cloned()
                        .collect();
                    if !linked.is_empty() {
                        sections.push(SessionSection {
                            section_type: "linked_temp".to_string(),
                            title: format!("临时会话（关联 {}）", main_s.name),
                            parent_session_id: Some(main_s.session_id.clone()),
                            parent_session_name: Some(if main_s.name.is_empty() { main_s.session_id.clone() } else { main_s.name.clone() }),
                            sessions: linked.iter().map(|s| SessionEntry::from(*s)).collect(),
                            broken: Vec::new(),
                        });
                    }
                }
                // 如果有 linked temp 但匹配不到 main（边界情况），放在最后
                let accounted: HashSet<String> = sections.iter()
                    .filter(|s| s.section_type == "linked_temp")
                    .flat_map(|s| s.sessions.iter().map(|e| e.session_id.clone()))
                    .collect();
                let remaining: Vec<&SessionMeta> = sorted_linked.iter()
                    .filter(|s| !accounted.contains(&s.session_id))
                    .cloned()
                    .collect();
                if !remaining.is_empty() {
                    sections.push(SessionSection {
                        section_type: "unlinked_temp".to_string(),
                        title: "临时会话（关联-未匹配）".to_string(),
                        parent_session_id: None,
                        parent_session_name: None,
                        sessions: remaining.iter().map(|s| SessionEntry::from(*s)).collect(),
                        broken: Vec::new(),
                    });
                }
            }

            // 3. 其他临时
            if !sorted_unlinked.is_empty() {
                sections.push(SessionSection {
                    section_type: "unlinked_temp".to_string(),
                    title: "其他临时会话".to_string(),
                    parent_session_id: None,
                    parent_session_name: None,
                    sessions: sorted_unlinked.iter().map(|s| SessionEntry::from(*s)).collect(),
                    broken: Vec::new(),
                });
            }

            // 4. 残缺会话
            let broken = self.detect_broken(source, &cli_dir.path);
            if !broken.is_empty() {
                sections.push(SessionSection {
                    section_type: "broken".to_string(),
                    title: "残缺会话".to_string(),
                    parent_session_id: None,
                    parent_session_name: None,
                    sessions: Vec::new(),
                    broken,
                });
            }

            if !sections.is_empty() {
                groups.push(SortedSessionGroup {
                    source: source.to_string(),
                    sections,
                });
            }
        }

        groups
    }

    /// 检测残缺会话：目录中已有文件但未解析成功的 session
    fn detect_broken(&self, source: &SessionSource, dir: &Path) -> Vec<BrokenEntry> {
        if !dir.exists() {
            return Vec::new();
        }

        let existing_ids: HashSet<&str> = self.list_by_source(source)
            .iter()
            .map(|s| s.session_id.as_str())
            .collect();

        let mut broken: Vec<BrokenEntry> = Vec::new();

        for entry in WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // 检查扩展名
            match path.extension().and_then(|e| e.to_str()) {
                Some(e) if e == "json" || e == "jsonl" => {}
                _ => continue,
            }

            let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            // 跳过索引文件和非会话文件
            if file_name == "sessions.json" || file_name.ends_with(".journal.jsonl") || file_name.ends_with(".bak") {
                continue;
            }

            let session_id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if session_id.is_empty() {
                continue;
            }

            // 如果已成功解析，跳过
            if existing_ids.contains(session_id) {
                continue;
            }

            let mtime = file_mtime_iso(path).unwrap_or_default();

            broken.push(BrokenEntry {
                session_id: session_id.to_string(),
                file_path: path.to_string_lossy().to_string(),
                effective_updated_at: mtime,
            });
        }

        // 残缺按 time 降序
        broken.sort_by(|a, b| b.effective_updated_at.cmp(&a.effective_updated_at));

        broken
    }

    // ============================================================
    // 查询
    // ============================================================

    pub fn list_all(&self) -> Vec<&SessionMeta> {
        self.sessions.values().collect()
    }

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

    pub fn get(&self, source: &SessionSource, session_id: &str) -> Option<&SessionMeta> {
        let key = format!("{}:{}", source, session_id);
        self.sessions.get(&key)
    }

    // ============================================================
    // 删除（增强版：按 session_id 扫描目录）
    // ============================================================

    pub fn delete(&mut self, source: &SessionSource, session_id: &str) -> Result<Vec<PathBuf>> {
        let key = format!("{}:{}", source, session_id);
        let meta = self.sessions.remove(&key)
            .ok_or_else(|| AppError::SessionNotFound(format!("{}:{}", source, session_id)))?;

        // 从分组中移除
        if let Some(keys) = self.by_source.get_mut(source) {
            keys.retain(|k| k != &key);
        }

        let mut deleted: Vec<PathBuf> = Vec::new();

        // 找到会话所在目录
        if let Some(parent) = meta.file_path.parent() {
            if parent.exists() {
                // 扫描目录中所有文件名包含 session_id 且扩展名在白名单内的文件
                let allowed_exts: HashSet<&str> = ["json", "jsonl", "bak"].iter().cloned().collect();

                for entry in WalkDir::new(parent)
                    .follow_links(false)
                    .max_depth(1)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let p = entry.path();
                    if !p.is_file() {
                        continue;
                    }
                    let name = match p.file_name().and_then(|s| s.to_str()) {
                        Some(n) => n,
                        None => continue,
                    };
                    // 文件名包含 session_id
                    if !name.contains(session_id) {
                        continue;
                    }
                    // 扩展名检查
                    let ext = match p.extension().and_then(|e| e.to_str()) {
                        Some(e) => e,
                        None => continue,
                    };
                    if !allowed_exts.contains(ext) {
                        continue;
                    }

                    if p.exists() {
                        std::fs::remove_file(p)
                            .map_err(|e| AppError::Io(e))?;
                        info!("Deleted file: {}", p.display());
                        deleted.push(p.to_path_buf());
                    }
                }
            }
        }

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_json(dir: &Path, name: &str, custom_title: Option<&str>, wd: &str, ts: &str) -> PathBuf {
        let path = dir.join(format!("{}.json", name));
        let title_val = custom_title.unwrap_or("");
        let ct_field = if custom_title.is_some() {
            format!(r#""custom_title": "{}","#, title_val)
        } else {
            String::new()
        };
        let json = format!(
            r#"{{
                "title": "Test",
                {}"messages": [{{"role":"user","content":"hi"}},{{"role":"assistant","content":"hello"}}],
                "created_at": "{}",
                "updated_at": "{}",
                "working_dir": "{}",
                "provider_key": "test"
            }}"#,
            ct_field, ts, ts, wd
        );
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
        let base = std::env::temp_dir().join("ai-session-test-registry2");
        let jdir = base.join("jcode");
        let cdir = base.join("codex");
        let ndir = base.join("continue");
        std::fs::create_dir_all(&jdir).unwrap();
        std::fs::create_dir_all(&cdir).unwrap();
        std::fs::create_dir_all(&ndir).unwrap();

        create_json(&jdir, "session_a", Some("MainA"), "/proj/a", "2025-01-01T00:00:00Z");
        create_json(&jdir, "session_b", None, "/proj/a", "2025-01-02T00:00:00Z");
        create_mock_codex(&cdir, "codex_sess_1");
        create_mock_continue(&ndir, "cont_sess_1");

        let cli_dirs = vec![
            CliDir { path: jdir, cli_type: SessionSource::Jcode },
            CliDir { path: cdir, cli_type: SessionSource::Codex },
            CliDir { path: ndir, cli_type: SessionSource::Continue },
        ];

        let registry = SessionRegistry::scan(&cli_dirs).unwrap();

        assert_eq!(registry.list_all().len(), 4);
        assert_eq!(registry.list_by_source(&SessionSource::Jcode).len(), 2);
        assert_eq!(registry.list_by_source(&SessionSource::Codex).len(), 1);
        assert_eq!(registry.list_by_source(&SessionSource::Continue).len(), 1);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_sorted_order() {
        let base = std::env::temp_dir().join("ai-session-test-sorted");
        let jdir = base.join("jcode");
        std::fs::create_dir_all(&jdir).unwrap();

        // 一个主会话（有 custom_title），一个关联临时（同 working_dir），一个其他临时
        create_json(&jdir, "main_sess", Some("MainSess"), "/proj/x", "2025-01-03T00:00:00Z");
        create_json(&jdir, "temp_linked", None, "/proj/x", "2025-01-02T00:00:00Z");
        create_json(&jdir, "temp_other", None, "/other/path", "2025-01-01T00:00:00Z");

        let cli_dirs = vec![
            CliDir { path: jdir, cli_type: SessionSource::Jcode },
        ];

        let registry = SessionRegistry::scan(&cli_dirs).unwrap();
        let sorted = registry.sorted_list();

        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].source, "jcode");

        let sections = &sorted[0].sections;
        // Should have: main, linked_temp, unlinked_temp
        // (since linked matches main's wd)
        assert!(sections.len() >= 2, "Expected at least 2 sections, got {}", sections.len());

        // First section should be main
        assert_eq!(sections[0].section_type, "main");
        assert_eq!(sections[0].sessions.len(), 1);
        assert_eq!(sections[0].sessions[0].session_id, "main_sess");

        // Linked temp section
        let linked = sections.iter().find(|s| s.section_type == "linked_temp");
        assert!(linked.is_some(), "Expected linked_temp section");
        if let Some(l) = linked {
            assert_eq!(l.sessions.len(), 1);
            assert_eq!(l.sessions[0].session_id, "temp_linked");
        }

        // Unlinked temp section (temp_other has different wd)
        let unlinked = sections.iter().find(|s| s.section_type == "unlinked_temp");
        assert!(unlinked.is_some(), "Expected unlinked_temp section");
        if let Some(u) = unlinked {
            assert_eq!(u.sessions[0].session_id, "temp_other");
        }

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_delete_scans_directory() {
        let base = std::env::temp_dir().join("ai-session-test-delete-scan");
        let jdir = base.join("jcode");
        std::fs::create_dir_all(&jdir).unwrap();

        create_json(&jdir, "sess_to_delete", Some("Del"), "/test", "2025-01-01T00:00:00Z");

        // Create extra files with same session_id prefix
        let extra_jsonl = jdir.join("sess_to_delete.journal.jsonl");
        std::fs::write(&extra_jsonl, "{\"type\":\"test\"}\n").unwrap();
        let extra_bak = jdir.join("sess_to_delete.bak");
        std::fs::write(&extra_bak, "bak").unwrap();
        // A non-matching file that should NOT be deleted
        let other_file = jdir.join("other_session.json");
        std::fs::write(&other_file, "{}").unwrap();

        let cli_dirs = vec![
            CliDir { path: jdir, cli_type: SessionSource::Jcode },
        ];

        let mut registry = SessionRegistry::scan(&cli_dirs).unwrap();
        let deleted = registry.delete(&SessionSource::Jcode, "sess_to_delete").unwrap();

        // Should have deleted 3 files (.json, .journal.jsonl, .bak)
        assert_eq!(deleted.len(), 3, "Expected 3 deleted files, got {}", deleted.len());

        // Other file should still exist
        assert!(other_file.exists(), "Other file should not be deleted");

        let _ = std::fs::remove_dir_all(&base);
    }
}
