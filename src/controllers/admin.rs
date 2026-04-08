use std::sync::Arc;

use askama::Template;
use axum::Form;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use surrealdb::types::{RecordId, SurrealValue};

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::models::dmca::{DmcaClaim, DmcaClaimView};
use crate::models::person::Person;
use crate::router::AppState;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/admin_dashboard.html")]
struct AdminDashboardTemplate {
    person_count: i64,
    film_count: i64,
    platform_count: i64,
    pending_dmca: i64,
    active_streams: i64,
    queued_jobs: i64,
}

#[derive(Template)]
#[template(path = "pages/admin_persons.html")]
struct AdminPersonsTemplate {
    persons: Vec<AdminPersonView>,
}

#[derive(Template)]
#[template(path = "pages/admin_dmca.html")]
struct AdminDmcaTemplate {
    claims: Vec<DmcaClaimView>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AdminPersonView {
    key_str: String,
    email: String,
    name: String,
    roles: Vec<String>,
    status: String,
}

use serde::Serialize;

// ── Middleware ──────────────────────────────────────────────

fn require_admin(claims: &Claims) -> Result<(), AppError> {
    if claims.has_role("admin") {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

// ── Handlers ───────────────────────────────────────────────

pub async fn dashboard(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    require_admin(&claims)?;

    let person_count = count_table(&state, "person").await;
    let film_count = count_table(&state, "film").await;
    let platform_count = count_table(&state, "platform").await;
    let pending_dmca =
        count_where(&state, "dmca_claim", "status IN ['filed', 'under_review']").await;
    let active_streams =
        count_where(&state, "watch_session", "last_heartbeat > time::now() - 5m").await;
    let queued_jobs = count_where(
        &state,
        "transcode_job",
        "status IN ['queued', 'claimed', 'processing']",
    )
    .await;

    render_or_error(&AdminDashboardTemplate {
        person_count,
        film_count,
        platform_count,
        pending_dmca,
        active_streams,
        queued_jobs,
    })
}

pub async fn persons(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    require_admin(&claims)?;

    let persons: Vec<Person> = state
        .db
        .query("SELECT * FROM person ORDER BY created_at DESC LIMIT 100")
        .await?
        .take(0)?;

    let views: Vec<AdminPersonView> = persons
        .into_iter()
        .map(|p| AdminPersonView {
            key_str: crate::util::record_id_key_string(&p.id.key),
            email: p.email,
            name: p.name,
            roles: p.roles,
            status: "active".into(),
        })
        .collect();

    render_or_error(&AdminPersonsTemplate { persons: views })
}

#[derive(Deserialize)]
pub struct RoleForm {
    pub roles: String,
}

pub async fn update_roles(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(person_id): Path<String>,
    Form(form): Form<RoleForm>,
) -> Result<Response, AppError> {
    require_admin(&claims)?;

    let roles: Vec<String> = form
        .roles
        .split(',')
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .collect();

    let pid = RecordId::new("person", person_id.as_str());
    state
        .db
        .query("UPDATE $pid SET roles = $roles")
        .bind(("pid", pid))
        .bind(("roles", roles))
        .await?;

    Ok(Redirect::to("/admin/persons").into_response())
}

pub async fn dmca_list(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    require_admin(&claims)?;

    let dmca_claims: Vec<DmcaClaim> = state
        .db
        .query("SELECT * FROM dmca_claim ORDER BY filed_at DESC LIMIT 100")
        .await?
        .take(0)?;

    let views: Vec<DmcaClaimView> = dmca_claims.into_iter().map(DmcaClaimView::from).collect();
    render_or_error(&AdminDmcaTemplate { claims: views })
}

/// GDPR: export all data for a person.
pub async fn gdpr_export(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(person_id): Path<String>,
) -> Result<Response, AppError> {
    require_admin(&claims)?;

    let pid = RecordId::new("person", person_id.as_str());
    let person: Option<Person> = state.db.select(pid.clone()).await?;
    let person = person.ok_or(AppError::NotFound)?;

    let export = serde_json::json!({
        "person": {
            "email": person.email,
            "name": person.name,
            "roles": person.roles,
            "bio": person.bio,
            "created_at": person.created_at,
        },
        "exported_at": chrono::Utc::now(),
        "exported_by": "admin",
    });

    Ok((
        axum::http::StatusCode::OK,
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/json".to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"gdpr-export-{person_id}.json\""),
            ),
        ],
        serde_json::to_string_pretty(&export)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("JSON error: {e}")))?,
    )
        .into_response())
}

/// GDPR: delete a person and all their data.
pub async fn gdpr_delete(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(person_id): Path<String>,
) -> Result<Response, AppError> {
    require_admin(&claims)?;

    let pid = RecordId::new("person", person_id.as_str());

    state
        .db
        .query(
            "DELETE FROM agreed_to WHERE in = $pid; \
         DELETE FROM filmmaker_of WHERE in = $pid; \
         DELETE FROM curator_of WHERE in = $pid; \
         DELETE FROM attending WHERE in = $pid; \
         DELETE FROM watch_session WHERE person = $pid; \
         DELETE FROM entitlement WHERE person = $pid; \
         DELETE FROM viewer_subscription WHERE person = $pid; \
         DELETE FROM rating WHERE person = $pid; \
         DELETE FROM credit_balance WHERE person = $pid; \
         DELETE FROM credit_transaction WHERE person = $pid; \
         DELETE FROM storage_usage WHERE person = $pid; \
         DELETE $pid;",
        )
        .bind(("pid", pid))
        .await?;

    tracing::info!(person = %person_id, "GDPR erasure completed by admin");
    Ok(Redirect::to("/admin/persons").into_response())
}

// ── Helpers ────────────────────────────────────────────────

/// Tables allowed in dynamic count queries. Prevents SurrealQL injection
/// if these helpers are ever called with non-hardcoded values.
const ALLOWED_TABLES: &[&str] = &[
    "person",
    "film",
    "platform",
    "dmca_claim",
    "watch_session",
    "transcode_job",
];

async fn count_table(state: &AppState, table: &str) -> i64 {
    debug_assert!(
        ALLOWED_TABLES.contains(&table),
        "count_table called with unknown table: {table}"
    );
    if !ALLOWED_TABLES.contains(&table) {
        return 0;
    }

    #[derive(Deserialize, SurrealValue)]
    struct C {
        count: Option<i64>,
    }

    let query = format!("SELECT count() AS count FROM {table}");
    let rows: Result<Vec<C>, _> = state.db.query(&query).await.and_then(|mut r| r.take(0));
    rows.ok()
        .and_then(|r| r.into_iter().next())
        .and_then(|r| r.count)
        .unwrap_or(0)
}

async fn count_where(state: &AppState, table: &str, condition: &str) -> i64 {
    debug_assert!(
        ALLOWED_TABLES.contains(&table),
        "count_where called with unknown table: {table}"
    );
    if !ALLOWED_TABLES.contains(&table) {
        return 0;
    }

    #[derive(Deserialize, SurrealValue)]
    struct C {
        count: Option<i64>,
    }

    let query = format!("SELECT count() AS count FROM {table} WHERE {condition}");
    let rows: Result<Vec<C>, _> = state.db.query(&query).await.and_then(|mut r| r.take(0));
    rows.ok()
        .and_then(|r| r.into_iter().next())
        .and_then(|r| r.count)
        .unwrap_or(0)
}
