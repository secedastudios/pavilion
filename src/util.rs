//! Shared utility functions and extension traits.

use surrealdb::types::{RecordId, RecordIdKey};

use crate::db::Db;
use crate::error::AppError;

/// Extract a displayable string from a [`RecordIdKey`].
///
/// SurrealDB v3's `RecordIdKey` doesn't implement `Display`, so we
/// pattern-match to get a human-readable string for URLs and templates.
///
/// # Examples
///
/// ```ignore
/// use surrealdb::types::RecordIdKey;
/// let key = RecordIdKey::String("abc123".into());
/// assert_eq!(record_id_key_string(&key), "abc123");
/// ```
pub fn record_id_key_string(key: &RecordIdKey) -> String {
    match key {
        RecordIdKey::String(s) => s.clone(),
        RecordIdKey::Number(n) => n.to_string(),
        RecordIdKey::Uuid(u) => u.to_string(),
        other => format!("{other:?}"),
    }
}

/// Extension trait for [`RecordId`] that provides a displayable key string.
///
/// Use this instead of calling `record_id_key_string` directly.
///
/// # Examples
///
/// ```ignore
/// use pavilion::util::RecordIdExt;
/// let id = RecordId::new("film", "abc123");
/// assert_eq!(id.key_str(), "abc123");
/// ```
pub trait RecordIdExt {
    /// Get the record's key as a displayable string.
    fn key_str(&self) -> String;
}

impl RecordIdExt for RecordId {
    fn key_str(&self) -> String {
        record_id_key_string(&self.key)
    }
}

/// Check whether a graph relation exists between a person and a target record.
///
/// This is the shared implementation behind film ownership checks,
/// curator verification, and other permission gates.
///
/// # Arguments
///
/// * `relation` — The SurrealDB relation table name (e.g., `"filmmaker_of"`, `"curator_of"`)
///
/// # Examples
///
/// ```ignore
/// let is_owner = verify_relation(&db, &person_id, "filmmaker_of", &film_id).await?;
/// if !is_owner {
///     return Err(AppError::Forbidden);
/// }
/// ```
pub async fn verify_relation(
    db: &Db,
    person_id: &RecordId,
    relation: &str,
    target_id: &RecordId,
) -> Result<bool, surrealdb::Error> {
    let result: Vec<serde_json::Value> = db
        .query(&format!(
            "SELECT id FROM {relation} WHERE in = $person_id AND out = $target_id LIMIT 1"
        ))
        .bind(("person_id", person_id.clone()))
        .bind(("target_id", target_id.clone()))
        .await?
        .take(0)?;

    Ok(!result.is_empty())
}

/// Check a graph relation and return `Err(AppError::Forbidden)` if it doesn't exist.
///
/// Convenience wrapper around [`verify_relation`] for use in route handlers.
pub async fn require_relation(
    db: &Db,
    person_id: &RecordId,
    relation: &str,
    target_id: &RecordId,
) -> Result<(), AppError> {
    let exists = verify_relation(db, person_id, relation, target_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Relation check failed: {e}")))?;

    if exists {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

/// Slugify a string for use in URLs.
///
/// Converts to lowercase, replaces non-alphanumeric chars with hyphens,
/// and collapses consecutive hyphens.
pub fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Validation constants used across the application.
pub mod validation {
    /// Minimum password length for registration.
    pub const MIN_PASSWORD_LENGTH: usize = 8;

    /// Maximum number of TMDB cast members to import.
    pub const MAX_CAST_IMPORT: usize = 20;

    /// Default segment token TTL in seconds (5 minutes).
    pub const SEGMENT_TOKEN_TTL_SECS: u64 = 300;

    /// Default segment URL prefix.
    pub const SEGMENT_URL_PREFIX: &str = "/segments/";
}
