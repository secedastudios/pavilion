use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use surrealdb::types::RecordId;

use crate::auth::claims::Claims;
use crate::auth::middleware::OptionalClaims;
use crate::error::AppError;
use crate::models::event::{CreateEvent, Event, EventView};
use crate::router::AppState;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/events_index.html")]
struct EventsIndexTemplate {
    events: Vec<EventView>,
    platform_key_str: String,
    platform_name: String,
}

#[derive(Template)]
#[template(path = "pages/event_new.html")]
struct EventNewTemplate {
    platform_key_str: String,
    platform_name: String,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/event_detail.html")]
struct EventDetailTemplate {
    event: EventView,
    is_attending: bool,
    is_curator: bool,
    claims: Option<Claims>,
}

// ── Form data ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EventForm {
    pub title: String,
    pub description: Option<String>,
    pub event_type: String,
    pub film_id: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub max_attendees: Option<String>,
    pub ticket_price: Option<String>,
}

// ── Handlers ───────────────────────────────────────────────

pub async fn index(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(platform_id): Path<String>,
) -> Result<Response, AppError> {
    let platform = crate::controllers::platforms::get_platform(&state, &platform_id).await?;
    crate::controllers::platforms::require_curator_public(&state, &claims, &platform).await?;

    let pid = RecordId::new("platform", platform_id.as_str());
    let events: Vec<Event> = state.db
        .query("SELECT * FROM event WHERE platform = $pid ORDER BY start_time DESC")
        .bind(("pid", pid))
        .await?.take(0)?;

    let mut views = Vec::new();
    for e in events {
        let count = attendee_count(&state, &e.id).await;
        views.push(EventView::from_event(e, count));
    }

    render_or_error(&EventsIndexTemplate {
        events: views,
        platform_key_str: platform_id,
        platform_name: platform.name,
    })
}

pub async fn new_form(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(platform_id): Path<String>,
) -> Result<Response, AppError> {
    let platform = crate::controllers::platforms::get_platform(&state, &platform_id).await?;
    crate::controllers::platforms::require_curator_public(&state, &claims, &platform).await?;

    render_or_error(&EventNewTemplate {
        platform_key_str: platform_id,
        platform_name: platform.name,
        error: None,
    })
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(platform_id): Path<String>,
    Form(form): Form<EventForm>,
) -> Result<Response, AppError> {
    let platform = crate::controllers::platforms::get_platform(&state, &platform_id).await?;
    crate::controllers::platforms::require_curator_public(&state, &claims, &platform).await?;

    if form.title.trim().is_empty() {
        return render_or_error(&EventNewTemplate {
            platform_key_str: platform_id,
            platform_name: platform.name,
            error: Some("Title is required.".into()),
        });
    }

    let start_time = chrono::DateTime::parse_from_rfc3339(&format!("{}:00Z", form.start_time))
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(&form.start_time))
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|_| AppError::Validation("Invalid start time format.".into()))?;

    let end_time = form.end_time.as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&format!("{s}:00Z"))
                .or_else(|_| chrono::DateTime::parse_from_rfc3339(s))
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok()
        });

    let max_attendees: Option<i64> = form.max_attendees.as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok());

    let ticket_price_cents: Option<i64> = form.ticket_price.as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v * 100.0).round() as i64);

    let event: Option<Event> = state.db
        .create("event")
        .content(CreateEvent {
            title: form.title.trim().to_string(),
            description: form.description.filter(|s| !s.trim().is_empty()),
            event_type: form.event_type,
            film: RecordId::new("film", form.film_id.as_str()),
            platform: RecordId::new("platform", platform_id.as_str()),
            start_time,
            end_time,
            max_attendees,
            ticket_price_cents,
            status: "upcoming".to_string(),
        })
        .await?;

    let event = event.ok_or_else(|| AppError::Internal(anyhow::anyhow!("Failed to create event")))?;
    let key = crate::util::record_id_key_string(&event.id.key);
    Ok(Redirect::to(&format!("/events/{key}")).into_response())
}

