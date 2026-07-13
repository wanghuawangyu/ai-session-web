use std::path::PathBuf;
use clap::Parser;

/// 配置文件路径
#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub jcode_dir: Option<PathBuf>,
    pub codex_dir: Option<PathBuf>,
    pub continue_dir: Option<PathBuf>,
    pub port: Option<u16>,
    pub host: Option<String>,
    pub log: Option<String>,
    pub log_level: Option<String>,
}

impl AppConfig {
    /// 程序内置默认值
    pub fn defaults() -> Self {
        let home = dirs_dir().unwrap_or_else(|| PathBuf::from("."));
        AppConfig {
            jcode_dir: Some(home.join(".jcode").join("sessions")),
            codex_dir: Some(home.join(".codex").join("sessions")),
            continue_dir: Some(home.join(".continue").join("sessions")),
            port: Some(8100),
            host: Some("127.0.0.1".to_string()),
            log: Some(String::new()),
            log_level: Some("info".to_string()),
        }
    }

    fn merge_from(&mut self, other: &AppConfig) {
        if let Some(v) = other.port {
            self.port = Some(v);
        }
        if let Some(ref v) = other.host {
            self.host = Some(v.clone());
        }
        if let Some(ref v) = other.jcode_dir {
            self.jcode_dir = Some(v.clone());
        }
        if let Some(ref v) = other.codex_dir {
            self.codex_dir = Some(v.clone());
        }
        if let Some(ref v) = other.continue_dir {
            self.continue_dir = Some(v.clone());
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
    // Windows: %USERPROFILE%
    // Linux/macOS: $HOME
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

// ============================================================
// CLI 参数
// ============================================================

#[derive(Parser)]
#[command(name = "ai-session-web")]
#[command(about = "Web UI for managing local CLI session files", long_about = None)]
pub struct Cli {
    /// Jcode sessions 目录（默认: ~/.jcode/sessions）
    #[arg(long)]
    pub jcode_dir: Option<String>,

    /// Codex sessions 目录（默认: ~/.codex/sessions）
    #[arg(long)]
    pub codex_dir: Option<String>,

    /// Continue sessions 目录（默认: ~/.continue/sessions）
    #[arg(long)]
    pub continue_dir: Option<String>,

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
        AppConfig {
            jcode_dir: cli.jcode_dir.as_ref().map(PathBuf::from),
            codex_dir: cli.codex_dir.as_ref().map(PathBuf::from),
            continue_dir: cli.continue_dir.as_ref().map(PathBuf::from),
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
    /// 合并配置，优先级 CLI > 默认 INI > 内置默认
    pub fn load(cli: AppConfig, defaults: AppConfig) -> AppConfig {
        let mut result = defaults;
        result.merge_from(&cli);
        result
    }
}

/// 最终合并后的配置展示信息
#[derive(Debug, Clone)]
pub struct ConfigDisplay {
    pub port: u16,
    pub host: String,
    pub jcode_dir: String,
    pub codex_dir: String,
    pub continue_dir: String,
    pub log_path: Option<String>,
    pub log_level: String,
}
