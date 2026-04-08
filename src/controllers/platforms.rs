use std::sync::Arc;

use askama::Template;
use axum::Form;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use surrealdb::types::RecordId;

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::models::film::{Film, FilmView};
use crate::models::platform::{CreatePlatform, Platform, PlatformTheme, PlatformView};
use crate::router::AppState;
use crate::sse;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/platforms_index.html")]
struct PlatformsIndexTemplate {
    platforms: Vec<PlatformView>,
}

#[derive(Template)]
#[template(path = "pages/platform_new.html")]
struct PlatformNewTemplate {
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/platform_dashboard.html")]
struct PlatformDashboardTemplate {
    platform: PlatformView,
    films: Vec<FilmView>,
}

#[derive(Template)]
#[template(path = "partials/platform_edit.html")]
struct PlatformEditTemplate {
    platform: PlatformView,
}

#[derive(Template)]
#[template(path = "partials/platform_info.html")]
struct PlatformInfoTemplate {
    platform: PlatformView,
}

// ── Form data ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PlatformForm {
    pub name: String,
    pub description: Option<String>,
    pub monetization_model: Option<String>,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub accent_color: Option<String>,
    pub font_heading: Option<String>,
    pub font_body: Option<String>,
    pub border_radius: Option<String>,
    pub dark_mode: Option<String>,
}

#[derive(Deserialize)]
pub struct AddFilmForm {
    pub film_id: String,
    pub featured: Option<String>,
}

// ── Handlers ───────────────────────────────────────────────

pub async fn index(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    let person_id = claims.person_id();
    let platforms: Vec<Platform> = state
        .db
        .query("SELECT * FROM platform WHERE <-curator_of<-person CONTAINS $person_id ORDER BY created_at DESC")
        .bind(("person_id", person_id))
        .await?
        .take(0)?;

    let views: Vec<PlatformView> = platforms.into_iter().map(PlatformView::from).collect();
    render_or_error(&PlatformsIndexTemplate { platforms: views })
}

