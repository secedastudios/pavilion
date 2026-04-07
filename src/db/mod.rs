//! SurrealDB connection management.
//!
//! Uses `surrealdb::engine::any::Any` as the engine type so the same code
//! works across connection modes:
//!
//! - `ws://` — WebSocket (production, connects to running SurrealDB)
//! - `mem://` — In-memory (tests, no external dependency)
//! - `file://` — File-based (single-node dev)

use surrealdb::Surreal;
use surrealdb::engine::any::Any;

use crate::config::Config;

/// Type alias for the SurrealDB client. Used throughout the app as the DB handle.
pub type Db = Surreal<Any>;

/// Connect to SurrealDB, authenticate, and select the namespace/database.
///
/// # Errors
///
/// Returns `surrealdb::Error` if connection, authentication, or namespace
/// selection fails.
pub async fn connect(config: &Config) -> Result<Db, surrealdb::Error> {
    let db = surrealdb::engine::any::connect(&config.database_url).await?;

    db.signin(surrealdb::opt::auth::Root {
        username: config.database_user.clone(),
        password: config.database_pass.clone(),
    })
    .await?;

    db.use_ns(&config.database_ns)
        .use_db(&config.database_db)
        .await?;

    tracing::info!(
        url = %config.database_url,
        ns = %config.database_ns,
        db = %config.database_db,
        "Connected to SurrealDB"
    );

    Ok(db)
}
