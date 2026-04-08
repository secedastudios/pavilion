use std::sync::Arc;

use askama::Template;
use axum::Form;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use surrealdb::types::RecordId;

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::models::license::{CreateLicense, License, LicenseView, validate_license};
use crate::router::AppState;
use crate::sse;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/licenses_index.html")]
struct LicensesIndexTemplate {
    licenses: Vec<LicenseView>,
    film_key_str: String,
    film_title: String,
}

#[derive(Template)]
#[template(path = "pages/license_new.html")]
struct LicenseNewTemplate {
    film_key_str: String,
    film_title: String,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "partials/license_edit.html")]
struct LicenseEditTemplate {
    license: LicenseView,
    film_key_str: String,
}

#[derive(Template)]
#[template(path = "partials/license_detail.html")]
struct LicenseDetailTemplate {
    license: LicenseView,
    film_key_str: String,
}

// ── Form data ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LicenseForm {
    pub license_type: String,
    pub territories: Option<String>,
    pub window_start: Option<String>,
    pub window_end: Option<String>,
    pub approval_required: Option<String>,
    // TVOD
    pub rental_price: Option<String>,
    pub rental_duration_hours: Option<String>,
    pub purchase_price: Option<String>,
    // SVOD / AVOD
    pub flat_fee_monthly: Option<String>,
    pub revenue_share_pct: Option<String>,
    // Event
    pub event_flat_fee: Option<String>,
    pub ticket_split_pct: Option<String>,
    pub max_attendees: Option<String>,
    // Educational
    pub institution_types: Option<String>,
    pub pricing_tier: Option<String>,
    // Creative Commons
    pub cc_license_type: Option<String>,
}

// ── Handlers ───────────────────────────────────────────────

pub async fn index(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let record_id = RecordId::new("film", film_id.as_str());
    let licenses = crate::licensing::rights::licenses_for_film(&state.db, &record_id).await?;
    let views: Vec<LicenseView> = licenses.into_iter().map(LicenseView::from).collect();

    render_or_error(&LicensesIndexTemplate {
        licenses: views,
        film_key_str: film_id,
        film_title: film.title,
    })
}

pub async fn new_form(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    render_or_error(&LicenseNewTemplate {
        film_key_str: film_id,
        film_title: film.title,
        error: None,
    })
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
    Form(form): Form<LicenseForm>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let create_license = form_to_create_license(&form);

    if let Err(msg) = validate_license(&create_license) {
        return render_or_error(&LicenseNewTemplate {
            film_key_str: film_id,
            film_title: film.title,
            error: Some(msg),
        });
    }

    let license: Option<License> = state.db.create("license").content(create_license).await?;

    let license =
        license.ok_or_else(|| AppError::Internal(anyhow::anyhow!("Failed to create license")))?;

    // Create the licensed_via relation
    let film_record = RecordId::new("film", film_id.as_str());
    state
        .db
        .query("RELATE $film_id->licensed_via->$license_id")
        .bind(("film_id", film_record))
        .bind(("license_id", license.id))
        .await?;

    Ok(Redirect::to(&format!("/films/{film_id}/licenses")).into_response())
}

