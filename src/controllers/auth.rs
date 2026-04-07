use std::sync::Arc;

use askama::Template;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;

use crate::auth::claims::issue_token;
use crate::auth::middleware::OptionalClaims;
use crate::auth::password;
use crate::error::AppError;
use crate::models::person::{CreatePerson, GdprConsent, Person};
use crate::router::AppState;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/register.html")]
struct RegisterTemplate {
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/login.html")]
struct LoginTemplate {
    error: Option<String>,
}

// ── Form data ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterForm {
    pub email: String,
    pub name: String,
    pub password: String,
    pub password_confirm: String,
    // Terms acceptance checkboxes
    pub accept_terms: Option<String>,
    pub accept_no_porn: Option<String>,
    pub accept_copyright: Option<String>,
    pub accept_talent: Option<String>,
    // GDPR consent
    pub consent_marketing: Option<String>,
    pub consent_analytics: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
}

// ── Handlers ───────────────────────────────────────────────

pub async fn register_page(
    OptionalClaims(claims): OptionalClaims,
) -> Result<Response, AppError> {
    if claims.is_some() {
        return Ok(Redirect::to("/profile").into_response());
    }
    render_or_error(&RegisterTemplate { error: None })
}

pub async fn register_submit(
    State(state): State<Arc<AppState>>,
    Form(form): Form<RegisterForm>,
) -> Result<Response, AppError> {
    // Validate terms acceptance — all four required
    if form.accept_terms.is_none()
        || form.accept_no_porn.is_none()
        || form.accept_copyright.is_none()
        || form.accept_talent.is_none()
    {
        return render_or_error(&RegisterTemplate {
            error: Some("You must accept all terms and content policy checkboxes to register.".into()),
        });
    }

    // Validate fields
    if form.name.trim().is_empty() {
        return render_or_error(&RegisterTemplate {
            error: Some("Name is required.".into()),
        });
    }
    if form.password.len() < 8 {
        return render_or_error(&RegisterTemplate {
            error: Some("Password must be at least 8 characters.".into()),
        });
    }
    if form.password != form.password_confirm {
        return render_or_error(&RegisterTemplate {
            error: Some("Passwords do not match.".into()),
        });
    }

    // Check if email already exists
    let email = form.email.trim().to_lowercase();
    let existing: Vec<Person> = state
        .db
        .query("SELECT * FROM person WHERE email = $email LIMIT 1")
        .bind(("email", email.clone()))
        .await?
        .take(0)?;

    if !existing.is_empty() {
        return render_or_error(&RegisterTemplate {
            error: Some("An account with this email already exists.".into()),
        });
    }

    // Hash password
    let hash = password::hash_password(&form.password)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Password hashing failed: {e}")))?;

    let consent = GdprConsent {
        marketing: Some(form.consent_marketing.is_some()),
        analytics: Some(form.consent_analytics.is_some()),
        updated_at: Some(chrono::Utc::now()),
    };

    // Create person
    let person: Option<Person> = state
        .db
        .create("person")
        .content(CreatePerson {
            email,
            name: form.name.trim().to_string(),
            password_hash: hash,
            roles: vec!["filmmaker".to_string()],
            gdpr_consent: consent,
        })
        .await?;

    let person = person.ok_or_else(|| AppError::Internal(anyhow::anyhow!("Failed to create person")))?;

    // Issue JWT
    let key = crate::util::record_id_key_string(&person.id.key);
    let token = issue_token(&key, &person.name, &person.roles, &state.config.jwt_secret)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT error: {e}")))?;

    Ok(set_token_cookie_and_redirect(&token, "/profile"))
}

pub async fn login_page(
    OptionalClaims(claims): OptionalClaims,
) -> Result<Response, AppError> {
    if claims.is_some() {
        return Ok(Redirect::to("/profile").into_response());
    }
    render_or_error(&LoginTemplate { error: None })
}

pub async fn login_submit(
    State(state): State<Arc<AppState>>,
    Form(form): Form<LoginForm>,
) -> Result<Response, AppError> {
    let email = form.email.trim().to_lowercase();
    let persons: Vec<Person> = state
        .db
        .query("SELECT * FROM person WHERE email = $email LIMIT 1")
        .bind(("email", email))
        .await?
        .take(0)?;

    let person = match persons.into_iter().next() {
        Some(p) => p,
        None => {
            return render_or_error(&LoginTemplate {
                error: Some("Invalid email or password.".into()),
            });
        }
    };

    let valid = password::verify_password(&form.password, &person.password_hash)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Password verify error: {e}")))?;

    if !valid {
        return render_or_error(&LoginTemplate {
            error: Some("Invalid email or password.".into()),
        });
    }

    let key = crate::util::record_id_key_string(&person.id.key);
    let token = issue_token(&key, &person.name, &person.roles, &state.config.jwt_secret)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT error: {e}")))?;

    Ok(set_token_cookie_and_redirect(&token, "/profile"))
}

pub async fn logout() -> impl IntoResponse {
    (
        StatusCode::SEE_OTHER,
        [
            (SET_COOKIE, "pavilion_token=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax".to_string()),
            (axum::http::header::LOCATION, "/login".to_string()),
        ],
    )
}

// ── SlateHub OAuth stubs ───────────────────────────────────

pub async fn slatehub_oauth_start() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "SlateHub OAuth not yet available")
}

pub async fn slatehub_oauth_callback() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "SlateHub OAuth not yet available")
}

// ── Helpers ────────────────────────────────────────────────

fn set_token_cookie_and_redirect(token: &str, location: &str) -> Response {
    let cookie = format!(
        "pavilion_token={token}; Path=/; Max-Age=86400; HttpOnly; SameSite=Lax"
    );
    (
        StatusCode::SEE_OTHER,
        [
            (SET_COOKIE, cookie),
            (axum::http::header::LOCATION, location.to_string()),
        ],
    )
        .into_response()
}
