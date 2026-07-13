use clap::Parser;
use tracing::info;
use tracing_subscriber::{self, filter::EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt, Layer};

mod api;
mod config;
mod error;
mod session;
mod web;

use config::{AppConfig, ConfigDisplay, ConfigLoader, Cli};
use error::Result;
use session::registry::SessionRegistry;
use api::create_router;

#[tokio::main]
async fn main() -> Result<()> {
    // ---- 0. 解析命令行参数 ----
    let cli = Cli::parse();

    // ---- 1. 构建配置 ----
    let c1 = AppConfig::from(&cli);
    let c4 = AppConfig::defaults();
    let final_config = ConfigLoader::load(c1, c4);

    // ---- 2. 初始化日志 ----
    let log_path = final_config.log.as_deref().filter(|s| !s.is_empty());
    let log_level = final_config.log_level.as_deref().unwrap_or("info");

    let env_filter = EnvFilter::try_new(format!(
        "ai_session_web={},tower_http={}", log_level, log_level
    ))
    .unwrap_or_else(|_| {
        EnvFilter::builder()
            .with_regex(false)
            .parse_lossy("info")
    });

    if let Some(log_path) = log_path {
        let console = fmt::Layer::new()
            .with_writer(std::io::stdout)
            .with_ansi(true)
            .with_filter(env_filter.clone());

        let file_writer = std::fs::File::create(log_path)
            .map_err(|e| error::AppError::Io(e))?;
        let file = fmt::Layer::new()
            .with_writer(std::sync::Arc::new(file_writer))
            .with_ansi(false)
            .with_filter(env_filter);

        tracing_subscriber::registry()
            .with(console)
            .with(file)
            .init();
    } else {
        fmt()
            .with_writer(std::io::stdout)
            .with_ansi(true)
            .with_env_filter(env_filter)
            .init();
    }

    // ---- 3. 打印配置信息 ----
    println!("═══════════════════════════════════════════");
    println!("       AI Session Web 配置信息");
    println!("═══════════════════════════════════════════");
    println!("  监听端口  │ {}", final_config.port.unwrap_or(8100));
    println!("  监听地址  │ {}", final_config.host.as_deref().unwrap_or("127.0.0.1"));
    println!("  Jcode 目录 │ {}", final_config.jcode_dir.as_deref().map(|p| p.display().to_string()).unwrap_or_else(|| "(未设置)".to_string()));
    println!("  Codex 目录 │ {}", final_config.codex_dir.as_deref().map(|p| p.display().to_string()).unwrap_or_else(|| "(未设置)".to_string()));
    println!("  Continue 目录│ {}", final_config.continue_dir.as_deref().map(|p| p.display().to_string()).unwrap_or_else(|| "(未设置)".to_string()));
    let log_display = log_path.unwrap_or("");
    if log_display.is_empty() {
        println!("  日志文件  │ （仅控制台输出）");
    } else {
        println!("  日志文件  │ {}", log_display);
    }
    println!("  日志级别  │ {}", log_level);
    println!("═══════════════════════════════════════════");

    // ---- 4. 扫描会话 ----
    info!("Scanning session directories...");
    let registry = SessionRegistry::scan(
        final_config.jcode_dir.as_ref(),
        final_config.codex_dir.as_ref(),
        final_config.continue_dir.as_ref(),
    )?;
    info!("Found {} sessions total", registry.list_all().len());

    // ---- 5. 构建配置展示信息 ----
    let display_config = ConfigDisplay {
        port: final_config.port.unwrap_or(8100),
        host: final_config.host.clone().unwrap_or_else(|| "127.0.0.1".to_string()),
        jcode_dir: final_config.jcode_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "（未设置）".to_string()),
        codex_dir: final_config.codex_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "（未设置）".to_string()),
        continue_dir: final_config.continue_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "（未设置）".to_string()),
        log_path: final_config.log.clone().filter(|s| !s.is_empty()),
        log_level: log_level.to_string(),
    };

    // ---- 6. 启动 HTTP 服务 ----
    let app = create_router(registry, display_config);

    let host = final_config.host.as_deref().unwrap_or("127.0.0.1");
    let port = final_config.port.unwrap_or(8100);
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Listening on http://{}", addr);
    println!("🚀 服务已启动：http://{}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
