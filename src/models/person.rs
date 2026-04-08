use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

/// A registered user in the system (filmmaker, platform owner, viewer, or admin).
///
/// This is the full database record including sensitive fields like `password_hash`.
/// Use [`PersonView`] when exposing person data to templates or API responses.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Person {
    pub id: RecordId,
    pub email: String,
    pub name: String,
    /// Argon2-hashed password. Never expose outside the server.
    pub password_hash: String,
    /// Role strings such as `"admin"`, `"filmmaker"`, `"viewer"`.
    pub roles: Vec<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    /// Optional link to the user's SlateHub profile for industry identity.
    pub slatehub_id: Option<String>,
    pub gdpr_consent: Option<GdprConsent>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// GDPR consent preferences recorded for a [`Person`].
///
/// Each field is `None` until the user explicitly opts in or out.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct GdprConsent {
    /// Whether the user consents to marketing communications.
    pub marketing: Option<bool>,
    /// Whether the user consents to analytics tracking.
    pub analytics: Option<bool>,
    /// When the consent preferences were last changed.
    pub updated_at: Option<DateTime<Utc>>,
}

/// Used when creating a new person (registration).
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreatePerson {
    pub email: String,
    pub name: String,
    pub password_hash: String,
    pub roles: Vec<String>,
    pub gdpr_consent: GdprConsent,
}

/// Used for profile updates — never exposes password_hash.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePerson {
    pub name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
}

/// Template-safe projection of [`Person`] that excludes `password_hash`,
/// `slatehub_id`, and `gdpr_consent`. Safe for rendering in HTML templates
/// and JSON API responses.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PersonView {
    pub id: RecordId,
    pub email: String,
    pub name: String,
    pub roles: Vec<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<Person> for PersonView {
    fn from(p: Person) -> Self {
        Self {
            id: p.id,
            email: p.email,
            name: p.name,
            roles: p.roles,
            bio: p.bio,
            avatar_url: p.avatar_url,
            created_at: p.created_at,
        }
    }
}