pub async fn new_form() -> Result<Response, AppError> {
    render_or_error(&PlatformNewTemplate { error: None })
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Form(form): Form<PlatformForm>,
) -> Result<Response, AppError> {
    if form.name.trim().is_empty() {
        return render_or_error(&PlatformNewTemplate {
            error: Some("Name is required.".into()),
        });
    }

    let slug = crate::controllers::films::slugify_public(&form.name);

    // Check slug uniqueness
    let existing: Vec<Platform> = state
        .db
        .query("SELECT * FROM platform WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.clone()))
        .await?
        .take(0)?;

    if !existing.is_empty() {
        return render_or_error(&PlatformNewTemplate {
            error: Some("A platform with this name already exists.".into()),
        });
    }

    let theme = form_to_theme(&form);

    let platform: Option<Platform> = state
        .db
        .create("platform")
        .content(CreatePlatform {
            name: form.name.trim().to_string(),
            slug,
            description: form.description.filter(|s| !s.trim().is_empty()),
            monetization_model: form.monetization_model.filter(|s| !s.trim().is_empty()),
            status: "setup".to_string(),
            theme,
        })
        .await?;

    let platform =
        platform.ok_or_else(|| AppError::Internal(anyhow::anyhow!("Failed to create platform")))?;

    // Create curator_of relation
    state
        .db
        .query("RELATE $person_id->curator_of->$platform_id SET role = 'owner'")
        .bind(("person_id", claims.person_id()))
        .bind(("platform_id", platform.id.clone()))
        .await?;

    let key = crate::util::record_id_key_string(&platform.id.key);
    Ok(Redirect::to(&format!("/platforms/{key}")).into_response())
}

pub async fn dashboard(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let platform = get_platform(&state, &id).await?;
    require_curator(&state, &claims, &platform).await?;

    // Get carried films
    let platform_id = RecordId::new("platform", id.as_str());
    let films: Vec<Film> = state
        .db
        .query("SELECT * FROM film WHERE <-carries<-platform CONTAINS $platform_id ORDER BY created_at DESC")
        .bind(("platform_id", platform_id))
        .await?
        .take(0)?;

    let film_views: Vec<FilmView> = films.into_iter().map(FilmView::from).collect();

    render_or_error(&PlatformDashboardTemplate {
        platform: platform.into(),
        films: film_views,
    })
}

pub async fn edit(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let platform = get_platform(&state, &id).await?;
    require_curator(&state, &claims, &platform).await?;

    let html = PlatformEditTemplate {
        platform: platform.into(),
    }
    .render()
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Template error: {e}")))?;
    Ok(sse::fragment("#platform-detail", html).into_response())
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
    Form(form): Form<PlatformForm>,
) -> Result<Response, AppError> {
    let platform = get_platform(&state, &id).await?;
    require_curator(&state, &claims, &platform).await?;

    if form.name.trim().is_empty() {
        return Err(AppError::Validation("Name cannot be empty.".into()));
    }

    let theme = form_to_theme(&form);
    let platform_id = RecordId::new("platform", id.as_str());

    let updated: Option<Platform> = state
        .db
        .query(
            "UPDATE $pid SET \
                name = $name, \
                description = $description, \
                monetization_model = $monetization, \
                theme = $theme \
             RETURN AFTER",
        )
        .bind(("pid", platform_id))
        .bind(("name", form.name.trim().to_string()))
        .bind((
            "description",
            form.description.filter(|s| !s.trim().is_empty()),
        ))
        .bind((
            "monetization",
            form.monetization_model.filter(|s| !s.trim().is_empty()),
        ))
        .bind(("theme", theme))
        .await?
        .take(0)?;

    let platform = updated.ok_or(AppError::NotFound)?;
    let html = PlatformInfoTemplate {
        platform: platform.into(),
    }
    .render()
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Template error: {e}")))?;
    Ok(sse::fragment("#platform-detail", html).into_response())
}

pub async fn activate(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let platform = get_platform(&state, &id).await?;
    require_curator(&state, &claims, &platform).await?;

    let pid = RecordId::new("platform", id.as_str());
    state
        .db
        .query("UPDATE $pid SET status = 'active'")
        .bind(("pid", pid))
        .await?;

    Ok(Redirect::to(&format!("/platforms/{id}")).into_response())
}

/// Add a film to the platform's carried content.
pub async fn add_film(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
    Form(form): Form<AddFilmForm>,
) -> Result<Response, AppError> {
    let platform = get_platform(&state, &id).await?;
    require_curator(&state, &claims, &platform).await?;

    let platform_id = RecordId::new("platform", id.as_str());
    let film_id = RecordId::new("film", form.film_id.as_str());
    let featured = form.featured.is_some();

    state
        .db
        .query("RELATE $platform_id->carries->$film_id SET featured = $featured")
        .bind(("platform_id", platform_id))
        .bind(("film_id", film_id))
        .bind(("featured", featured))
        .await?;

    Ok(Redirect::to(&format!("/platforms/{id}")).into_response())
}

/// Remove a film from the platform.
pub async fn remove_film(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((id, film_id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let platform = get_platform(&state, &id).await?;
    require_curator(&state, &claims, &platform).await?;

    let platform_id = RecordId::new("platform", id.as_str());
    let film_record = RecordId::new("film", film_id.as_str());

    state
        .db
        .query("DELETE FROM carries WHERE in = $platform_id AND out = $film_id")
        .bind(("platform_id", platform_id))
        .bind(("film_id", film_record))
        .await?;

    Ok(Redirect::to(&format!("/platforms/{id}")).into_response())
}

// ── Public platform rendering ──────────────────────────────

#[derive(Template)]
#[template(path = "pages/public_platform.html")]
struct PublicPlatformTemplate {
    platform: PlatformView,
    films: Vec<FilmView>,
    theme_css: String,
}

#[derive(Template)]
#[template(path = "pages/public_platform_film.html")]
struct PublicPlatformFilmTemplate {
    platform: PlatformView,
    film: FilmView,
    theme_css: String,
}

pub async fn public_home(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Response, AppError> {
    let platform = get_platform_by_slug(&state, &slug).await?;
    if platform.status != "active" {
        return Err(AppError::NotFound);
    }

    let platform_id = platform.id.clone();
    let films: Vec<Film> = state
        .db
        .query("SELECT * FROM film WHERE <-carries<-platform CONTAINS $platform_id AND status = 'published' ORDER BY created_at DESC")
        .bind(("platform_id", platform_id))
        .await?
        .take(0)?;

    let theme_css = platform
        .theme
        .as_ref()
        .map(|t| t.to_css_overrides())
        .unwrap_or_default();

    render_or_error(&PublicPlatformTemplate {
        platform: platform.into(),
        films: films.into_iter().map(FilmView::from).collect(),
        theme_css,
    })
}

pub async fn public_film(
    State(state): State<Arc<AppState>>,
    Path((slug, film_slug)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let platform = get_platform_by_slug(&state, &slug).await?;
    if platform.status != "active" {
        return Err(AppError::NotFound);
    }

    // Find film by slug
    let films: Vec<Film> = state
        .db
        .query("SELECT * FROM film WHERE slug = $film_slug AND status = 'published' LIMIT 1")
        .bind(("film_slug", film_slug))
        .await?
        .take(0)?;
    let film = films.into_iter().next().ok_or(AppError::NotFound)?;

    // Verify platform carries this film
    let carried: Vec<serde_json::Value> = state
        .db
        .query("SELECT id FROM carries WHERE in = $pid AND out = $fid LIMIT 1")
        .bind(("pid", platform.id.clone()))
        .bind(("fid", film.id.clone()))
        .await?
        .take(0)?;

    if carried.is_empty() {
        return Err(AppError::NotFound);
    }

    let theme_css = platform
        .theme
        .as_ref()
        .map(|t| t.to_css_overrides())
        .unwrap_or_default();

    render_or_error(&PublicPlatformFilmTemplate {
        platform: platform.into(),
        film: film.into(),
        theme_css,
    })
}

// ── Helpers ────────────────────────────────────────────────

pub async fn get_platform(state: &AppState, id: &str) -> Result<Platform, AppError> {
    let pid = RecordId::new("platform", id);
    let platform: Option<Platform> = state.db.select(pid).await?;
    platform.ok_or(AppError::NotFound)
}

async fn get_platform_by_slug(state: &AppState, slug: &str) -> Result<Platform, AppError> {
    let platforms: Vec<Platform> = state
        .db
        .query("SELECT * FROM platform WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.to_string()))
        .await?
        .take(0)?;
    platforms.into_iter().next().ok_or(AppError::NotFound)
}

pub async fn require_curator_public(
    state: &AppState,
    claims: &Claims,
    platform: &Platform,
) -> Result<(), AppError> {
    require_curator(state, claims, platform).await
}

async fn require_curator(
    state: &AppState,
    claims: &Claims,
    platform: &Platform,
) -> Result<(), AppError> {
    let person_id = claims.person_id();
    let result: Vec<serde_json::Value> = state
        .db
        .query("SELECT id FROM curator_of WHERE in = $person_id AND out = $platform_id LIMIT 1")
        .bind(("person_id", person_id))
        .bind(("platform_id", platform.id.clone()))
        .await
        .map(|mut r| r.take(0).unwrap_or_default())
        .unwrap_or_default();

    if result.is_empty() {
        Err(AppError::Forbidden)
    } else {
        Ok(())
    }
}

fn form_to_theme(form: &PlatformForm) -> PlatformTheme {
    PlatformTheme {
        primary_color: form.primary_color.clone().filter(|s| !s.is_empty()),
        secondary_color: form.secondary_color.clone().filter(|s| !s.is_empty()),
        accent_color: form.accent_color.clone().filter(|s| !s.is_empty()),
        font_heading: form.font_heading.clone().filter(|s| !s.is_empty()),
        font_body: form.font_body.clone().filter(|s| !s.is_empty()),
        border_radius: form.border_radius.clone().filter(|s| !s.is_empty()),
        dark_mode: Some(form.dark_mode.is_some()),
    }
}
