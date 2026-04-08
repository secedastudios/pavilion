use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

/// A DMCA takedown claim filed against a [`Film`](super::film::Film).
///
/// Status flow: `"filed"` -> `"under_review"` -> `"upheld"` | `"rejected"` | `"counter_filed"`.
/// Active claims (`filed`, `under_review`, `upheld`) block film playback via
/// [`film_has_active_claim`].
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct DmcaClaim {
    pub id: RecordId,
    /// Full legal name of the person filing the claim.
    pub claimant_name: String,
    /// Contact email for the claimant.
    pub claimant_email: String,
    /// Company or organization the claimant represents, if any.
    pub claimant_company: Option<String>,
    /// The film targeted by this claim.
    pub film: RecordId,
    /// Free-text description of the alleged infringement.
    pub description: String,
    /// URL to supporting evidence (e.g. original work, registration certificate).
    pub evidence_url: Option<String>,
    /// Claim status: `"filed"`, `"under_review"`, `"upheld"`, `"rejected"`, `"counter_filed"`.
    pub status: String,
    /// Claimant affirms the claim is made in good faith.
    pub good_faith_statement: bool,
    /// Claimant acknowledges penalty of perjury for false claims.
    pub perjury_declaration: bool,
    /// Reason provided by the filmmaker in a counter-notification.
    pub counter_reason: Option<String>,
    /// Internal notes added by an admin during review.
    pub admin_notes: Option<String>,
    pub filed_at: DateTime<Utc>,
    /// When an admin began reviewing the claim.
    pub reviewed_at: Option<DateTime<Utc>>,
    /// When the claim reached a final resolution.
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Payload for filing a new [`DmcaClaim`]. Status is set to `"filed"` automatically.
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateDmcaClaim {
    pub claimant_name: String,
    pub claimant_email: String,
    pub claimant_company: Option<String>,
    pub film: RecordId,
    pub description: String,
    pub evidence_url: Option<String>,
    pub good_faith_statement: bool,
    pub perjury_declaration: bool,
}

/// Template-safe projection of [`DmcaClaim`] for admin dashboards.
///
/// Excludes `claimant_company`, `evidence_url`, `good_faith_statement`,
/// `perjury_declaration`, `counter_reason`, and `admin_notes`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DmcaClaimView {
    pub id: RecordId,
    pub key_str: String,
    pub claimant_name: String,
    pub claimant_email: String,
    pub film: RecordId,
    pub film_key_str: String,
    pub description: String,
    pub status: String,
    pub filed_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl From<DmcaClaim> for DmcaClaimView {
    fn from(c: DmcaClaim) -> Self {
        Self {
            key_str: crate::util::record_id_key_string(&c.id.key),
            film_key_str: crate::util::record_id_key_string(&c.film.key),
            id: c.id,
            claimant_name: c.claimant_name,
            claimant_email: c.claimant_email,
            film: c.film,
            description: c.description,
            status: c.status,
            filed_at: c.filed_at,
            reviewed_at: c.reviewed_at,
            resolved_at: c.resolved_at,
        }
    }
}

/// Check whether a film has any active DMCA claims (status `filed`, `under_review`, or `upheld`).
///
/// Used in the enforcement chain to block playback and distribution while a claim is pending.
///
/// # Errors
///
/// Returns a [`surrealdb::Error`] if the database query fails.
pub async fn film_has_active_claim(
    db: &crate::db::Db,
    film_id: &RecordId,
) -> Result<bool, surrealdb::Error> {
    let claims: Vec<serde_json::Value> = db
        .query("SELECT id FROM dmca_claim WHERE film = $film AND status IN ['filed', 'under_review', 'upheld'] LIMIT 1")
        .bind(("film", film_id.clone()))
        .await?
        .take(0)?;
    Ok(!claims.is_empty())
}

/// Count the number of upheld DMCA claims across all films by a given filmmaker.
///
/// Used for repeat-infringer detection: if the count exceeds a threshold the
/// filmmaker's account may be suspended.
///
/// # Errors
///
/// Returns a [`surrealdb::Error`] if the database query fails.
pub async fn upheld_claims_for_filmmaker(
    db: &crate::db::Db,
    person_id: &RecordId,
) -> Result<i64, surrealdb::Error> {
    #[derive(serde::Deserialize, SurrealValue)]
    struct CountRow {
        count: Option<i64>,
    }

    let rows: Vec<CountRow> = db
        .query(
            "SELECT count() AS count FROM dmca_claim \
             WHERE status = 'upheld' \
               AND film IN (SELECT out FROM filmmaker_of WHERE in = $person)",
        )
        .bind(("person", person_id.clone()))
        .await?
        .take(0)?;

    Ok(rows.into_iter().next().and_then(|r| r.count).unwrap_or(0))
}
