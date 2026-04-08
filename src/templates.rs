//! Askama template rendering helpers for Axum.
//!
//! Askama 0.15 doesn't include built-in Axum integration, so these
//! helpers bridge the gap — rendering a template into an HTML response
//! or an `AppError` on failure.

use askama::Template;
use axum::response::{Html, IntoResponse, Response};

/// Render a template, returning [`AppError`](crate::error::AppError) on failure.
///
/// This is the preferred helper for route handlers since it integrates
/// with the `?` operator and Pavilion's error handling.
///
/// # Example
///
/// ```ignore
/// use pavilion::templates::render_or_error;
///
/// pub async fn my_page() -> Result<Response, AppError> {
///     render_or_error(&MyTemplate { name: "world" })
/// }
/// ```
pub fn render_or_error(template: &impl Template) -> Result<Response, crate::error::AppError> {
    template
        .render()
        .map(|html| Html(html).into_response())
        .map_err(|e| crate::error::AppError::Internal(anyhow::anyhow!("Template error: {e}")))
}
