use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

/// A viewer's rating and optional review of a [`Film`](super::film::Film)
/// on a specific [`Platform`](super::platform::Platform).
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Rating {
    pub id: RecordId,
    /// The person who submitted the rating.
    pub person: RecordId,
    /// The film being rated.
    pub film: RecordId,
    /// The platform where the viewer watched the film.
    pub platform: RecordId,
    /// Numeric score (e.g. 1-5 stars).
    pub score: i64,
    /// Optional free-text review.
    pub review_text: Option<String>,
    /// Whether the rating has been hidden by a moderator.
    pub hidden: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Payload for creating a new [`Rating`] record.
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateRating {
    pub person: RecordId,
    pub film: RecordId,
    pub platform: RecordId,
    pub score: i64,
    pub review_text: Option<String>,
}

/// Template-safe projection of [`Rating`] that excludes `person`, `film`,
/// `platform`, and `hidden` fields. Suitable for public display.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RatingView {
    pub id: RecordId,
    pub key_str: String,
    pub score: i64,
    pub review_text: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<Rating> for RatingView {
    fn from(r: Rating) -> Self {
        Self {
            key_str: crate::util::record_id_key_string(&r.id.key),
            id: r.id,
            score: r.score,
            review_text: r.review_text,
            created_at: r.created_at,
        }
    }
}

/// Aggregated rating statistics for a film, used in listings and detail pages.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone, Default)]
pub struct RatingStats {
    /// Mean score across all visible ratings.
    pub average: f64,
    /// Total number of ratings included in the average.
    pub count: i64,
}
