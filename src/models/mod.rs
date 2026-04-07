//! SurrealDB record structs, view models, and DTOs.
//!
//! Each model module follows a consistent pattern:
//!
//! - **Record struct** (e.g., `Film`) — maps directly to a SurrealDB table, derives `SurrealValue`
//! - **Create struct** (e.g., `CreateFilm`) — fields needed to create a new record
//! - **View struct** (e.g., `FilmView`) — safe for templates, includes `key_str` for URL rendering,
//!   never exposes sensitive fields like `password_hash`
//!
//! View structs exist because SurrealDB's `RecordIdKey` doesn't implement `Display`,
//! so we pre-compute a `key_str: String` for use in templates and URLs.
//!
//! # Common imports
//!
//! All model files need these SurrealDB types:
//! ```ignore
//! use surrealdb::types::{RecordId, SurrealValue};
//! ```

/// License acquisition requests from curators.
pub mod acquisition;

/// DMCA copyright claims and enforcement.
pub mod dmca;

/// Screenings, premieres, and Q&A events.
pub mod event;

/// Films with metadata, content declarations, and poster variants.
pub mod film;

/// Licensing terms (TVOD, SVOD, AVOD, event, educational, CC).
pub mod license;

/// Person accounts (filmmakers, curators, admins).
pub mod person;

/// White-label streaming platforms with theming.
pub mod platform;

/// Per-platform film ratings with cross-platform aggregation.
pub mod rating;

/// Transcode job queue records and profiles.
pub mod transcode;
