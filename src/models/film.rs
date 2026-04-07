use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Film {
    pub id: RecordId,
    pub title: String,
    pub slug: String,
    pub synopsis: Option<String>,
    pub year: Option<i64>,
    pub duration_seconds: Option<i64>,
    pub genres: Vec<String>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub poster_url: Option<String>,
    pub trailer_url: Option<String>,
    pub status: String,
    pub content_declaration: Option<ContentDeclaration>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct ContentDeclaration {
    pub is_copyright_holder: Option<bool>,
    pub talent_cleared: Option<bool>,
    pub no_prohibited_content: Option<bool>,
    pub declared_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateFilm {
    pub title: String,
    pub slug: String,
    pub synopsis: Option<String>,
    pub year: Option<i64>,
    pub duration_seconds: Option<i64>,
    pub genres: Vec<String>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub status: String,
    pub content_declaration: ContentDeclaration,
}

/// View model for film listings — uses String key for template rendering
/// since RecordIdKey doesn't implement Display.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilmView {
    pub id: RecordId,
    pub key_str: String,
    pub title: String,
    pub slug: String,
    pub synopsis: Option<String>,
    pub year: Option<i64>,
    pub genres: Vec<String>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub poster_url: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

impl From<Film> for FilmView {
    fn from(f: Film) -> Self {
        let key_str = crate::util::record_id_key_string(&f.id.key);
        Self {
            id: f.id,
            key_str,
            title: f.title,
            slug: f.slug,
            synopsis: f.synopsis,
            year: f.year,
            genres: f.genres,
            language: f.language,
            country: f.country,
            poster_url: f.poster_url,
            status: f.status,
            created_at: f.created_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Asset {
    pub id: RecordId,
    pub asset_type: String,
    pub codec: Option<String>,
    pub resolution: Option<String>,
    pub bitrate: Option<i64>,
    pub format: Option<String>,
    pub storage_key: String,
    pub size_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Edge data on the filmmaker_of relation.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct FilmmakerOf {
    pub id: RecordId,
    pub role: String,
    pub created_at: DateTime<Utc>,
}
