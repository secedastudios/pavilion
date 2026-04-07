//! Per-filmmaker storage usage tracking and metering.
//! Records byte-level storage consumption for each filmmaker and provides
//! queries to check current usage against tier limits.

use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

use crate::db::Db;

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct StorageUsage {
    pub id: RecordId,
    pub person: RecordId,
    pub total_bytes: i64,
    pub master_bytes: i64,
    pub rendition_bytes: i64,
    pub asset_count: i64,
    pub film_count: i64,
}

/// Get or create storage usage record for a person.
pub async fn get_usage(db: &Db, person_id: &RecordId) -> Result<StorageUsage, surrealdb::Error> {
    let existing: Vec<StorageUsage> = db
        .query("SELECT * FROM storage_usage WHERE person = $person LIMIT 1")
        .bind(("person", person_id.clone()))
        .await?
        .take(0)?;

    if let Some(usage) = existing.into_iter().next() {
        return Ok(usage);
    }

    // Create default record
    let created: Vec<StorageUsage> = db
        .query(
            "CREATE storage_usage SET person = $person, total_bytes = 0, \
             master_bytes = 0, rendition_bytes = 0, asset_count = 0, film_count = 0 \
             RETURN AFTER"
        )
        .bind(("person", person_id.clone()))
        .await?
        .take(0)?;

    created.into_iter().next().ok_or_else(|| {
        surrealdb::Error::thrown("Failed to create storage usage record".into())
    })
}

/// Record a file upload (adds to totals).
pub async fn record_upload(
    db: &Db,
    person_id: &RecordId,
    size_bytes: i64,
    is_master: bool,
) -> Result<(), surrealdb::Error> {
    let field = if is_master { "master_bytes" } else { "rendition_bytes" };
    let query = format!(
        "UPDATE storage_usage SET \
            total_bytes += $size, \
            {field} += $size, \
            asset_count += 1 \
         WHERE person = $person"
    );

    db.query(&query)
        .bind(("size", size_bytes))
        .bind(("person", person_id.clone()))
        .await?;

    Ok(())
}

/// Record a file deletion (subtracts from totals).
pub async fn record_deletion(
    db: &Db,
    person_id: &RecordId,
    size_bytes: i64,
    is_master: bool,
) -> Result<(), surrealdb::Error> {
    let field = if is_master { "master_bytes" } else { "rendition_bytes" };
    let query = format!(
        "UPDATE storage_usage SET \
            total_bytes -= $size, \
            {field} -= $size, \
            asset_count -= 1 \
         WHERE person = $person"
    );

    db.query(&query)
        .bind(("size", size_bytes))
        .bind(("person", person_id.clone()))
        .await?;

    Ok(())
}

/// Increment the film count for a person.
pub async fn increment_film_count(
    db: &Db,
    person_id: &RecordId,
) -> Result<(), surrealdb::Error> {
    db.query("UPDATE storage_usage SET film_count += 1 WHERE person = $person")
        .bind(("person", person_id.clone()))
        .await?;
    Ok(())
}

/// Format bytes into a human-readable string.
pub fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
