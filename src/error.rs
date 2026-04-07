//! Application error types with automatic HTTP status code mapping.
//!
//! All controller handlers return `Result<Response, AppError>`. The
//! `IntoResponse` implementation maps each variant to the appropriate
//! HTTP status code, logs server errors, and returns a generic message
//! to the client (no internal details leak in production).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Unified error type for all Pavilion route handlers.
///
/// Implements `IntoResponse` so handlers can use `?` directly.
/// Server-side errors (Database, Internal) are logged via `tracing::error`
/// before returning a generic 500 to the client.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Resource not found. Returns 404.
    #[error("Not found")]
    NotFound,

    /// Authenticated person lacks permission. Returns 403.
    #[error("Forbidden")]
    Forbidden,

    /// No valid authentication token. Returns 401.
    #[error("Unauthorized")]
    Unauthorized,

    /// Client-provided data is invalid. Returns 422.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Content access blocked by licensing rules. Returns 403.
    #[error("License violation: {0}")]
    LicenseViolation(String),

    /// SurrealDB query or connection error. Returns 500. Logged server-side.
    #[error(transparent)]
    Database(#[from] surrealdb::Error),

    /// Catch-all for unexpected failures. Returns 500. Logged server-side.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Forbidden | AppError::LicenseViolation(_) => StatusCode::FORBIDDEN,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Database(err) => {
                tracing::error!(error = %err, "Database error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::Internal(err) => {
                tracing::error!(error = %err, "Internal server error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        (status, status.canonical_reason().unwrap_or("Error")).into_response()
    }
}
