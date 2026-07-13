use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Config error: {0}")]
    Config(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            AppError::SessionNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::Parse(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::Config(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        let body = Json(json!({
            "success": false,
            "error": message,
        }));
        (status, body).into_response()
    }
}
