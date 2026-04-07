use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Acquisition {
    pub id: RecordId,
    pub film: RecordId,
    pub license: RecordId,
    pub platform: Option<RecordId>,
    pub requester: RecordId,
    pub status: String,
    pub requested_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<RecordId>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateAcquisition {
    pub film: RecordId,
    pub license: RecordId,
    pub platform: Option<RecordId>,
    pub requester: RecordId,
    pub status: String,
}

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
