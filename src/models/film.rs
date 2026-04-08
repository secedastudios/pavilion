use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

/// A film (short, feature, documentary, etc.) submitted to the platform.
///
/// Films progress through a lifecycle tracked by `status`:
/// `"draft"` -> `"submitted"` -> `"published"` (or `"rejected"`).
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Film {
    pub id: RecordId,
    pub title: String,
    /// URL-safe identifier used in routes (e.g. `/films/my-film-title`).
    pub slug: String,
    pub synopsis: Option<String>,
    /// Production or release year.
    pub year: Option<i64>,
    /// Runtime in seconds; displayed as HH:MM in templates.
    pub duration_seconds: Option<i64>,
    pub genres: Vec<String>,
    /// Primary language (ISO 639-1 code or display name).
    pub language: Option<String>,
    /// Country of origin (ISO 3166-1 or display name).
    pub country: Option<String>,
    pub poster_url: Option<String>,
    pub trailer_url: Option<String>,
    /// Lifecycle status: `"draft"`, `"submitted"`, `"published"`, or `"rejected"`.
    pub status: String,
    /// Filmmaker's legal declarations about the content.
    pub content_declaration: Option<ContentDeclaration>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Legal declarations a filmmaker makes when submitting a [`Film`].
///
/// All fields are `None` until the filmmaker completes the declaration form.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct ContentDeclaration {
    /// Filmmaker asserts they own or control the copyright.
    pub is_copyright_holder: Option<bool>,
    /// All on-screen talent has signed releases.
    pub talent_cleared: Option<bool>,
    /// Content does not contain prohibited material.
    pub no_prohibited_content: Option<bool>,
    /// Timestamp when the declaration was signed.
    pub declared_at: Option<DateTime<Utc>>,
}

/// Payload for creating a new [`Film`] record.
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

/// Template-safe projection of [`Film`] for listings and detail pages.
///
/// Includes a `key_str` field because `RecordIdKey` does not implement `Display`,
/// so templates need a plain string for URL construction.
/// Excludes `trailer_url`, `content_declaration`, and `updated_at`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilmView {
    pub id: RecordId,
    /// String representation of the record key for use in URLs and templates.
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

/// A media asset (video file, poster image, subtitle track, etc.) linked to a [`Film`].
///
/// Assets are created by the transcode pipeline or uploaded directly by the filmmaker.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Asset {
    pub id: RecordId,
    /// Kind of asset: `"video"`, `"poster"`, `"subtitle"`, `"trailer"`, etc.
    pub asset_type: String,
    /// Video/audio codec, e.g. `"h264"`, `"aac"`.
    pub codec: Option<String>,
    /// Resolution label, e.g. `"1080p"`, `"720p"`.
    pub resolution: Option<String>,
    /// Bitrate in bits per second.
    pub bitrate: Option<i64>,
    /// Container format, e.g. `"cmaf"`, `"mp4"`.
    pub format: Option<String>,
    /// Object storage key (S3 path) for retrieving the file.
    pub storage_key: String,
    /// File size in bytes.
    pub size_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Edge data on the `filmmaker_of` graph relation (Person -> Film).
///
/// Represents a person's credited role on a film (e.g. director, producer, editor).
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct FilmmakerOf {
    pub id: RecordId,
    /// Credit role, e.g. `"director"`, `"producer"`, `"editor"`.
    pub role: String,
    pub created_at: DateTime<Utc>,
}
