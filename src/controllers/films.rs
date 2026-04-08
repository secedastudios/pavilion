use std::sync::Arc;

use askama::Template;
use axum::Form;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::models::film::{ContentDeclaration, CreateFilm, Film, FilmView};
use crate::router::AppState;
use crate::sse;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/films_index.html")]
struct FilmsIndexTemplate {
    films: Vec<FilmView>,
}

#[derive(Template)]
#[template(path = "pages/film_new.html")]
struct FilmNewTemplate {
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/film_detail.html")]
struct FilmDetailTemplate {
    film: FilmView,
    is_owner: bool,
}

#[derive(Template)]
#[template(path = "partials/film_edit.html")]
struct FilmEditTemplate {
    film: FilmView,
}

#[derive(Template)]
#[template(path = "partials/film_info.html")]
struct FilmInfoTemplate {
    film: FilmView,
    is_owner: bool,
}

// ── Form data ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct FilmForm {
    pub title: String,
    pub synopsis: Option<String>,
    pub year: Option<String>,
    pub genres: Option<String>,
    pub language: Option<String>,
    pub country: Option<String>,
    // Content declaration
    pub declare_copyright: Option<String>,
    pub declare_talent: Option<String>,
    pub declare_no_prohibited: Option<String>,
}

#[derive(Deserialize)]
pub struct FilmUpdateForm {
    pub title: String,
    pub synopsis: Option<String>,
    pub year: Option<String>,
    pub genres: Option<String>,
    pub language: Option<String>,
    pub country: Option<String>,
}

#[derive(Deserialize)]
pub struct StatusForm {
    pub status: String,
}

// ── Handlers ───────────────────────────────────────────────

pub async fn index(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    let person_id = claims.person_id();
    let films: Vec<Film> = state
        .db
        .query("SELECT * FROM film WHERE <-filmmaker_of<-person CONTAINS $person_id ORDER BY created_at DESC")
        .bind(("person_id", person_id))
        .await?
        .take(0)?;

    let film_views: Vec<FilmView> = films.into_iter().map(FilmView::from).collect();
    render_or_error(&FilmsIndexTemplate { films: film_views })
}

pub async fn new_form() -> Result<Response, AppError> {
    render_or_error(&FilmNewTemplate { error: None })
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Form(form): Form<FilmForm>,
) -> Result<Response, AppError> {
    // Validate content declaration
    if form.declare_copyright.is_none()
        || form.declare_talent.is_none()
        || form.declare_no_prohibited.is_none()
    {
        return render_or_error(&FilmNewTemplate {
            error: Some("You must confirm all content declarations.".into()),
        });
    }

    if form.title.trim().is_empty() {
        return render_or_error(&FilmNewTemplate {
            error: Some("Title is required.".into()),
        });
    }

    let title = form.title.trim().to_string();
    let slug = crate::util::slugify(&title);

    // Check slug uniqueness
    let existing: Vec<Film> = state
        .db
        .query("SELECT * FROM film WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.clone()))
        .await?
        .take(0)?;

    if !existing.is_empty() {
        return render_or_error(&FilmNewTemplate {
            error: Some("A film with a similar title already exists.".into()),
        });
    }

    let year: Option<i64> = form
        .year
        .as_deref()
        .filter(|y| !y.is_empty())
        .and_then(|y| y.parse().ok());

    let genres: Vec<String> = form
        .genres
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|g| g.trim().to_string())
        .filter(|g| !g.is_empty())
        .collect();

    let declaration = ContentDeclaration {
        is_copyright_holder: Some(true),
        talent_cleared: Some(true),
        no_prohibited_content: Some(true),
        declared_at: Some(chrono::Utc::now()),
    };

    let film: Option<Film> = state
        .db
        .create("film")
        .content(CreateFilm {
            title,
            slug,
            synopsis: form.synopsis.filter(|s| !s.trim().is_empty()),
            year,
            duration_seconds: None,
            genres,
            language: form.language.filter(|l| !l.trim().is_empty()),
            country: form.country.filter(|c| !c.trim().is_empty()),
            status: "draft".to_string(),
            content_declaration: declaration,
        })
        .await?;

    let film = film.ok_or_else(|| AppError::Internal(anyhow::anyhow!("Failed to create film")))?;

    // Create the filmmaker_of relation
    let person_id = claims.person_id();
    state
        .db
        .query("RELATE $person_id->filmmaker_of->$film_id SET role = 'director'")
        .bind(("person_id", person_id))
        .bind(("film_id", film.id.clone()))
        .await?;

    let key = crate::util::record_id_key_string(&film.id.key);
    Ok(Redirect::to(&format!("/films/{key}")).into_response())
}