pub async fn edit(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((film_id, license_id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let license = get_license(&state, &license_id).await?;
    let html = LicenseEditTemplate {
        license: license.into(),
        film_key_str: film_id,
    }
    .render()
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Template error: {e}")))?;

    Ok(sse::fragment(format!("#license-{license_id}"), html).into_response())
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((film_id, license_id)): Path<(String, String)>,
    Form(form): Form<LicenseForm>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let create = form_to_create_license(&form);
    if let Err(msg) = validate_license(&create) {
        return Err(AppError::Validation(msg));
    }

    let lid = RecordId::new("license", license_id.as_str());
    let updated: Option<License> = state
        .db
        .query(
            "UPDATE $lid SET \
                license_type = $license_type, \
                territories = $territories, \
                window_start = $window_start, \
                window_end = $window_end, \
                approval_required = $approval_required, \
                rental_price_cents = $rental_price_cents, \
                rental_duration_hours = $rental_duration_hours, \
                purchase_price_cents = $purchase_price_cents, \
                flat_fee_monthly_cents = $flat_fee_monthly_cents, \
                revenue_share_pct = $revenue_share_pct, \
                event_flat_fee_cents = $event_flat_fee_cents, \
                ticket_split_pct = $ticket_split_pct, \
                max_attendees = $max_attendees, \
                institution_types = $institution_types, \
                pricing_tier = $pricing_tier, \
                cc_license_type = $cc_license_type \
             RETURN AFTER",
        )
        .bind(("lid", lid))
        .bind(("license_type", create.license_type))
        .bind(("territories", create.territories))
        .bind(("window_start", create.window_start))
        .bind(("window_end", create.window_end))
        .bind(("approval_required", create.approval_required))
        .bind(("rental_price_cents", create.rental_price_cents))
        .bind(("rental_duration_hours", create.rental_duration_hours))
        .bind(("purchase_price_cents", create.purchase_price_cents))
        .bind(("flat_fee_monthly_cents", create.flat_fee_monthly_cents))
        .bind(("revenue_share_pct", create.revenue_share_pct))
        .bind(("event_flat_fee_cents", create.event_flat_fee_cents))
        .bind(("ticket_split_pct", create.ticket_split_pct))
        .bind(("max_attendees", create.max_attendees))
        .bind(("institution_types", create.institution_types))
        .bind(("pricing_tier", create.pricing_tier))
        .bind(("cc_license_type", create.cc_license_type))
        .await?
        .take(0)?;

    let license = updated.ok_or(AppError::NotFound)?;
    let html = LicenseDetailTemplate {
        license: license.into(),
        film_key_str: film_id,
    }
    .render()
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Template error: {e}")))?;

    Ok(sse::fragment(format!("#license-{license_id}"), html).into_response())
}

pub async fn deactivate(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((film_id, license_id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let lid = RecordId::new("license", license_id.as_str());
    state
        .db
        .query("UPDATE $lid SET active = false")
        .bind(("lid", lid))
        .await?;

    Ok(Redirect::to(&format!("/films/{film_id}/licenses")).into_response())
}

// ── Helpers ────────────────────────────────────────────────

async fn get_license(state: &AppState, id: &str) -> Result<License, AppError> {
    let lid = RecordId::new("license", id);
    let license: Option<License> = state.db.select(lid).await?;
    license.ok_or(AppError::NotFound)
}

fn parse_cents(s: &Option<String>) -> Option<i64> {
    s.as_deref()
        .filter(|v| !v.is_empty())
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| (v * 100.0).round() as i64)
}

fn parse_f64(s: &Option<String>) -> Option<f64> {
    s.as_deref()
        .filter(|v| !v.is_empty())
        .and_then(|v| v.parse().ok())
}

fn parse_i64(s: &Option<String>) -> Option<i64> {
    s.as_deref()
        .filter(|v| !v.is_empty())
        .and_then(|v| v.parse().ok())
}

fn parse_datetime(s: &Option<String>) -> Option<chrono::DateTime<chrono::Utc>> {
    s.as_deref()
        .filter(|v| !v.is_empty())
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

fn form_to_create_license(form: &LicenseForm) -> CreateLicense {
    let territories: Vec<String> = form
        .territories
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|t| t.trim().to_uppercase())
        .filter(|t| !t.is_empty())
        .collect();

    let institution_types: Option<Vec<String>> = form
        .institution_types
        .as_deref()
        .filter(|v| !v.is_empty())
        .map(|v| {
            v.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        });

    CreateLicense {
        license_type: form.license_type.clone(),
        territories,
        window_start: parse_datetime(&form.window_start),
        window_end: parse_datetime(&form.window_end),
        approval_required: form.approval_required.is_some(),
        active: true,
        rental_price_cents: parse_cents(&form.rental_price),
        rental_duration_hours: parse_i64(&form.rental_duration_hours),
        purchase_price_cents: parse_cents(&form.purchase_price),
        flat_fee_monthly_cents: parse_cents(&form.flat_fee_monthly),
        revenue_share_pct: parse_f64(&form.revenue_share_pct),
        event_flat_fee_cents: parse_cents(&form.event_flat_fee),
        ticket_split_pct: parse_f64(&form.ticket_split_pct),
        max_attendees: parse_i64(&form.max_attendees),
        institution_types,
        pricing_tier: form.pricing_tier.clone().filter(|v| !v.is_empty()),
        cc_license_type: form.cc_license_type.clone().filter(|v| !v.is_empty()),
    }
}
