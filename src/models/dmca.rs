use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct DmcaClaim {
    pub id: RecordId,
    pub claimant_name: String,
    pub claimant_email: String,
    pub claimant_company: Option<String>,
    pub film: RecordId,
    pub description: String,
    pub evidence_url: Option<String>,
    pub status: String,
    pub good_faith_statement: bool,
    pub perjury_declaration: bool,
    pub counter_reason: Option<String>,
    pub admin_notes: Option<String>,
    pub filed_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
}

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

/// Check if a film has any upheld DMCA claims (used in enforcement chain).
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

/// Count upheld claims for a person's films (repeat infringer detection).
pub async fn upheld_claims_for_filmmaker(
    db: &crate::db::Db,
    person_id: &RecordId,
) -> Result<i64, surrealdb::Error> {
    #[derive(serde::Deserialize, SurrealValue)]
    struct CountRow { count: Option<i64> }

    let rows: Vec<CountRow> = db
        .query(
            "SELECT count() AS count FROM dmca_claim \
             WHERE status = 'upheld' \
               AND film IN (SELECT out FROM filmmaker_of WHERE in = $person)"
        )
        .bind(("person", person_id.clone()))
        .await?
        .take(0)?;

    Ok(rows.into_iter().next().and_then(|r| r.count).unwrap_or(0))
}
