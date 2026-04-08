//! Environment-based application configuration.
//!
//! All settings are loaded from environment variables (via dotenv) with
//! sensible defaults for local development. See `.env-example` for the
//! complete list with descriptions.
//!
//! # Loading
//!
//! Call `Config::from_env()` once at startup. The returned struct is
//! cheap to clone and is shared via [`AppState`](crate::router::AppState).
//!
//! # Errors
//!
//! `from_env()` returns `anyhow::Result` so invalid config (e.g. non-numeric
//! PORT) produces a clean error message instead of a panic.

use std::env;

/// Default facilitation fee percentage for platform transactions.
const DEFAULT_FEE_PCT: f64 = 5.0;

/// Application configuration loaded from environment variables.
///
/// Created once at startup via `Config::from_env()` and shared across
/// all handlers through [`AppState`](crate::router::AppState).
///
/// # Fields
///
/// - **Database**: SurrealDB connection URL, namespace, database, and credentials.
/// - **Storage**: RustFS (S3-compatible) endpoint, credentials, and bucket.
/// - **Search**: Qdrant endpoint for vector similarity search.
/// - **Payments**: Optional Stripe Connect configuration. Leave `stripe_secret_key`
///   as `None` to disable all payment processing (useful for self-hosted instances).
/// - **Server**: Bind address, port, log format, and public base URL.
#[derive(Debug, Clone)]
pub struct Config {
    /// SurrealDB WebSocket URL (e.g., `ws://localhost:8001`).
    pub database_url: String,
    /// SurrealDB namespace.
    pub database_ns: String,
    /// SurrealDB database name.
    pub database_db: String,
    /// SurrealDB authentication username.
    pub database_user: String,
    /// SurrealDB authentication password.
    pub database_pass: String,
    /// Secret used to sign and verify JWT tokens. **Change in production.**
    pub jwt_secret: String,
    /// RustFS S3-compatible API endpoint.
    pub rustfs_endpoint: String,
    /// RustFS access key (equivalent to AWS_ACCESS_KEY_ID).
    pub rustfs_access_key: String,
    /// RustFS secret key (equivalent to AWS_SECRET_ACCESS_KEY).
    pub rustfs_secret_key: String,
    /// RustFS bucket name for all video and image storage.
    pub rustfs_bucket: String,
    /// Qdrant gRPC endpoint for vector search.
    pub qdrant_endpoint: String,
    /// Network interface to bind to (`0.0.0.0` for all interfaces).
    pub host: String,
    /// TCP port to listen on.
    pub port: u16,
    /// Use human-readable log output (`true`) or structured JSON (`false`).
    pub pretty_logs: bool,
    /// Public-facing base URL used for OAuth redirects and email links.
    pub base_url: String,
    /// Stripe secret key. `None` disables all payment features.
    pub stripe_secret_key: Option<String>,
    /// Stripe publishable key for client-side checkout.
    pub stripe_publishable_key: Option<String>,
    /// Stripe webhook signing secret for verifying incoming webhooks.
    pub stripe_webhook_secret: Option<String>,
    /// Percentage of each transaction kept as a platform facilitation fee.
    /// Set to `0.0` on self-hosted instances to disable.
    pub facilitation_fee_pct: f64,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// Reads `.env` if present (via dotenv), then falls back to sensible
    /// defaults for every variable. Returns an error only if a value is
    /// present but unparsable (e.g., `PORT=abc`).
    ///
    /// # Errors
    ///
    /// Returns `anyhow::Error` if `PORT` or `FACILITATION_FEE_PCT` contain
    /// non-numeric values.
    pub fn from_env() -> anyhow::Result<Self> {
        dotenv::dotenv().ok();

        let port: u16 = env_or("PORT", "3000")
            .parse()
            .map_err(|_| anyhow::anyhow!("PORT must be a valid number"))?;

        let facilitation_fee_pct: f64 = env_or("FACILITATION_FEE_PCT", "5.0")
            .parse()
            .unwrap_or(DEFAULT_FEE_PCT);

        Ok(Self {
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
            port,
            pretty_logs: env_or("PRETTY_LOGS", "true").parse().unwrap_or(true),
            base_url: env_or("BASE_URL", "http://localhost:3000"),
            stripe_secret_key: optional_env("STRIPE_SECRET_KEY"),
            stripe_publishable_key: optional_env("STRIPE_PUBLISHABLE_KEY"),
            stripe_webhook_secret: optional_env("STRIPE_WEBHOOK_SECRET"),
            facilitation_fee_pct,
        })
    }

    /// The `host:port` string for binding the TCP listener.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Whether Stripe payment processing is configured and enabled.
    pub fn payments_enabled(&self) -> bool {
        self.stripe_secret_key.is_some()
    }
}

/// Read an environment variable, returning `default` if unset.
fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Read an optional environment variable. Returns `None` if unset or empty.
fn optional_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.is_empty())
}
