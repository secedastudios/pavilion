use std::sync::Arc;

use askama::Template;
use axum::Form;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::models::person::Person;
use crate::router::AppState;
use crate::sse;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/settings_privacy.html")]
struct PrivacySettingsTemplate {
    marketing: bool,
    analytics: bool,
}

// ── Form data ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ConsentForm {
    pub consent_marketing: Option<String>,
    pub consent_analytics: Option<String>,
}

// ── Handlers ───────────────────────────────────────────────

pub async fn privacy_settings(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    let person: Option<Person> = state.db.select(claims.person_id()).await?;
    let person = person.ok_or(AppError::NotFound)?;

    let consent = person.gdpr_consent.unwrap_or_default();
    render_or_error(&PrivacySettingsTemplate {
        marketing: consent.marketing.unwrap_or(false),
        analytics: consent.analytics.unwrap_or(false),
    })
}

pub async fn update_consent(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Form(form): Form<ConsentForm>,
) -> Result<Response, AppError> {
    let marketing = form.consent_marketing.is_some();
    let analytics = form.consent_analytics.is_some();

    state
        .db
        .query(
            "UPDATE $person_id SET \
                gdpr_consent.marketing = $marketing, \
                gdpr_consent.analytics = $analytics, \
                gdpr_consent.updated_at = time::now()",
        )
        .bind(("person_id", claims.person_id()))
        .bind(("marketing", marketing))
        .bind(("analytics", analytics))
        .await?;

    let html = r#"<p class="settings-saved">Consent preferences updated.</p>"#.to_string();
    Ok(sse::fragment("#consent-result", html).into_response())
}

pub async fn data_export(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    let person: Option<Person> = state.db.select(claims.person_id()).await?;
    let person = person.ok_or(AppError::NotFound)?;

    // Build export — future phases will add watch history, transactions, etc.
    let export = serde_json::json!({
        "person": {
            "email": person.email,
            "name": person.name,
            "roles": person.roles,
            "bio": person.bio,
            "avatar_url": person.avatar_url,
            "gdpr_consent": person.gdpr_consent,
            "created_at": person.created_at,
        },
        "exported_at": chrono::Utc::now(),
    });

    Ok((
        StatusCode::OK,
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/json".to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"pavilion-data-export.json\"".to_string(),
            ),
        ],
        serde_json::to_string_pretty(&export)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("JSON error: {e}")))?,
    )
        .into_response())
}

pub async fn delete_account(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    let person_id = claims.person_id();

    // Full GDPR-compliant cascade: delete all personal data across every table.
    // Must match the admin GDPR delete in admin.rs.
    let person_id_str = format!("{:?}", person_id);
    state
        .db
        .query(
            "DELETE FROM agreed_to WHERE in = $pid; \
             DELETE FROM filmmaker_of WHERE in = $pid; \
             DELETE FROM curator_of WHERE in = $pid; \
             DELETE FROM attending WHERE in = $pid; \
             DELETE FROM watch_session WHERE person = $pid; \
             DELETE FROM entitlement WHERE person = $pid; \
             DELETE FROM viewer_subscription WHERE person = $pid; \
             DELETE FROM rating WHERE person = $pid; \
             DELETE FROM credit_balance WHERE person = $pid; \
             DELETE FROM credit_transaction WHERE person = $pid; \
             DELETE FROM storage_usage WHERE person = $pid; \
             DELETE $pid;",
        )
        .bind(("pid", person_id))
        .await?;

    tracing::info!(person = %person_id_str, "Account deleted (GDPR right to erasure)");

    // Clear cookie and redirect to home
    Ok((
        StatusCode::SEE_OTHER,
        [
            (
                axum::http::header::SET_COOKIE,
                crate::controllers::auth::clear_token_cookie(),
            ),
            (axum::http::header::LOCATION, "/".to_string()),
        ],
    )
        .into_response())
}

// Default impl for GdprConsent so we can unwrap_or_default
impl Default for crate::models::person::GdprConsent {
    fn default() -> Self {
        Self {
            marketing: Some(false),
            analytics: Some(false),
            updated_at: None,
        }
    }
}