pub async fn detail(
    State(state): State<Arc<AppState>>,
    OptionalClaims(claims): OptionalClaims,
    Path(event_id): Path<String>,
) -> Result<Response, AppError> {
    let event = get_event(&state, &event_id).await?;
    let count = attendee_count(&state, &event.id).await;

    let is_attending = if let Some(ref c) = claims {
        is_person_attending(&state, &c.person_id(), &event.id).await
    } else {
        false
    };

    let is_curator = if let Some(ref c) = claims {
        let platform = crate::controllers::platforms::get_platform(
            &state,
            &crate::util::record_id_key_string(&event.platform.key),
        ).await;
        match platform {
            Ok(p) => crate::controllers::platforms::require_curator_public(&state, c, &p).await.is_ok(),
            Err(_) => false,
        }
    } else {
        false
    };

    render_or_error(&EventDetailTemplate {
        event: EventView::from_event(event, count),
        is_attending,
        is_curator,
        claims,
    })
}

/// Purchase a ticket / register for an event.
pub async fn purchase_ticket(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(event_id): Path<String>,
) -> Result<Response, AppError> {
    let event = get_event(&state, &event_id).await?;

    if event.status != "upcoming" && event.status != "live" {
        return Err(AppError::Validation("This event is no longer accepting attendees.".into()));
    }

    // Check attendee cap
    if let Some(max) = event.max_attendees {
        let count = attendee_count(&state, &event.id).await;
        if count >= max {
            return Err(AppError::Validation("This event is sold out.".into()));
        }
    }

    // Check if already attending
    if is_person_attending(&state, &claims.person_id(), &event.id).await {
        return Err(AppError::Validation("You are already registered for this event.".into()));
    }

    // TODO: If ticket_price_cents > 0, route through payment flow (Phase 9)
    // For now, create the attendance record directly (free events or deferred payment)
    let ticket_id = uuid::Uuid::now_v7().to_string();
    state.db
        .query("RELATE $person->attending->$event SET ticket_id = $ticket_id")
        .bind(("person", claims.person_id()))
        .bind(("event", event.id))
        .bind(("ticket_id", ticket_id))
        .await?;

    Ok(Redirect::to(&format!("/events/{event_id}")).into_response())
}

/// Curator sets event status (upcoming → live → ended).
pub async fn update_status(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(event_id): Path<String>,
    Form(form): Form<StatusForm>,
) -> Result<Response, AppError> {
    let event = get_event(&state, &event_id).await?;
    let platform_key = crate::util::record_id_key_string(&event.platform.key);
    let platform = crate::controllers::platforms::get_platform(&state, &platform_key).await?;
    crate::controllers::platforms::require_curator_public(&state, &claims, &platform).await?;

    let valid = matches!(
        (event.status.as_str(), form.status.as_str()),
        ("upcoming", "live") | ("live", "ended") | ("upcoming", "canceled")
    );

    if !valid {
        return Err(AppError::Validation(format!(
            "Cannot transition from '{}' to '{}'.", event.status, form.status
        )));
    }

    let eid = RecordId::new("event", event_id.as_str());
    state.db
        .query("UPDATE $eid SET status = $status")
        .bind(("eid", eid))
        .bind(("status", form.status))
        .await?;

    Ok(Redirect::to(&format!("/events/{event_id}")).into_response())
}

#[derive(Deserialize)]
pub struct StatusForm {
    pub status: String,
}

// ── Helpers ────────────────────────────────────────────────

async fn get_event(state: &AppState, id: &str) -> Result<Event, AppError> {
    let eid = RecordId::new("event", id);
    let event: Option<Event> = state.db.select(eid).await?;
    event.ok_or(AppError::NotFound)
}

async fn attendee_count(state: &AppState, event_id: &RecordId) -> i64 {
    use surrealdb::types::SurrealValue;
    #[derive(serde::Deserialize, SurrealValue)]
    struct CountRow { count: Option<i64> }

    let rows: Result<Vec<CountRow>, _> = state.db
        .query("SELECT count() AS count FROM attending WHERE out = $event")
        .bind(("event", event_id.clone()))
        .await
        .and_then(|mut r| r.take(0));

    rows.ok()
        .and_then(|r| r.into_iter().next())
        .and_then(|r| r.count)
        .unwrap_or(0)
}

async fn is_person_attending(state: &AppState, person_id: &RecordId, event_id: &RecordId) -> bool {
    let result: Result<Vec<serde_json::Value>, _> = state.db
        .query("SELECT id FROM attending WHERE in = $person AND out = $event LIMIT 1")
        .bind(("person", person_id.clone()))
        .bind(("event", event_id.clone()))
        .await
        .and_then(|mut r| r.take(0));

    result.map(|r| !r.is_empty()).unwrap_or(false)
}
