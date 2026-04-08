use std::sync::Arc;

use askama::Template;
use axum::Form;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::models::person::{Person, PersonView};
use crate::router::AppState;
use crate::sse;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/profile.html")]
struct ProfileTemplate {
    person: PersonView,
}

#[derive(Template)]
#[template(path = "partials/profile_edit.html")]
struct ProfileEditTemplate {
    person: PersonView,
}

#[derive(Template)]
#[template(path = "partials/profile_display.html")]
struct ProfileDisplayTemplate {
    person: PersonView,
}

// ── Form data ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ProfileUpdateForm {
    pub name: String,
    pub bio: Option<String>,
}

// ── Handlers ───────────────────────────────────────────────

pub async fn show(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    let person = get_person(&state, &claims).await?;
    render_or_error(&ProfileTemplate {
        person: person.into(),
    })
}

pub async fn edit(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    let person = get_person(&state, &claims).await?;
    let html = ProfileEditTemplate {
        person: person.into(),
    }
    .render()
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Template error: {e}")))?;
    Ok(sse::fragment("#profile-detail", html).into_response())
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Form(form): Form<ProfileUpdateForm>,
) -> Result<Response, AppError> {
    if form.name.trim().is_empty() {
        return Err(AppError::Validation("Name cannot be empty.".into()));
    }

    let person_id = claims.person_id();
    let bio = form.bio.filter(|b| !b.trim().is_empty());

    let updated: Option<Person> = state
        .db
        .query("UPDATE $person_id SET name = $name, bio = $bio RETURN AFTER")
        .bind(("person_id", person_id))
        .bind(("name", form.name.trim().to_string()))
        .bind(("bio", bio))
        .await?
        .take(0)?;

    let person = updated.ok_or(AppError::NotFound)?;
    let html = ProfileDisplayTemplate {
        person: person.into(),
    }
    .render()
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Template error: {e}")))?;
    Ok(sse::fragment("#profile-detail", html).into_response())
}

// ── Helpers ────────────────────────────────────────────────

async fn get_person(state: &AppState, claims: &Claims) -> Result<Person, AppError> {
    let person_id = claims.person_id();
    let person: Option<Person> = state.db.select(person_id).await?;
    person.ok_or(AppError::NotFound)
}
