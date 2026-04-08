use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

/// A request by a platform or person to acquire distribution rights to a [`Film`](super::film::Film)
/// under a specific [`License`](super::license::License).
///
/// Status flow: `"pending"` -> `"approved"` | `"rejected"`.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Acquisition {
    pub id: RecordId,
    /// The film being acquired.
    pub film: RecordId,
    /// The license governing the terms of acquisition.
    pub license: RecordId,
    /// The platform requesting the film (if platform-initiated).
    pub platform: Option<RecordId>,
    /// The person who submitted the acquisition request.
    pub requester: RecordId,
    /// Workflow status: `"pending"`, `"approved"`, or `"rejected"`.
    pub status: String,
    pub requested_at: DateTime<Utc>,
    /// When the filmmaker or admin resolved the request.
    pub resolved_at: Option<DateTime<Utc>>,
    /// The person who approved or rejected the request.
    pub resolved_by: Option<RecordId>,
}

/// Payload for creating a new [`Acquisition`] request.
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateAcquisition {
    pub film: RecordId,
    pub license: RecordId,
    pub platform: Option<RecordId>,
    pub requester: RecordId,
    pub status: String,
}

/// Template-safe projection of [`Acquisition`] with string keys for all `RecordId` references.
///
/// Excludes `resolved_by` and provides `*_key_str` fields for URL construction in templates.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AcquisitionView {
    pub id: RecordId,
    pub key_str: String,
    pub film: RecordId,
    pub film_key_str: String,
    pub license: RecordId,
    pub license_key_str: String,
    pub requester: RecordId,
    pub requester_key_str: String,
    pub status: String,
    pub requested_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl From<Acquisition> for AcquisitionView {
    fn from(a: Acquisition) -> Self {
        Self {
            key_str: crate::util::record_id_key_string(&a.id.key),
            film_key_str: crate::util::record_id_key_string(&a.film.key),
            license_key_str: crate::util::record_id_key_string(&a.license.key),
            requester_key_str: crate::util::record_id_key_string(&a.requester.key),
            id: a.id,
            film: a.film,
            license: a.license,
            requester: a.requester,
            status: a.status,
            requested_at: a.requested_at,
            resolved_at: a.resolved_at,
        }
    }
}
