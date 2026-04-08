use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

/// A background job that transcodes a [`Film`](super::film::Film)'s source video
/// into adaptive-bitrate renditions.
///
/// Status flow: `"queued"` -> `"processing"` -> `"completed"` | `"failed"`.
/// Failed jobs are automatically retried up to `max_retries` times.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct TranscodeJob {
    pub id: RecordId,
    /// The film whose source video is being transcoded.
    pub film: RecordId,
    /// Job status: `"queued"`, `"processing"`, `"completed"`, or `"failed"`.
    pub status: String,
    /// Identifier of the worker process that claimed this job.
    pub worker_id: Option<String>,
    /// Codec, resolution, and format settings for the transcode.
    pub profile: Option<TranscodeProfile>,
    /// Completion percentage (0-100).
    pub progress_pct: i64,
    /// Error message from the most recent failure, if any.
    pub error_msg: Option<String>,
    /// How many times this job has been retried after failure.
    pub retry_count: i64,
    /// Maximum retry attempts before the job is marked permanently failed.
    pub max_retries: i64,
    pub created_at: DateTime<Utc>,
    /// When a worker claimed the job for processing.
    pub claimed_at: Option<DateTime<Utc>>,
    /// When the job finished (successfully or after final failure).
    pub completed_at: Option<DateTime<Utc>>,
}

/// Configuration for a transcode job specifying the target codec, resolutions,
/// and container format.
///
/// Use [`TranscodeProfile::h264_default`] for the standard adaptive-bitrate ladder.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct TranscodeProfile {
    /// Video codec, e.g. `"h264"`, `"h265"`, `"av1"`.
    pub codec: Option<String>,
    /// Target resolution labels, e.g. `["360p", "720p", "1080p"]`.
    pub resolutions: Option<Vec<String>>,
    /// Container/packaging format, e.g. `"cmaf"`, `"hls"`, `"dash"`.
    pub format: Option<String>,
}

/// Payload for creating a new [`TranscodeJob`].
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateTranscodeJob {
    pub film: RecordId,
    pub status: String,
    pub profile: TranscodeProfile,
    pub max_retries: i64,
}

/// Template-safe projection of [`TranscodeJob`] with string keys for URL rendering.
///
/// Excludes `worker_id` and `profile` (internal implementation details).
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
