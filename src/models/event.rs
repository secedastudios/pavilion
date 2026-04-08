use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

/// A scheduled screening or live event on a [`Platform`](super::platform::Platform)
/// for a specific [`Film`](super::film::Film).
///
/// Events can be free or ticketed and may have a capacity limit.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Event {
    pub id: RecordId,
    pub title: String,
    pub description: Option<String>,
    /// Kind of event: `"screening"`, `"premiere"`, `"q_and_a"`, etc.
    pub event_type: String,
    /// The film being screened.
    pub film: RecordId,
    /// The platform hosting the event.
    pub platform: RecordId,
    pub start_time: DateTime<Utc>,
    /// End time; `None` if open-ended or determined by film runtime.
    pub end_time: Option<DateTime<Utc>>,
    /// Capacity cap. `None` means unlimited.
    pub max_attendees: Option<i64>,
    /// Ticket price in cents. `None` or `0` means free admission.
    pub ticket_price_cents: Option<i64>,
    /// Lifecycle status: `"draft"`, `"scheduled"`, `"live"`, `"completed"`, `"cancelled"`.
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Payload for creating a new [`Event`] record.
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateEvent {
    pub title: String,
    pub description: Option<String>,
    pub event_type: String,
    pub film: RecordId,
    pub platform: RecordId,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub max_attendees: Option<i64>,
    pub ticket_price_cents: Option<i64>,
    pub status: String,
}

/// Template-safe projection of [`Event`] with pre-computed display values.
///
/// Includes `ticket_price_display` (formatted as `"$X.XX"` or `"Free"`) and
/// `attendee_count` (current number of registered attendees) so templates
/// need no business logic.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventView {
    pub id: RecordId,
    /// String representation of the record key for use in URLs and templates.
    pub key_str: String,
    pub title: String,
    pub description: Option<String>,
    pub event_type: String,
    pub film: RecordId,
    pub film_key_str: String,
    pub platform: RecordId,
    pub platform_key_str: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub max_attendees: Option<i64>,
    pub ticket_price_cents: Option<i64>,
    /// Human-readable price string, e.g. `"$5.00"` or `"Free"`.
    pub ticket_price_display: String,
    pub status: String,
    /// Current number of attendees registered for this event.
    pub attendee_count: i64,
}

impl EventView {
    /// Build an [`EventView`] from an [`Event`] and its current attendee count.
    ///
    /// Formats the ticket price for display and converts all `RecordId` keys
    /// to template-safe strings.
    pub fn from_event(e: Event, attendee_count: i64) -> Self {
        let price_display = e
            .ticket_price_cents
            .map(|c| format!("${:.2}", c as f64 / 100.0))
            .unwrap_or_else(|| "Free".into());
        Self {
            key_str: crate::util::record_id_key_string(&e.id.key),
            film_key_str: crate::util::record_id_key_string(&e.film.key),
            platform_key_str: crate::util::record_id_key_string(&e.platform.key),
            id: e.id,
            title: e.title,
            description: e.description,
            event_type: e.event_type,
            film: e.film,
            platform: e.platform,
            start_time: e.start_time,
            end_time: e.end_time,
            max_attendees: e.max_attendees,
            ticket_price_cents: e.ticket_price_cents,
            ticket_price_display: price_display,
            status: e.status,
            attendee_count,
        }
    }
}
