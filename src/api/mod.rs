use axum::{
    routing::{get, delete},
    Router,
};
use std::sync::Arc;

use crate::session::registry::SessionRegistry;
use crate::config::ConfigDisplay;
use crate::web;

mod handlers;
mod response;

pub use response::ApiResponse;

pub struct AppState {
    pub registry: tokio::sync::Mutex<SessionRegistry>,
    pub display_config: ConfigDisplay,
}

pub fn create_router(
    registry: SessionRegistry,
    display_config: ConfigDisplay,
) -> Router {
    let state = Arc::new(AppState {
        registry: tokio::sync::Mutex::new(registry),
        display_config,
    });

    Router::new()
        .route("/api/sessions", get(handlers::list_sessions))
        .route("/api/sessions/{source}/{session_id}", delete(handlers::delete_session))
        .route("/api/sessions/{source}/{session_id}/json", get(handlers::get_session_json))
        .route("/", get(web::index_handler))
        .route("/config", get(web::config_handler))
        .route("/style.css", get(web::style_handler))
        .route("/app.js", get(web::script_handler))
        .with_state(state)
}
