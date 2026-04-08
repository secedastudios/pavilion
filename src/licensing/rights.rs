//! Core queries that resolve which films are available on a given platform.
//! Checks active licenses against territory rules and availability windows
//! to produce the authoritative list of streamable content.

use surrealdb::types::{RecordId, SurrealValue};

use crate::db::Db;
use crate::models::license::License;

/// Given a platform's territory, determine which films are available
/// through active, in-window licenses.
///
/// This is the core gatekeeper — no content flows without passing
/// rights resolution.
pub async fn resolve_available_films(
    db: &Db,
    territory: &str,
) -> Result<Vec<FilmWithLicenses>, surrealdb::Error> {
    let results: Vec<FilmWithLicenses> = db
        .query(
            "SELECT \
                out.* AS film, \
                id AS license_id, \
                license_type, \
                territories, \
                window_start, \
                window_end, \
                approval_required \
             FROM licensed_via \
             WHERE out.status = 'published' \
               AND in.active = true \
               AND ($territory IN in.territories OR array::len(in.territories) = 0) \
               AND (in.window_start IS NONE OR in.window_start <= time::now()) \
               AND (in.window_end IS NONE OR in.window_end >= time::now())",
        )
        .bind(("territory", territory.to_string()))
        .await?
        .take(0)?;

    Ok(results)
}

/// Check if a specific film has an active license available for a territory.
pub async fn film_is_licensed_for(
    db: &Db,
    film_id: &RecordId,
    territory: &str,
) -> Result<Vec<License>, surrealdb::Error> {
    let licenses: Vec<License> = db
        .query(
            "SELECT in.* FROM licensed_via \
             WHERE out = $film_id \
               AND in.active = true \
               AND ($territory IN in.territories OR array::len(in.territories) = 0) \
               AND (in.window_start IS NONE OR in.window_start <= time::now()) \
               AND (in.window_end IS NONE OR in.window_end >= time::now())",
        )
        .bind(("film_id", film_id.clone()))
        .bind(("territory", territory.to_string()))
        .await?
        .take(0)?;

    Ok(licenses)
}

/// Check if a film has any active license at all (regardless of territory).
pub async fn film_has_any_license(db: &Db, film_id: &RecordId) -> Result<bool, surrealdb::Error> {
    // licensed_via: FROM film TO license — in=film, out=license
    let count: Vec<serde_json::Value> = db
        .query(
            "SELECT count() AS c FROM licensed_via \
             WHERE in = $film_id \
               AND out.active = true \
               AND (out.window_start IS NONE OR out.window_start <= time::now()) \
               AND (out.window_end IS NONE OR out.window_end >= time::now()) \
             LIMIT 1",
        )
        .bind(("film_id", film_id.clone()))
        .await?
        .take(0)?;

    Ok(!count.is_empty())
}

/// Get all licenses for a film (via the licensed_via relation).
pub async fn licenses_for_film(
    db: &Db,
    film_id: &RecordId,
) -> Result<Vec<License>, surrealdb::Error> {
    let licenses: Vec<License> = db
        .query(
            "SELECT * FROM license WHERE <-licensed_via<-film CONTAINS $film_id ORDER BY created_at DESC"
        )
        .bind(("film_id", film_id.clone()))
        .await?
        .take(0)?;

    Ok(licenses)
}

/// Intermediate struct for rights resolution query results.
#[derive(Debug, serde::Deserialize, SurrealValue, Clone)]
pub struct FilmWithLicenses {
    pub film: serde_json::Value,
    pub license_id: RecordId,
    pub license_type: String,
    pub territories: Vec<String>,
    pub window_start: Option<chrono::DateTime<chrono::Utc>>,
    pub window_end: Option<chrono::DateTime<chrono::Utc>>,
    pub approval_required: bool,
}
