//! ERROR-HANDLING PATTERN
//!
//! Idiomatic axum apps return `Result<T, AppError>` from handlers and lean on the
//! `?` operator. For that to work two things are needed:
//!   1. a single error enum that every failure converts into, and
//!   2. an `IntoResponse` impl so axum knows the HTTP status/body for each variant.
//!
//! This keeps handlers free of manual status-code juggling: a `repo` call can
//! `?`-propagate a `sqlx::Error` and it lands here as a clean 500, while domain
//! errors (NotFound / Gone / BadRequest) map to their intended codes.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound, // -> 404

    #[error("{0}")]
    Gone(String), // -> 410 (deleted or expired)

    #[error("{0}")]
    BadRequest(String), // -> 400 (validation)

    #[error("database error: {0}")]
    Db(#[from] sqlx::Error), // -> 500 (the #[from] enables `?` on sqlx calls)

    #[error("{0}")]
    Internal(String), // -> 500
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not Found".to_string()),
            AppError::Gone(m) => (StatusCode::GONE, m),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::Db(e) => {
                // Log the real cause server-side; never leak SQL details to clients.
                eprintln!("[db error] {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string())
            }
            AppError::Internal(m) => {
                eprintln!("[internal error] {m}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string())
            }
        };
        (status, Json(json!({ "detail": message }))).into_response()
    }
}
