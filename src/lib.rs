//! # Pavilion
//!
//! Open-source film distribution and white-label OTT platform.
//!
//! Pavilion gives independent filmmakers control over their work while
//! letting curators, festivals, and entrepreneurs launch branded streaming
//! services. Built with Rust, Axum, SurrealDB, and Datastar.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │              Pavilion (Axum)             │
//! │                                         │
//! │  auth ─── controllers ─── templates     │
//! │  models ── licensing ──── delivery       │
//! │  payments─ revenue ────── billing        │
//! │  media ─── transcode ──── middleware     │
//! └────────┬─────────────┬──────────────────┘
//!          │             │
//!    ┌─────┴─────┐ ┌────┴─────┐
//!    │ SurrealDB  │ │  RustFS  │
//!    └───────────┘ └──────────┘
//! ```
//!
//! ## Key modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`auth`] | JWT tokens, Argon2 passwords, request extractors |
//! | [`controllers`] | Axum route handlers (films, platforms, player, etc.) |
//! | [`models`] | SurrealDB record structs with view models |
//! | [`delivery`] | Signed tokens and manifest rewriting for secure streaming |
//! | [`licensing`] | Rights resolution engine (territory, window, approval) |
//! | [`payments`] | Stripe Connect, entitlements, payment provider trait |
//! | [`revenue`] | Transaction recording, splits, dashboards |
//! | [`billing`] | Storage metering, pricing tiers, curator credits |
//! | [`transcode`] | FFmpeg job queue, worker, reaper |
//! | [`media`] | Image processing, TMDB enrichment, presigned uploads |
//! | [`router`] | Route definitions and AppState |
//! | [`sse`] | Datastar SSE fragment helpers |

/// Authentication: JWT tokens, Argon2 password hashing, and Axum extractors.
pub mod auth;

/// Storage metering, pricing tiers, and curator credits.
pub mod billing;

/// Environment-based configuration loaded from `.env` via dotenv.
pub mod config;

/// Axum route handlers organized by feature area.
pub mod controllers;

/// SurrealDB connection and initialization.
pub mod db;

/// Signed token generation and manifest rewriting for secure video delivery.
/// Re-exports from the `pavilion-media` crate.
pub mod delivery;

/// Application error types with HTTP status code mapping.
pub mod error;

/// Rights resolution engine — determines which films are available
/// based on territory, time windows, and license status.
pub mod licensing;

/// Image processing, TMDB/IMDB metadata enrichment, presigned upload config.
pub mod media;

/// Security headers middleware (CSP, X-Frame-Options, nosniff).
pub mod middleware;

/// SurrealDB record structs, view models, and create/update DTOs.
pub mod models;

/// Stripe Connect payment provider, entitlements, and viewer subscriptions.
pub mod payments;

/// Transaction recording, revenue splits, filmmaker and curator dashboards.
pub mod revenue;

/// Axum router with all route definitions and shared application state.
pub mod router;

/// Datastar SSE fragment helpers for hypermedia-driven reactivity.
pub mod sse;

/// Askama template rendering helpers for Axum responses.
pub mod templates;

/// FFmpeg transcoding job queue, worker, stale job reaper.
/// Re-exports FFmpeg and manifest modules from `pavilion-media`.
pub mod transcode;

/// Shared utilities: RecordId key extraction, relation verification,
/// slugification, validation constants.
pub mod util;
