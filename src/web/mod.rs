use askama::Template;
use axum::{extract::State, response::{Html, IntoResponse, Response}};
use std::sync::Arc;

use crate::api::AppState;
use crate::config::CliDirDisplay;
use crate::error::AppError;

const STYLE_CSS: &str = include_str!("assets/style.css");
const APP_JS: &str = include_str!("assets/app.js");

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate;

#[derive(Template)]
#[template(path = "config.html")]
struct ConfigTemplate {
    port: u16,
    host: String,
    cli_dirs: Vec<CliDirDisplay>,
    log_path: String,
    log_level: String,
}

pub async fn index_handler(
    State(_state): State<Arc<AppState>>,
) -> std::result::Result<Html<String>, AppError> {
    let tmpl = IndexTemplate {};
    let rendered = tmpl.render().map_err(|e| AppError::Config(e.to_string()))?;
    Ok(Html(rendered))
}

pub async fn config_handler(
    State(state): State<Arc<AppState>>,
) -> std::result::Result<Html<String>, AppError> {
    let cfg = &state.display_config;

    let log_path = cfg.log_path.clone().unwrap_or_else(|| "（未设置，输出到控制台）".to_string());

    let tmpl = ConfigTemplate {
        port: cfg.port,
        host: cfg.host.clone(),
        cli_dirs: cfg.cli_dirs.clone(),
        log_path,
        log_level: cfg.log_level.clone(),
    };
    let rendered = tmpl.render().map_err(|e| AppError::Config(e.to_string()))?;
    Ok(Html(rendered))
}

/// Serve compiled-in CSS as a static file
pub async fn style_handler() -> Response {
    (
        [("content-type", "text/css; charset=utf-8")],
        STYLE_CSS,
    )
        .into_response()
}

/// Serve compiled-in JS as a static file
pub async fn script_handler() -> Response {
    (
        [("content-type", "application/javascript; charset=utf-8")],
        APP_JS,
    )
        .into_response()
}
