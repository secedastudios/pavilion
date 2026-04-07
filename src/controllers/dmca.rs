use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use surrealdb::types::RecordId;

use crate::auth::claims::Claims;
use crate::auth::middleware::OptionalClaims;
use crate::error::AppError;
use crate::models::dmca::{CreateDmcaClaim, DmcaClaim, DmcaClaimView};
use crate::router::AppState;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/dmca_form.html")]
struct DmcaFormTemplate {
    error: Option<String>,
    success: bool,
}

#[derive(Template)]
#[template(path = "pages/dmca_agent.html")]
struct DmcaAgentTemplate;

#[derive(Template)]
#[template(path = "pages/dmca_claims.html")]
struct DmcaClaimsTemplate {
    claims: Vec<DmcaClaimView>,
    film_key_str: String,
    film_title: String,
}

// ── Form data ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DmcaForm {
    pub claimant_name: String,
    pub claimant_email: String,
    pub claimant_company: Option<String>,
    pub film_id: String,
    pub description: String,
    pub evidence_url: Option<String>,
    pub good_faith: Option<String>,
    pub perjury: Option<String>,
}

#[derive(Deserialize)]
pub struct CounterForm {
    pub reason: String,
}

// ── Public handlers (no login required) ────────────────────

pub async fn dmca_form(
    OptionalClaims(_claims): OptionalClaims,
) -> Result<Response, AppError> {
    render_or_error(&DmcaFormTemplate {
        error: None,
        success: false,
    })
}

pub async fn submit_claim(
    State(state): State<Arc<AppState>>,
    Form(form): Form<DmcaForm>,
) -> Result<Response, AppError> {
    // Validate required fields
    if form.claimant_name.trim().is_empty() || form.claimant_email.trim().is_empty() {
        return render_or_error(&DmcaFormTemplate {
            error: Some("Name and email are required.".into()),
            success: false,
        });
    }

    if form.description.trim().is_empty() {
        return render_or_error(&DmcaFormTemplate {
            error: Some("Description of the copyrighted work is required.".into()),
            success: false,
        });
    }

    if form.good_faith.is_none() || form.perjury.is_none() {
        return render_or_error(&DmcaFormTemplate {
            error: Some("You must confirm the good faith statement and perjury declaration.".into()),
            success: false,
        });
    }

    let film_id = RecordId::new("film", form.film_id.as_str());

    let _claim: Option<DmcaClaim> = state
        .db
        .create("dmca_claim")
        .content(CreateDmcaClaim {
            claimant_name: form.claimant_name.trim().to_string(),
            claimant_email: form.claimant_email.trim().to_lowercase(),
            claimant_company: form.claimant_company.filter(|s| !s.trim().is_empty()),
            film: film_id,
            description: form.description.trim().to_string(),
            evidence_url: form.evidence_url.filter(|s| !s.trim().is_empty()),
            good_faith_statement: true,
            perjury_declaration: true,
        })
        .await?;

    tracing::info!(film = %form.film_id, claimant = %form.claimant_email.trim(), "DMCA claim filed");

    render_or_error(&DmcaFormTemplate {
        error: None,
        success: true,
    })
}

pub async fn dmca_agent() -> Result<Response, AppError> {
    render_or_error(&DmcaAgentTemplate)
}

// ── Filmmaker: view claims against their films ─────────────

pub async fn film_claims(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let record_id = RecordId::new("film", film_id.as_str());
    let dmca_claims: Vec<DmcaClaim> = state
        .db
        .query("SELECT * FROM dmca_claim WHERE film = $film ORDER BY filed_at DESC")
        .bind(("film", record_id))
        .await?
        .take(0)?;

    let views: Vec<DmcaClaimView> = dmca_claims.into_iter().map(DmcaClaimView::from).collect();

    render_or_error(&DmcaClaimsTemplate {
        claims: views,
        film_key_str: film_id,
        film_title: film.title,
    })
}

/// Filmmaker submits a counter-notification.
pub async fn counter_claim(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((film_id, claim_id)): Path<(String, String)>,
    Form(form): Form<CounterForm>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let cid = RecordId::new("dmca_claim", claim_id.as_str());
    state
        .db
        .query("UPDATE $cid SET status = 'counter_filed', counter_reason = $reason WHERE status IN ['filed', 'under_review', 'upheld']")
        .bind(("cid", cid))
        .bind(("reason", form.reason.trim().to_string()))
        .await?;

    Ok(Redirect::to(&format!("/films/{film_id}/claims")).into_response())
}

// ── Admin: review and resolve claims ───────────────────────

pub async fn review_claim(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(claim_id): Path<String>,
    Form(form): Form<AdminReviewForm>,
) -> Result<Response, AppError> {
    if !claims.has_role("admin") {
        return Err(AppError::Forbidden);
    }

    let cid = RecordId::new("dmca_claim", claim_id.as_str());

    match form.action.as_str() {
        "uphold" => {
            state
                .db
                .query(
                    "UPDATE $cid SET status = 'upheld', admin_notes = $notes, \
                     reviewed_at = time::now()"
                )
                .bind(("cid", cid))
                .bind(("notes", form.notes))
                .await?;
        }
        "reject" => {
            state
                .db
                .query(
                    "UPDATE $cid SET status = 'rejected', admin_notes = $notes, \
                     reviewed_at = time::now(), resolved_at = time::now()"
                )
                .bind(("cid", cid))
                .bind(("notes", form.notes))
                .await?;
        }
        "resolve" => {
            state
                .db
                .query(
                    "UPDATE $cid SET status = 'resolved', admin_notes = $notes, \
                     resolved_at = time::now()"
                )
                .bind(("cid", cid))
                .bind(("notes", form.notes))
                .await?;
        }
        _ => return Err(AppError::Validation("Invalid action.".into())),
    }

    Ok(Redirect::to("/admin/dmca").into_response())
}

#[derive(Deserialize)]
pub struct AdminReviewForm {
    pub action: String,
    pub notes: Option<String>,
}
