//! JWT claims structure, token issuance, and verification.
//!
//! Tokens carry the person's ID, display name, and roles. They're issued
//! on login/registration and verified on every authenticated request via
//! the `Claims` extractor in the `middleware` module.

use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use surrealdb::types::RecordId;

use crate::util::RecordIdExt;

/// How long a JWT remains valid after issuance.
const TOKEN_EXPIRY_SECS: u64 = 24 * 60 * 60; // 24 hours

/// Authenticated person's identity extracted from a JWT.
///
/// Carried in every authenticated request via the Axum extractor.
/// Contains just enough information for authorization decisions
/// without hitting the database on every request.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Person record key (the `key` portion of `person:<key>`).
    pub sub: String,
    /// Display name at the time the token was issued.
    pub name: String,
    /// Roles granted to this person (e.g., `["filmmaker", "curator"]`).
    pub roles: Vec<String>,
    /// Expiration timestamp (seconds since UNIX epoch).
    pub exp: u64,
    /// Issued-at timestamp (seconds since UNIX epoch).
    pub iat: u64,
}

impl Claims {
    /// Reconstruct the full SurrealDB [`RecordId`] for this person.
    pub fn person_id(&self) -> RecordId {
        RecordId::new("person", self.sub.as_str())
    }

    /// The person's record key as a displayable string.
    ///
    /// Convenience method that avoids the `RecordIdExt` import at call sites.
    pub fn person_key_str(&self) -> String {
        self.person_id().key_str()
    }

    /// Check whether this person has a specific role.
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

/// Issue a signed JWT for a person.
///
/// # Arguments
///
/// * `person_key` — The record key portion of `person:<key>` (not the full RecordId).
/// * `name` — Display name to embed in the token.
/// * `roles` — Roles to embed (e.g., `&["filmmaker".into()]`).
/// * `secret` — HMAC secret for signing.
///
/// # Errors
///
/// Returns a `jsonwebtoken` error if encoding fails (should not happen
/// with valid inputs).
pub fn issue_token(
    person_key: &str,
    name: &str,
    roles: &[String],
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = now_epoch_secs();

    let claims = Claims {
        sub: person_key.to_string(),
        name: name.to_string(),
        roles: roles.to_vec(),
        exp: now + TOKEN_EXPIRY_SECS,
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// Verify a JWT and extract the claims.
///
/// Checks the HMAC signature and expiration. Returns an error if the
/// token is malformed, expired, or signed with a different secret.
pub fn verify_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

/// Current time as seconds since UNIX epoch.
///
/// Panics if the system clock is before 1970, which indicates a
/// fundamentally broken environment.
fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before 1970")
        .as_secs()
}
