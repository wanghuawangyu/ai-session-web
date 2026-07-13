use askama::Template;
use axum::{extract::State, response::Html};
use std::sync::Arc;

use crate::api::AppState;
use crate::error::AppError;

const STYLE_CSS: &str = include_str!("assets/style.css");

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    style_css: &'static str,
    app_js: &'static str,
}

#[derive(Template)]
#[template(path = "config.html")]
struct ConfigTemplate {
    style_css: &'static str,
    port: u16,
    host: String,
    jcode_dir: String,
    codex_dir: String,
    continue_dir: String,
    log_path: String,
    log_level: String,
}

pub async fn index_handler(
    State(_state): State<Arc<AppState>>,
) -> std::result::Result<Html<String>, AppError> {
    let tmpl = IndexTemplate {
        style_css: STYLE_CSS,
        app_js: include_str!("assets/app.js"),
    };
    let rendered = tmpl.render().map_err(|e| AppError::Config(e.to_string()))?;
    Ok(Html(rendered))
}

pub async fn config_handler(
    State(state): State<Arc<AppState>>,
) -> std::result::Result<Html<String>, AppError> {
    let cfg = &state.display_config;

    let log_path = cfg.log_path.clone().unwrap_or_else(|| "（未设置，输出到控制台）".to_string());

    let tmpl = ConfigTemplate {
        style_css: STYLE_CSS,
        port: cfg.port,
        host: cfg.host.clone(),
        jcode_dir: cfg.jcode_dir.clone(),
        codex_dir: cfg.codex_dir.clone(),
        continue_dir: cfg.continue_dir.clone(),
        log_path,
        log_level: cfg.log_level.clone(),
    };
    let rendered = tmpl.render().map_err(|e| AppError::Config(e.to_string()))?;
    Ok(Html(rendered))
}
