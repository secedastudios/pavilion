//! Environment-based application configuration.
//!
//! All settings are loaded from environment variables (via dotenv) with
//! sensible defaults for local development. See `.env-example` for the
//! full list.

use std::env;

/// Application configuration loaded from environment variables.
///
/// Created once at startup via [`Config::from_env()`] and shared across
/// all handlers through [`AppState`](crate::router::AppState).
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub database_ns: String,
    pub database_db: String,
    pub database_user: String,
    pub database_pass: String,
    pub jwt_secret: String,
    pub rustfs_endpoint: String,
    pub rustfs_access_key: String,
    pub rustfs_secret_key: String,
    pub rustfs_bucket: String,
    pub qdrant_endpoint: String,
    pub host: String,
    pub port: u16,
    pub pretty_logs: bool,
    pub base_url: String,
    // Payments — leave STRIPE_SECRET_KEY empty to disable payments entirely
    pub stripe_secret_key: Option<String>,
    pub stripe_publishable_key: Option<String>,
    pub stripe_webhook_secret: Option<String>,
    pub facilitation_fee_pct: f64,
}

impl Config {
    pub fn from_env() -> Self {
        dotenv::dotenv().ok();

        Self {
            database_url: env_or("DATABASE_URL", "ws://localhost:8001"),
            database_ns: env_or("DATABASE_NS", "pavilion"),
            database_db: env_or("DATABASE_DB", "pavilion"),
            database_user: env_or("DATABASE_USER", "root"),
            database_pass: env_or("DATABASE_PASS", "root"),
            jwt_secret: env_or("JWT_SECRET", "change-me-in-production"),
            rustfs_endpoint: env_or("RUSTFS_ENDPOINT", "http://localhost:9002"),
            rustfs_access_key: env_or("RUSTFS_ACCESS_KEY", "rustfsadmin"),
            rustfs_secret_key: env_or("RUSTFS_SECRET_KEY", "rustfsadmin"),
            rustfs_bucket: env_or("RUSTFS_BUCKET", "pavilion"),
            qdrant_endpoint: env_or("QDRANT_ENDPOINT", "http://localhost:6336"),
            host: env_or("HOST", "0.0.0.0"),
            port: env_or("PORT", "3000").parse().expect("PORT must be a number"),
            pretty_logs: env_or("PRETTY_LOGS", "true").parse().unwrap_or(true),
            base_url: env_or("BASE_URL", "http://localhost:3000"),
            stripe_secret_key: env::var("STRIPE_SECRET_KEY").ok().filter(|s| !s.is_empty()),
            stripe_publishable_key: env::var("STRIPE_PUBLISHABLE_KEY").ok().filter(|s| !s.is_empty()),
            stripe_webhook_secret: env::var("STRIPE_WEBHOOK_SECRET").ok().filter(|s| !s.is_empty()),
            facilitation_fee_pct: env_or("FACILITATION_FEE_PCT", "5.0").parse().unwrap_or(5.0),
        }
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn payments_enabled(&self) -> bool {
        self.stripe_secret_key.is_some()
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}
