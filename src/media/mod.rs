//! Image processing, TMDB metadata enrichment, and presigned uploads.
//! Handles poster resizing to multiple variants, fetches film metadata
//! from The Movie Database API, and configures direct-to-storage uploads.

pub mod enrichment;
pub mod images;
pub mod presigned;
