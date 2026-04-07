//! Stream access audit logging to SurrealDB.
//! Records each content access event — including viewer, film, platform,
//! and action — for compliance tracking and analytics.

use surrealdb::types::RecordId;

use crate::db::Db;

pub async fn log_access(
    db: &Db,
    person: Option<RecordId>,
    film: Option<RecordId>,
    platform: Option<RecordId>,
    action: &str,
    result: &str,
    reason: Option<&str>,
) {
    let res = db
        .query(
            "CREATE stream_audit SET \
                person = $person, \
                film = $film, \
                platform = $platform, \
                action = $action, \
                result = $result, \
                reason = $reason"
        )
        .bind(("person", person))
        .bind(("film", film))
        .bind(("platform", platform))
        .bind(("action", action.to_string()))
        .bind(("result", result.to_string()))
        .bind(("reason", reason.map(|s| s.to_string())))
        .await;

    if let Err(err) = res {
        tracing::error!(error = %err, "Failed to write stream audit log");
    }
}
