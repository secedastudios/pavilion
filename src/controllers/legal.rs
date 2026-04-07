use askama::Template;
use axum::response::Response;

use crate::error::AppError;
use crate::templates::render_or_error;

#[derive(Template)]
#[template(path = "pages/terms.html")]
struct TermsTemplate;

#[derive(Template)]
#[template(path = "pages/privacy.html")]
struct PrivacyTemplate;

#[derive(Template)]
#[template(path = "pages/content_policy.html")]
struct ContentPolicyTemplate;

pub async fn terms() -> Result<Response, AppError> {
    render_or_error(&TermsTemplate)
}

pub async fn privacy() -> Result<Response, AppError> {
    render_or_error(&PrivacyTemplate)
}

pub async fn content_policy() -> Result<Response, AppError> {
    render_or_error(&ContentPolicyTemplate)
}
