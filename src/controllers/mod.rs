//! Axum route handlers organized by feature area.
//!
//! Each module contains:
//! - Askama template structs (private)
//! - Form deserialization structs (public for testing)
//! - Async handler functions wired to routes in [`crate::router`]
//!
//! All handlers that modify resources verify ownership through graph
//! traversal (e.g., `person -[filmmaker_of]-> film`).

/// License acquisition: curators request/acquire film licenses.
pub mod acquisitions;

/// Admin dashboard: system overview, person management, GDPR tools.
pub mod admin;

/// Registration, login, logout, SlateHub OAuth stubs.
pub mod auth;

/// Storage metering, pricing tier display, credit balance.
pub mod billing;

/// Public film catalog: browse, search, filter, film detail.
pub mod catalog;

/// DMCA takedown form (public), claim workflow, counter-notifications.
pub mod dmca;

/// TMDB and IMDB metadata enrichment for films.
pub mod enrichment;

/// Event CRUD, ticketing, status management.
pub mod events;

/// Film CRUD: create, edit, status workflow, ownership-verified.
pub mod films;

/// Landing page with marketing copy.
pub mod home;

/// Static legal pages: terms, privacy policy, content policy.
pub mod legal;

/// License management: create, edit, deactivate per film.
pub mod licenses;

/// Stripe Connect onboarding, viewer checkout, webhook handler.
pub mod payments;

/// Platform CRUD, theme engine, content management, public rendering.
pub mod platforms;

/// Video player page, manifest proxy, segment proxy, playhead tracking.
pub mod player;

/// Per-platform film ratings with curator moderation.
pub mod ratings;

/// Filmmaker revenue dashboard, curator platform analytics.
pub mod revenue;

/// Person profile: view, inline edit via Datastar SSE.
pub mod profile;

/// GDPR settings: consent management, data export, account deletion.
pub mod settings;

/// Pavilion Showcase: reference streaming site implementation.
pub mod showcase;

/// Transcode job management: enqueue, progress polling.
pub mod transcode;

/// Film and poster file upload endpoints.
pub mod upload;
