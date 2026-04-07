use askama::Template;
use axum::response::Response;

use crate::auth::claims::Claims;
use crate::auth::middleware::OptionalClaims;
use crate::error::AppError;
use crate::templates::render_or_error;

#[derive(Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate {
    claims: Option<Claims>,
}

pub async fn index(OptionalClaims(claims): OptionalClaims) -> Result<Response, AppError> {
    render_or_error(&HomeTemplate { claims })
}