pub async fn show(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let film = get_film(&state, &id).await?;
    let is_owner = verify_ownership(&state, &claims, &film).await;
    render_or_error(&FilmDetailTemplate {
        film: film.into(),
        is_owner,
    })
}

pub async fn edit(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let film = get_film(&state, &id).await?;
    require_ownership(&state, &claims, &film).await?;
    let html = FilmEditTemplate { film: film.into() }
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Template error: {e}")))?;
    Ok(sse::fragment("#film-detail", html).into_response())
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
    Form(form): Form<FilmUpdateForm>,
) -> Result<Response, AppError> {
    let film = get_film(&state, &id).await?;
    require_ownership(&state, &claims, &film).await?;

    if form.title.trim().is_empty() {
        return Err(AppError::Validation("Title cannot be empty.".into()));
    }

    let year: Option<i64> = form
        .year
        .as_deref()
        .filter(|y| !y.is_empty())
        .and_then(|y| y.parse().ok());

    let genres: Vec<String> = form
        .genres
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|g| g.trim().to_string())
        .filter(|g| !g.is_empty())
        .collect();

    let updated: Option<Film> = state
        .db
        .query(
            "UPDATE $film_id SET \
                title = $title, \
                synopsis = $synopsis, \
                year = $year, \
                genres = $genres, \
                language = $language, \
                country = $country \
             RETURN AFTER",
        )
        .bind(("film_id", film.id.clone()))
        .bind(("title", form.title.trim().to_string()))
        .bind(("synopsis", form.synopsis.filter(|s| !s.trim().is_empty())))
        .bind(("year", year))
        .bind(("genres", genres))
        .bind(("language", form.language.filter(|l| !l.trim().is_empty())))
        .bind(("country", form.country.filter(|c| !c.trim().is_empty())))
        .await?
        .take(0)?;

    let film = updated.ok_or(AppError::NotFound)?;
    let html = FilmInfoTemplate {
        film: film.into(),
        is_owner: true,
    }
    .render()
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Template error: {e}")))?;
    Ok(sse::fragment("#film-detail", html).into_response())
}

pub async fn update_status(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
    Form(form): Form<StatusForm>,
) -> Result<Response, AppError> {
    let film = get_film(&state, &id).await?;
    require_ownership(&state, &claims, &film).await?;

    // Validate status transition
    let valid = matches!(
        (film.status.as_str(), form.status.as_str()),
        ("draft", "published") | ("published", "archived") | ("archived", "draft")
    );

    if !valid {
        return Err(AppError::Validation(format!(
            "Cannot transition from '{}' to '{}'.",
            film.status, form.status
        )));
    }

    state
        .db
        .query("UPDATE $film_id SET status = $status")
        .bind(("film_id", film.id.clone()))
        .bind(("status", form.status))
        .await?;

    Ok(Redirect::to(&format!("/films/{}", id)).into_response())
}

pub async fn archive(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let film = get_film(&state, &id).await?;
    require_ownership(&state, &claims, &film).await?;

    state
        .db
        .query("UPDATE $film_id SET status = 'archived'")
        .bind(("film_id", film.id.clone()))
        .await?;

    Ok(Redirect::to("/films").into_response())
}

// ── Helpers ────────────────────────────────────────────────

pub async fn get_film_public(state: &AppState, id: &str) -> Result<Film, AppError> {
    let film_id = surrealdb::types::RecordId::new("film", id);
    let film: Option<Film> = state.db.select(film_id).await?;
    film.ok_or(AppError::NotFound)
}

async fn get_film(state: &AppState, id: &str) -> Result<Film, AppError> {
    get_film_public(state, id).await
}

async fn verify_ownership(state: &AppState, claims: &Claims, film: &Film) -> bool {
    let person_id = claims.person_id();
    let result: Vec<serde_json::Value> = state
        .db
        .query("SELECT id FROM filmmaker_of WHERE in = $person_id AND out = $film_id LIMIT 1")
        .bind(("person_id", person_id))
        .bind(("film_id", film.id.clone()))
        .await
        .map(|mut r| r.take(0).unwrap_or_default())
        .unwrap_or_default();
    !result.is_empty()
}

pub async fn require_film_ownership(
    state: &AppState,
    claims: &Claims,
    film: &Film,
) -> Result<(), AppError> {
    if verify_ownership(state, claims, film).await {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

async fn require_ownership(state: &AppState, claims: &Claims, film: &Film) -> Result<(), AppError> {
    require_film_ownership(state, claims, film).await
}

/// Re-export slugify for use by other controllers (e.g., platforms).
pub fn slugify_public(title: &str) -> String {
    crate::util::slugify(title)
}
