use std::sync::Arc;

use askama::Template;
use axum::Form;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use surrealdb::types::RecordId;

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::models::rating::{CreateRating, Rating, RatingStats, RatingView};
use crate::router::AppState;
use crate::sse;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/ratings.html")]
struct RatingsTemplate {
    ratings: Vec<RatingView>,
    stats: RatingStats,
    platform_slug: String,
    film_key_str: String,
    user_rating: Option<RatingView>,
    can_rate: bool,
}

// ── Form data ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RatingForm {
    pub score: i64,
    pub review_text: Option<String>,
}

// ── Handlers ───────────────────────────────────────────────

/// Get ratings for a film on a specific platform (SSE fragment).
pub async fn list_ratings(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((platform_slug, film_id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let platform = get_platform_by_slug(&state, &platform_slug).await?;
    let film_record = RecordId::new("film", film_id.as_str());

    let ratings = get_platform_ratings(&state, &film_record, &platform.id).await?;
    let stats = get_platform_stats(&state, &film_record, &platform.id).await?;
    let user_rating =
        get_user_rating(&state, &claims.person_id(), &film_record, &platform.id).await?;

    let html = RatingsTemplate {
        ratings: ratings.into_iter().map(RatingView::from).collect(),
        stats,
        platform_slug,
        film_key_str: film_id,
        user_rating: user_rating.map(RatingView::from),
        can_rate: true,
    }
    .render()
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Template error: {e}")))?;

    Ok(sse::fragment("#film-ratings", html).into_response())
}

/// Submit or update a rating.
pub async fn submit_rating(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((platform_slug, film_id)): Path<(String, String)>,
    Form(form): Form<RatingForm>,
) -> Result<Response, AppError> {
    if !(1..=5).contains(&form.score) {
        return Err(AppError::Validation(
            "Score must be between 1 and 5.".into(),
        ));
    }

    let platform = get_platform_by_slug(&state, &platform_slug).await?;
    let film_record = RecordId::new("film", film_id.as_str());
    let person_id = claims.person_id();

    // Verify the person has watched this film on this platform
    let has_watched: Vec<serde_json::Value> = state
        .db
        .query(
            "SELECT id FROM watch_session \
             WHERE person = $person AND film = $film AND platform = $platform LIMIT 1",
        )
        .bind(("person", person_id.clone()))
        .bind(("film", film_record.clone()))
        .bind(("platform", platform.id.clone()))
        .await?
        .take(0)?;

    if has_watched.is_empty() {
        return Err(AppError::Validation(
            "You must watch this film before rating it.".into(),
        ));
    }

    let review = form.review_text.filter(|t| !t.trim().is_empty());

    // Upsert: update if exists, create if not
    let existing = get_user_rating(&state, &person_id, &film_record, &platform.id).await?;

    if let Some(existing_rating) = existing {
        state
            .db
            .query("UPDATE $rid SET score = $score, review_text = $review")
            .bind(("rid", existing_rating.id))
            .bind(("score", form.score))
            .bind(("review", review))
            .await?;
    } else {
        let _: Option<Rating> = state
            .db
            .create("rating")
            .content(CreateRating {
                person: person_id,
                film: film_record.clone(),
                platform: platform.id.clone(),
                score: form.score,
                review_text: review,
            })
            .await?;
    }

    // Return updated ratings fragment
    list_ratings(State(state), claims, Path((platform_slug, film_id))).await
}

/// Delete own rating.
pub async fn delete_rating(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((platform_slug, film_id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let platform = get_platform_by_slug(&state, &platform_slug).await?;
    let film_record = RecordId::new("film", film_id.as_str());

    state
        .db
        .query(
            "DELETE FROM rating WHERE person = $person AND film = $film AND platform = $platform",
        )
        .bind(("person", claims.person_id()))
        .bind(("film", film_record))
        .bind(("platform", platform.id))
        .await?;

    list_ratings(State(state), claims, Path((platform_slug, film_id))).await
}

/// Curator hides a rating on their platform.
pub async fn hide_rating(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((platform_slug, rating_id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let platform = get_platform_by_slug(&state, &platform_slug).await?;
    crate::controllers::platforms::require_curator_public(&state, &claims, &platform).await?;

    let rid = RecordId::new("rating", rating_id.as_str());
    state
        .db
        .query("UPDATE $rid SET hidden = true")
        .bind(("rid", rid))
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT.into_response())
}

/// Get cross-platform aggregate rating for a film (for catalog/filmmaker dashboard).
pub async fn aggregate_stats(
    state: &AppState,
    film_id: &RecordId,
) -> Result<RatingStats, AppError> {
    let stats: Vec<RatingStats> = state
        .db
        .query(
            "SELECT math::mean(score) AS average, count() AS count \
             FROM rating WHERE film = $film_id AND hidden = false",
        )
        .bind(("film_id", film_id.clone()))
        .await?
        .take(0)?;

    Ok(stats.into_iter().next().unwrap_or_default())
}

// ── Helpers ────────────────────────────────────────────────

use crate::models::platform::Platform;

async fn get_platform_by_slug(state: &AppState, slug: &str) -> Result<Platform, AppError> {
    let platforms: Vec<Platform> = state
        .db
        .query("SELECT * FROM platform WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.to_string()))
        .await?
        .take(0)?;
    platforms.into_iter().next().ok_or(AppError::NotFound)
}

async fn get_platform_ratings(
    state: &AppState,
    film_id: &RecordId,
    platform_id: &RecordId,
) -> Result<Vec<Rating>, surrealdb::Error> {
    let ratings: Vec<Rating> = state
        .db
        .query(
            "SELECT * FROM rating \
             WHERE film = $film AND platform = $platform AND hidden = false \
             ORDER BY created_at DESC LIMIT 50",
        )
        .bind(("film", film_id.clone()))
        .bind(("platform", platform_id.clone()))
        .await?
        .take(0)?;
    Ok(ratings)
}

async fn get_platform_stats(
    state: &AppState,
    film_id: &RecordId,
    platform_id: &RecordId,
) -> Result<RatingStats, surrealdb::Error> {
    // Count and average separately to avoid deserialization issues with math::mean returning NONE
    let ratings = get_platform_ratings(state, film_id, platform_id).await?;
    let count = ratings.len() as i64;
    let average = if count > 0 {
        ratings.iter().map(|r| r.score as f64).sum::<f64>() / count as f64
    } else {
        0.0
    };
    Ok(RatingStats { average, count })
}

async fn get_user_rating(
    state: &AppState,
    person_id: &RecordId,
    film_id: &RecordId,
    platform_id: &RecordId,
) -> Result<Option<Rating>, surrealdb::Error> {
    let ratings: Vec<Rating> = state
        .db
        .query(
            "SELECT * FROM rating \
             WHERE person = $person AND film = $film AND platform = $platform LIMIT 1",
        )
        .bind(("person", person_id.clone()))
        .bind(("film", film_id.clone()))
        .bind(("platform", platform_id.clone()))
        .await?
        .take(0)?;
    Ok(ratings.into_iter().next())
}
