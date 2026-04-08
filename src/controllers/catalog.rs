use std::collections::HashMap;
use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use serde::Deserialize;

use crate::auth::claims::Claims;
use crate::auth::middleware::OptionalClaims;
use crate::error::AppError;
use crate::models::film::{Film, FilmView};
use crate::models::license::LicenseView;
use crate::router::AppState;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/catalog.html")]
struct CatalogTemplate {
    films: Vec<FilmView>,
    query: String,
    genre_filter: String,
    year_filter: String,
    language_filter: String,
    claims: Option<Claims>,
}

#[derive(Template)]
#[template(path = "pages/catalog_film.html")]
struct CatalogFilmTemplate {
    film: FilmView,
    licenses: Vec<LicenseView>,
    claims: Option<Claims>,
}

// ── Query params ───────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct CatalogQuery {
    pub q: Option<String>,
    pub genre: Option<String>,
    pub year: Option<String>,
    pub language: Option<String>,
}

// ── Handlers ───────────────────────────────────────────────

pub async fn browse(
    State(state): State<Arc<AppState>>,
    OptionalClaims(claims): OptionalClaims,
    Query(params): Query<CatalogQuery>,
) -> Result<Response, AppError> {
    // Build query with optional filters
    let mut conditions = vec!["status = 'published'".to_string()];
    let mut bindings: HashMap<String, String> = HashMap::new();

    if let Some(q) = &params.q
        && !q.trim().is_empty()
    {
        conditions.push(
            "(string::lowercase(title) CONTAINS string::lowercase($search_q) \
                 OR string::lowercase(synopsis) CONTAINS string::lowercase($search_q))"
                .to_string(),
        );
        bindings.insert("search_q".into(), q.trim().to_string());
    }

    if let Some(genre) = &params.genre
        && !genre.trim().is_empty()
    {
        conditions.push("$genre IN genres".to_string());
        bindings.insert("genre".into(), genre.trim().to_string());
    }

    if let Some(year) = &params.year
        && !year.trim().is_empty()
        && let Ok(y) = year.parse::<i64>()
    {
        conditions.push("year = $year_filter".to_string());
        bindings.insert("year_filter".into(), y.to_string());
    }

    if let Some(lang) = &params.language
        && !lang.trim().is_empty()
    {
        conditions
            .push("string::lowercase(language) = string::lowercase($lang_filter)".to_string());
        bindings.insert("lang_filter".into(), lang.trim().to_string());
    }

    // Only show films that have at least one active license
    // licensed_via: FROM film TO license, so from film: ->licensed_via->license
    conditions.push("count(->licensed_via->license[WHERE active = true]) > 0".to_string());

    let where_clause = conditions.join(" AND ");
    let query_str =
        format!("SELECT * FROM film WHERE {where_clause} ORDER BY created_at DESC LIMIT 50");

    let films: Vec<Film> = state.db.query(&query_str).bind(bindings).await?.take(0)?;
    let film_views: Vec<FilmView> = films.into_iter().map(FilmView::from).collect();

    render_or_error(&CatalogTemplate {
        films: film_views,
        query: params.q.unwrap_or_default(),
        genre_filter: params.genre.unwrap_or_default(),
        year_filter: params.year.unwrap_or_default(),
        language_filter: params.language.unwrap_or_default(),
        claims,
    })
}

pub async fn film_detail(
    State(state): State<Arc<AppState>>,
    OptionalClaims(claims): OptionalClaims,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &id).await?;

    if film.status != "published" {
        return Err(AppError::NotFound);
    }

    let film_id = surrealdb::types::RecordId::new("film", id.as_str());
    let licenses = crate::licensing::rights::licenses_for_film(&state.db, &film_id).await?;
    let active_licenses: Vec<LicenseView> = licenses
        .into_iter()
        .filter(|l| l.active)
        .map(LicenseView::from)
        .collect();

    render_or_error(&CatalogFilmTemplate {
        film: film.into(),
        licenses: active_licenses,
        claims,
    })
}
