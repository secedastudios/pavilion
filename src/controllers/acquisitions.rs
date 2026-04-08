use std::sync::Arc;

use askama::Template;
use axum::Form;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use surrealdb::types::RecordId;

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::models::acquisition::{Acquisition, AcquisitionView, CreateAcquisition};
use crate::models::license::License;
use crate::router::AppState;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/acquisition_result.html")]
struct AcquisitionResultTemplate {
    status: String,
    film_key_str: String,
    film_title: String,
}

#[derive(Template)]
#[template(path = "pages/film_requests.html")]
struct FilmRequestsTemplate {
    requests: Vec<AcquisitionView>,
    film_key_str: String,
    film_title: String,
}

// ── Form data ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AcquireForm {
    pub license_id: String,
}

// ── Handlers: Curator-facing ───────────────────────────────

/// Curator requests to acquire a license for a film.
pub async fn acquire(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
    Form(form): Form<AcquireForm>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    if film.status != "published" {
        return Err(AppError::NotFound);
    }

    let license_record = RecordId::new("license", form.license_id.as_str());
    let license: Option<License> = state.db.select(license_record.clone()).await?;
    let license = license.ok_or(AppError::NotFound)?;

    if !license.active {
        return Err(AppError::Validation(
            "This license is no longer active.".into(),
        ));
    }

    // Check for existing pending/approved acquisition
    let existing: Vec<Acquisition> = state
        .db
        .query(
            "SELECT * FROM acquisition \
             WHERE film = $film_id AND license = $license_id AND requester = $requester \
               AND status IN ['pending', 'approved'] \
             LIMIT 1",
        )
        .bind(("film_id", RecordId::new("film", film_id.as_str())))
        .bind(("license_id", license_record.clone()))
        .bind(("requester", claims.person_id()))
        .await?
        .take(0)?;

    if !existing.is_empty() {
        return render_or_error(&AcquisitionResultTemplate {
            status: "already_requested".into(),
            film_key_str: film_id,
            film_title: film.title,
        });
    }

    // If no approval required, auto-approve
    let status = if license.approval_required {
        "pending"
    } else {
        "approved"
    };

    let _acquisition: Option<Acquisition> = state
        .db
        .create("acquisition")
        .content(CreateAcquisition {
            film: RecordId::new("film", film_id.as_str()),
            license: license_record,
            platform: None,
            requester: claims.person_id(),
            status: status.to_string(),
        })
        .await?;

    render_or_error(&AcquisitionResultTemplate {
        status: status.to_string(),
        film_key_str: film_id,
        film_title: film.title,
    })
}

// ── Handlers: Filmmaker-facing (approval workflow) ─────────

/// List pending acquisition requests for a film.
pub async fn film_requests(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let requests: Vec<Acquisition> = state
        .db
        .query(
            "SELECT * FROM acquisition \
             WHERE film = $film_id \
             ORDER BY requested_at DESC",
        )
        .bind(("film_id", RecordId::new("film", film_id.as_str())))
        .await?
        .take(0)?;

    let views: Vec<AcquisitionView> = requests.into_iter().map(AcquisitionView::from).collect();

    render_or_error(&FilmRequestsTemplate {
        requests: views,
        film_key_str: film_id,
        film_title: film.title,
    })
}

/// Approve a pending acquisition request.
pub async fn approve_request(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((film_id, request_id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let acq_id = RecordId::new("acquisition", request_id.as_str());
    state
        .db
        .query(
            "UPDATE $acq_id SET \
                status = 'approved', \
                resolved_at = time::now(), \
                resolved_by = $person_id \
             WHERE status = 'pending'",
        )
        .bind(("acq_id", acq_id))
        .bind(("person_id", claims.person_id()))
        .await?;

    Ok(Redirect::to(&format!("/films/{film_id}/requests")).into_response())
}

/// Reject a pending acquisition request.
pub async fn reject_request(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((film_id, request_id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let acq_id = RecordId::new("acquisition", request_id.as_str());
    state
        .db
        .query(
            "UPDATE $acq_id SET \
                status = 'rejected', \
                resolved_at = time::now(), \
                resolved_by = $person_id \
             WHERE status = 'pending'",
        )
        .bind(("acq_id", acq_id))
        .bind(("person_id", claims.person_id()))
        .await?;

    Ok(Redirect::to(&format!("/films/{film_id}/requests")).into_response())
}
