use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use askama::Template;
use axum::extract::{Path, State};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use surrealdb::types::RecordId;

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::models::transcode::{TranscodeJobView, TranscodeProfile};
use crate::router::AppState;
use crate::transcode::queue;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/transcode_jobs.html")]
struct TranscodeJobsTemplate {
    jobs: Vec<TranscodeJobView>,
    film_key_str: String,
}

#[derive(Template)]
#[template(path = "partials/transcode_progress.html")]
struct TranscodeProgressTemplate {
    job: TranscodeJobView,
}

// ── Handlers ───────────────────────────────────────────────

/// Start a transcode job for a film. Requires film ownership.
pub async fn start_transcode(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let record_id = RecordId::new("film", film_id.as_str());
    let profile = TranscodeProfile::h264_default();

    queue::enqueue(&state.db, record_id, profile)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to enqueue: {e}")))?;

    Ok(Redirect::to(&format!("/films/{film_id}")).into_response())
}

/// Get the list of transcode jobs for a film (SSE fragment).
pub async fn jobs_list(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let record_id = RecordId::new("film", film_id.as_str());
    let jobs = queue::jobs_for_film(&state.db, &record_id).await?;
    let job_views: Vec<TranscodeJobView> = jobs.into_iter().map(TranscodeJobView::from).collect();

    let html = TranscodeJobsTemplate {
        jobs: job_views,
        film_key_str: film_id,
    }
    .render()
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Template error: {e}")))?;

    Ok(crate::sse::fragment("#transcode-jobs", html).into_response())
}

/// SSE endpoint for polling transcode job progress.
pub async fn job_progress(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((film_id, job_id)): Path<(String, String)>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let record_id = RecordId::new("transcode_job", job_id.as_str());
    let db = state.db.clone();

    let stream = async_stream::stream! {
        loop {
            match queue::get_job(&db, &record_id).await {
                Ok(Some(job)) => {
                    let view = TranscodeJobView::from(job.clone());
                    let html = TranscodeProgressTemplate { job: view }
                        .render()
                        .unwrap_or_else(|_| "Error".to_string());

                    let data = format!("selector #job-{}\nmerge morph\nfragment {}", job_id, html);
                    yield Ok(Event::default()
                        .event("datastar-merge-fragments")
                        .data(data));

                    // Stop streaming if terminal state
                    if job.status == "complete" || job.status == "failed" {
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    };

    Ok(Sse::new(stream))
}
