use std::path::PathBuf;
use clap::Parser;
use crate::session::SessionSource;

/// 单个 CLI 目录配置
#[derive(Debug, Clone)]
pub struct CliDir {
    pub path: PathBuf,
    pub cli_type: SessionSource,
}

/// 配置文件路径
#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub cli_dirs: Vec<CliDir>,
    pub port: Option<u16>,
    pub host: Option<String>,
    pub log: Option<String>,
    pub log_level: Option<String>,
}

impl AppConfig {
    /// 程序内置默认值
    pub fn defaults() -> Self {
        let home = dirs_dir().unwrap_or_else(|| PathBuf::from("."));
        let mut cli_dirs = Vec::new();
        let jcode = home.join(".jcode").join("sessions");
        if jcode.exists() {
            cli_dirs.push(CliDir { path: jcode, cli_type: SessionSource::Jcode });
        }
        let codex = home.join(".codex").join("sessions");
        if codex.exists() {
            cli_dirs.push(CliDir { path: codex, cli_type: SessionSource::Codex });
        }
        let cnt = home.join(".continue").join("sessions");
        if cnt.exists() {
            cli_dirs.push(CliDir { path: cnt, cli_type: SessionSource::Continue });
        }
        // 全都不存在时也加默认路径（扫描时会跳过不存在的目录）
        if cli_dirs.is_empty() {
            cli_dirs.push(CliDir { path: home.join(".jcode").join("sessions"), cli_type: SessionSource::Jcode });
            cli_dirs.push(CliDir { path: home.join(".codex").join("sessions"), cli_type: SessionSource::Codex });
            cli_dirs.push(CliDir { path: home.join(".continue").join("sessions"), cli_type: SessionSource::Continue });
        }
        AppConfig {
            cli_dirs,
            port: Some(8100),
            host: Some("127.0.0.1".to_string()),
            log: Some(String::new()),
            log_level: Some("info".to_string()),
        }
    }

    fn merge_from(&mut self, other: &AppConfig) {
        if !other.cli_dirs.is_empty() {
            self.cli_dirs = other.cli_dirs.clone();
        }
        if let Some(v) = other.port {
            self.port = Some(v);
        }
        if let Some(ref v) = other.host {
            self.host = Some(v.clone());
        }
        if let Some(ref v) = other.log {
            self.log = Some(v.clone());
        }
        if let Some(ref v) = other.log_level {
            self.log_level = Some(v.clone());
        }
    }
}

fn dirs_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// 根据目录路径推断 CLI 类型（检查路径中是否包含关键字）
fn infer_cli_type(path_str: &str) -> Option<SessionSource> {
    let lower = path_str.to_lowercase().replace('\\', "/");
    // 按优先级匹配
    if lower.contains("jcode") {
        Some(SessionSource::Jcode)
    } else if lower.contains("codex") {
        Some(SessionSource::Codex)
    } else if lower.contains("continue") {
        Some(SessionSource::Continue)
    } else {
        None
    }
}

// ============================================================
// CLI 参数
// ============================================================

#[derive(Parser)]
#[command(name = "ai-session-web")]
#[command(about = "Web UI for managing local CLI session files", long_about = None)]
pub struct Cli {
    /// CLI 会话目录列表，多个用空格分隔。根据目录名中的关键字自动识别类型（jcode / codex / continue）
    /// 示例：--cli-dirs "C:\Users\me\.jcode\sessions" "C:\Users\me\.codex\sessions"
    #[arg(long, num_args = 0..)]
    pub cli_dirs: Vec<String>,

    /// 监听端口
    #[arg(short, long)]
    pub port: Option<u16>,

    /// 监听 IP 地址
    #[arg(short = 'H', long)]
    pub host: Option<String>,

    /// 日志文件路径
    #[arg(long)]
    pub log: Option<String>,

    /// 日志级别: trace, debug, info, warn, error
    #[arg(long)]
    pub log_level: Option<String>,
}

impl From<&Cli> for AppConfig {
    fn from(cli: &Cli) -> Self {
        // 支持空格分隔和逗号分隔两种方式
        let cli_dirs = cli.cli_dirs.iter()
            .flat_map(|s| s.split(','))
            .filter(|s| !s.is_empty())
            .filter_map(|s| {
                let path = PathBuf::from(s.trim());
                let cli_type = infer_cli_type(s.trim())?;
                Some(CliDir { path, cli_type })
            }).collect();

        AppConfig {
            cli_dirs,
            port: cli.port,
            host: cli.host.clone(),
            log: cli.log.clone(),
            log_level: cli.log_level.clone(),
        }
    }
}

// ============================================================
// ConfigLoader
// ============================================================

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load(cli: AppConfig, defaults: AppConfig) -> AppConfig {
        let mut result = defaults;
        result.merge_from(&cli);
        result
    }
}

/// CLI 目录展示信息
#[derive(Debug, Clone)]
pub struct CliDirDisplay {
    pub label: String,
    pub path: String,
}

/// 最终合并后的配置展示信息
#[derive(Debug, Clone)]
pub struct ConfigDisplay {
    pub port: u16,
    pub host: String,
    pub cli_dirs: Vec<CliDirDisplay>,
    pub log_path: Option<String>,
    pub log_level: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_dirs_parsing() {
        // With space-separated args (each dir is a separate arg)
        let cli = Cli::try_parse_from(&[
            "ai-session-web",
            "--cli-dirs", "C:\\jcode\\sessions", "C:\\codex\\sessions", "C:\\continue\\sessions",
            "--port", "8080",
        ]).expect("CLI parsing should succeed");

        let config = AppConfig::from(&cli);
        assert_eq!(config.cli_dirs.len(), 3);
        assert_eq!(config.cli_dirs[0].cli_type, SessionSource::Jcode);
        assert_eq!(config.cli_dirs[1].cli_type, SessionSource::Codex);
        assert_eq!(config.cli_dirs[2].cli_type, SessionSource::Continue);
        assert_eq!(config.port, Some(8080));
    }

    #[test]
    fn test_cli_dirs_comma_separated() {
        // With comma-separated in a single arg
        let cli = Cli::try_parse_from(&[
            "ai-session-web",
            "--cli-dirs", "C:\\jcode\\sessions,C:\\codex\\sessions",
            "--port", "8080",
        ]).expect("CLI parsing should succeed");

        let config = AppConfig::from(&cli);
        assert_eq!(config.cli_dirs.len(), 2);
        assert_eq!(config.cli_dirs[0].cli_type, SessionSource::Jcode);
        assert_eq!(config.cli_dirs[1].cli_type, SessionSource::Codex);
    }

    #[test]
    fn test_infer_cli_type() {
        assert_eq!(infer_cli_type("C:\\Users\\me\\.jcode\\sessions"), Some(SessionSource::Jcode));
        assert_eq!(infer_cli_type("/home/user/.jcode"), Some(SessionSource::Jcode));
        assert_eq!(infer_cli_type("C:\\Users\\me\\.codex\\sessions"), Some(SessionSource::Codex));
        assert_eq!(infer_cli_type("C:\\Users\\me\\.continue\\sessions"), Some(SessionSource::Continue));
        assert_eq!(infer_cli_type("C:\\unknown\\path"), None);
    }

    #[test]
    fn test_defaults_always_fills() {
        let defaults = AppConfig::defaults();
        assert!(!defaults.cli_dirs.is_empty(), "Defaults should have at least one CLI dir");
        assert_eq!(defaults.port, Some(8100));
    }
}
