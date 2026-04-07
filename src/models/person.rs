use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Person {
    pub id: RecordId,
    pub email: String,
    pub name: String,
    pub password_hash: String,
    pub roles: Vec<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub slatehub_id: Option<String>,
    pub gdpr_consent: Option<GdprConsent>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct GdprConsent {
    pub marketing: Option<bool>,
    pub analytics: Option<bool>,
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

/// View model — never exposes password_hash.
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
