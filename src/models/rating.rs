use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Rating {
    pub id: RecordId,
    pub person: RecordId,
    pub film: RecordId,
    pub platform: RecordId,
    pub score: i64,
    pub review_text: Option<String>,
    pub hidden: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateRating {
    pub person: RecordId,
    pub film: RecordId,
    pub platform: RecordId,
    pub score: i64,
    pub review_text: Option<String>,
}

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

/// Aggregated rating stats for display.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone, Default)]
pub struct RatingStats {
    pub average: f64,
    pub count: i64,
}
