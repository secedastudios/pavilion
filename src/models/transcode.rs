use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct TranscodeJob {
    pub id: RecordId,
    pub film: RecordId,
    pub status: String,
    pub worker_id: Option<String>,
    pub profile: Option<TranscodeProfile>,
    pub progress_pct: i64,
    pub error_msg: Option<String>,
    pub retry_count: i64,
    pub max_retries: i64,
    pub created_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct TranscodeProfile {
    pub codec: Option<String>,
    pub resolutions: Option<Vec<String>>,
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateTranscodeJob {
    pub film: RecordId,
    pub status: String,
    pub profile: TranscodeProfile,
    pub max_retries: i64,
}

/// View for templates — includes key_str for URL rendering.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscodeJobView {
    pub id: RecordId,
    pub key_str: String,
    pub film: RecordId,
    pub film_key_str: String,
    pub status: String,
    pub progress_pct: i64,
    pub error_msg: Option<String>,
    pub retry_count: i64,
    pub max_retries: i64,
    pub created_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<TranscodeJob> for TranscodeJobView {
    fn from(j: TranscodeJob) -> Self {
        Self {
            key_str: crate::util::record_id_key_string(&j.id.key),
            film_key_str: crate::util::record_id_key_string(&j.film.key),
            id: j.id,
            film: j.film,
            status: j.status,
            progress_pct: j.progress_pct,
            error_msg: j.error_msg,
            retry_count: j.retry_count,
            max_retries: j.max_retries,
            created_at: j.created_at,
            claimed_at: j.claimed_at,
            completed_at: j.completed_at,
        }
    }
}

impl TranscodeProfile {
    /// Default H.264 adaptive bitrate profile.
    pub fn h264_default() -> Self {
        Self {
            codec: Some("h264".to_string()),
            resolutions: Some(vec![
                "360p".to_string(),
                "480p".to_string(),
                "720p".to_string(),
                "1080p".to_string(),
                "1440p".to_string(),
                "2160p".to_string(),
            ]),
            format: Some("cmaf".to_string()),
        }
    }
}
