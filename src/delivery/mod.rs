//! Stream delivery including token authentication, manifest generation, and audit logging.
//! Re-exports token and manifest modules from `pavilion-media` and adds
//! access audit logging for tracking who watches what and when.

pub mod audit;

// Re-export from pavilion-media — these modules now live in the media crate
pub use pavilion_media::manifest;
pub use pavilion_media::token;
