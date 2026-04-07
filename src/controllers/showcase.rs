use std::sync::Arc;

use askama::Template;
use axum::extract::State;
use axum::response::Response;

use crate::auth::middleware::OptionalClaims;
use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::models::film::{Film, FilmView};
use crate::router::AppState;
use crate::templates::render_or_error;

/// The Pavilion Showcase is a reference implementation of a curated streaming
/// site. It shows all published, licensed films and demonstrates the full
/// viewer experience: browsing, film detail, ratings, and playback.

#[derive(Template)]
#[template(path = "pages/showcase.html")]
struct ShowcaseTemplate {
    films: Vec<FilmView>,
    claims: Option<Claims>,
}

pub async fn home(
    State(state): State<Arc<AppState>>,
    OptionalClaims(claims): OptionalClaims,
) -> Result<Response, AppError> {
    // Show all published films with active licenses
    let films: Vec<Film> = state.db
        .query(
            "SELECT * FROM film \
             WHERE status = 'published' \
               AND count(->licensed_via->license[WHERE active = true]) > 0 \
             ORDER BY created_at DESC LIMIT 50"
        )
        .await?
        .take(0)?;

    let views: Vec<FilmView> = films.into_iter().map(FilmView::from).collect();

    render_or_error(&ShowcaseTemplate { films: views, claims })
}
