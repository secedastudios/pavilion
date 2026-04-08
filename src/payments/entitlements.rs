//! Viewer entitlement checking and granting for purchased or rented films.
//! Manages the lifecycle of access rights — creating entitlements after payment
//! and verifying them before allowing stream playback.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

use crate::db::Db;
use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Entitlement {
    pub id: RecordId,
    pub person: RecordId,
    pub film: RecordId,
    pub platform: RecordId,
    pub entitlement_type: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub external_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateEntitlement {
    pub person: RecordId,
    pub film: RecordId,
    pub platform: RecordId,
    pub entitlement_type: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub external_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct ViewerSubscription {
    pub id: RecordId,
    pub person: RecordId,
    pub platform: RecordId,
    pub provider: String,
    pub external_id: Option<String>,
    pub status: String,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Check if a person has the right to watch a film on a platform.
///
/// Returns the entitlement reason or None if not entitled.
pub async fn check_entitlement(
    db: &Db,
    person_id: &RecordId,
    film_id: &RecordId,
    platform_id: &RecordId,
    license_type: &str,
) -> Result<Option<String>, surrealdb::Error> {
    // AVOD, CC, and free licenses don't require entitlements
    if matches!(license_type, "avod" | "cc" | "free") {
        return Ok(Some("free_access".to_string()));
    }

    // Check for subscription (SVOD)
    if license_type == "svod" {
        let subs: Vec<ViewerSubscription> = db
            .query(
                "SELECT * FROM viewer_subscription \
                 WHERE person = $person AND platform = $platform AND status = 'active' \
                 LIMIT 1",
            )
            .bind(("person", person_id.clone()))
            .bind(("platform", platform_id.clone()))
            .await?
            .take(0)?;

        if !subs.is_empty() {
            return Ok(Some("subscription".to_string()));
        }
    }

    // Check for direct entitlement (TVOD rental/purchase, event ticket)
    let entitlements: Vec<Entitlement> = db
        .query(
            "SELECT * FROM entitlement \
             WHERE person = $person AND film = $film AND platform = $platform \
             AND (expires_at IS NONE OR expires_at > time::now()) \
             LIMIT 1",
        )
        .bind(("person", person_id.clone()))
        .bind(("film", film_id.clone()))
        .bind(("platform", platform_id.clone()))
        .await?
        .take(0)?;

    if let Some(ent) = entitlements.into_iter().next() {
        return Ok(Some(ent.entitlement_type));
    }

    Ok(None)
}

/// Create an entitlement for a viewer.
pub async fn grant_entitlement(
    db: &Db,
    person_id: RecordId,
    film_id: RecordId,
    platform_id: RecordId,
    entitlement_type: &str,
    expires_at: Option<DateTime<Utc>>,
    external_id: Option<String>,
) -> Result<Entitlement, AppError> {
    let ent: Option<Entitlement> = db
        .create("entitlement")
        .content(CreateEntitlement {
            person: person_id,
            film: film_id,
            platform: platform_id,
            entitlement_type: entitlement_type.to_string(),
            expires_at,
            external_id,
        })
        .await?;

    ent.ok_or_else(|| AppError::Internal(anyhow::anyhow!("Failed to create entitlement")))
}

/// Grant entitlements for all films carried by a platform (for SVOD subscriptions).
pub async fn grant_subscription_entitlements(
    db: &Db,
    person_id: &RecordId,
    platform_id: &RecordId,
) -> Result<(), surrealdb::Error> {
    // Get all films carried by this platform
    db.query(
        "FOR $film IN (SELECT out FROM carries WHERE in = $platform) { \
            CREATE entitlement SET \
                person = $person, \
                film = $film.out, \
                platform = $platform, \
                entitlement_type = 'subscription'; \
         }",
    )
    .bind(("person", person_id.clone()))
    .bind(("platform", platform_id.clone()))
    .await?;

    Ok(())
}

/// Revoke subscription entitlements (on cancellation/expiry).
pub async fn revoke_subscription_entitlements(
    db: &Db,
    person_id: &RecordId,
    platform_id: &RecordId,
) -> Result<(), surrealdb::Error> {
    db.query(
        "DELETE FROM entitlement \
         WHERE person = $person AND platform = $platform AND entitlement_type = 'subscription'",
    )
    .bind(("person", person_id.clone()))
    .bind(("platform", platform_id.clone()))
    .await?;

    Ok(())
}
