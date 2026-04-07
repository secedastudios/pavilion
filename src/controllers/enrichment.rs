use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use surrealdb::types::RecordId;

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::media::enrichment::{EnrichmentData, TmdbClient, TmdbSearchResult};
use crate::media::images;
use crate::router::AppState;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/enrich_search.html")]
struct EnrichSearchTemplate {
    film_key_str: String,
    film_title: String,
    results: Vec<TmdbSearchResult>,
    query: String,
}

#[derive(Template)]
#[template(path = "pages/enrich_preview.html")]
struct EnrichPreviewTemplate {
    film_key_str: String,
    data: EnrichmentData,
}

// ── Query params ───────────────────────────────────────────

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

#[derive(Deserialize)]
pub struct SelectForm {
    pub tmdb_id: i64,
}

#[derive(Deserialize)]
pub struct ImdbForm {
    pub imdb_id: String,
}

// ── Handlers ───────────────────────────────────────────────

/// Search TMDB for matching movies.
pub async fn search_tmdb(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
    Query(params): Query<SearchQuery>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let tmdb_key = std::env::var("TMDB_API_KEY")
        .map_err(|_| AppError::Validation("TMDB_API_KEY not configured.".into()))?;

    let query = params.q.unwrap_or_else(|| film.title.clone());
    let client = TmdbClient::new(tmdb_key);
    let results = client.search(&query, film.year).await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("TMDB search error: {e}")))?;

    render_or_error(&EnrichSearchTemplate {
        film_key_str: film_id,
        film_title: film.title,
        results,
        query,
    })
}

/// Preview enrichment data from a selected TMDB result.
pub async fn preview_tmdb(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
    Form(form): Form<SelectForm>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let tmdb_key = std::env::var("TMDB_API_KEY")
        .map_err(|_| AppError::Validation("TMDB_API_KEY not configured.".into()))?;

    let client = TmdbClient::new(tmdb_key);
    let data = client.enrich(form.tmdb_id).await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("TMDB enrichment error: {e}")))?;

    render_or_error(&EnrichPreviewTemplate {
        film_key_str: film_id,
        data,
    })
}

/// Apply TMDB enrichment data to a film (updates metadata, downloads poster, creates cast).
pub async fn apply_tmdb(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
    Form(form): Form<SelectForm>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let tmdb_key = std::env::var("TMDB_API_KEY")
        .map_err(|_| AppError::Validation("TMDB_API_KEY not configured.".into()))?;

    let client = TmdbClient::new(tmdb_key);
    let data = client.enrich(form.tmdb_id).await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("TMDB error: {e}")))?;

    let film_key = crate::util::record_id_key_string(&film.id.key);

    // Download and process poster if available
    let mut poster_url = None;
    let mut poster_thumb = None;
    let mut poster_small = None;
    let mut poster_large = None;

    if let Some(ref poster_path) = data.poster_url {
        if let Ok(poster_bytes) = client.download_poster(
            &poster_path.replace("https://image.tmdb.org/t/p/w780", "")
        ).await {
            if let Ok(keys) = images::upload_poster_variants(&state.storage, &film_key, &poster_bytes).await {
                poster_url = Some(keys.medium);
                poster_thumb = Some(keys.thumb);
                poster_small = Some(keys.small);
                poster_large = Some(keys.large);
            }
        }
    }

    // Update film metadata
    let fid = RecordId::new("film", film_id.as_str());
    state.db.query(
        "UPDATE $fid SET \
            synopsis = $synopsis, \
            tagline = $tagline, \
            year = $year, \
            runtime_minutes = $runtime, \
            genres = $genres, \
            country = $country, \
            language = $language, \
            tmdb_id = $tmdb_id, \
            imdb_id = $imdb_id, \
            poster_url = $poster_url, \
            poster_thumb = $poster_thumb, \
            poster_small = $poster_small, \
            poster_large = $poster_large"
    )
    .bind(("fid", fid.clone()))
    .bind(("synopsis", data.synopsis))
    .bind(("tagline", data.tagline))
    .bind(("year", data.year))
    .bind(("runtime", data.runtime_minutes))
    .bind(("genres", data.genres))
    .bind(("country", data.country))
    .bind(("language", data.language))
    .bind(("tmdb_id", data.tmdb_id))
    .bind(("imdb_id", data.imdb_id))
    .bind(("poster_url", poster_url))
    .bind(("poster_thumb", poster_thumb))
    .bind(("poster_small", poster_small))
    .bind(("poster_large", poster_large))
    .await?;

    // Create cast members (top 20 cast + key crew)
    // Delete existing cast first
    state.db.query("DELETE FROM cast_member WHERE film = $fid")
        .bind(("fid", fid.clone()))
        .await?;

    for (i, member) in data.cast.iter().take(20).enumerate() {
        state.db.query(
            "CREATE cast_member SET \
                name = $name, \
                character_name = $character, \
                department = 'Acting', \
                sort_order = $order, \
                profile_url = $profile, \
                tmdb_id = $tmdb_id, \
                film = $fid"
        )
        .bind(("name", member.name.clone()))
        .bind(("character", member.character.clone()))
        .bind(("order", i as i64))
        .bind(("profile", member.profile_path.as_ref().map(|p| format!("https://image.tmdb.org/t/p/w185{p}"))))
        .bind(("tmdb_id", member.id))
        .bind(("fid", fid.clone()))
        .await?;
    }

    for member in data.crew.iter().filter(|c| {
        matches!(c.job.as_deref(), Some("Director" | "Producer" | "Writer" | "Director of Photography" | "Editor" | "Original Music Composer"))
    }) {
        state.db.query(
            "CREATE cast_member SET \
                name = $name, \
                department = $department, \
                job = $job, \
                sort_order = 100, \
                profile_url = $profile, \
                tmdb_id = $tmdb_id, \
                film = $fid"
        )
        .bind(("name", member.name.clone()))
        .bind(("department", member.department.clone()))
        .bind(("job", member.job.clone()))
        .bind(("profile", member.profile_path.as_ref().map(|p| format!("https://image.tmdb.org/t/p/w185{p}"))))
        .bind(("tmdb_id", member.id))
        .bind(("fid", fid.clone()))
        .await?;
    }

    tracing::info!(film = %film_key, tmdb_id = data.tmdb_id, "Film enriched from TMDB");
    Ok(Redirect::to(&format!("/films/{film_id}")).into_response())
}

/// Enrich from IMDB ID. Uses TMDB's find-by-external-ID endpoint —
/// no separate API key needed, TMDB handles the IMDB cross-reference.
pub async fn enrich_imdb(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(film_id): Path<String>,
    Form(form): Form<ImdbForm>,
) -> Result<Response, AppError> {
    let film = crate::controllers::films::get_film_public(&state, &film_id).await?;
    crate::controllers::films::require_film_ownership(&state, &claims, &film).await?;

    let tmdb_key = std::env::var("TMDB_API_KEY")
        .map_err(|_| AppError::Validation("TMDB_API_KEY not configured.".into()))?;

    let client = TmdbClient::new(tmdb_key);
    let data = client.find_by_imdb_id(&form.imdb_id).await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("TMDB find error: {e}")))?;

    let data = data.ok_or_else(|| {
        AppError::Validation(format!("No movie found for IMDB ID: {}", form.imdb_id))
    })?;

    // Redirect through the apply flow with the TMDB ID we found
    apply_tmdb(
        State(state),
        claims,
        Path(film_id),
        Form(SelectForm { tmdb_id: data.tmdb_id }),
    ).await
}
