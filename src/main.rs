use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use pavilion::config::Config;
use pavilion::db;
use pavilion::router::{self, AppState};
use pavilion_media::config::StorageConfig;
use pavilion_media::storage::StorageClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env();
    init_logging(&config);

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        host = %config.host,
        port = config.port,
        "Starting Pavilion"
    );

    let db = db::connect(&config).await?;

    let storage = StorageClient::new(&StorageConfig {
        endpoint: config.rustfs_endpoint.clone(),
        access_key: config.rustfs_access_key.clone(),
        secret_key: config.rustfs_secret_key.clone(),
        bucket: config.rustfs_bucket.clone(),
        region: "us-east-1".into(),
        path_style: true,
    })?;
    tracing::info!(endpoint = %config.rustfs_endpoint, bucket = %config.rustfs_bucket, "Connected to RustFS");

    let app = router::build_router(AppState {
        db,
        config: config.clone(),
        storage,
    });

    let listener = tokio::net::TcpListener::bind(config.bind_addr()).await?;
    tracing::info!(addr = %config.bind_addr(), "Listening");
    axum::serve(listener, app).await?;

    Ok(())
}

fn init_logging(config: &Config) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("pavilion=debug,tower_http=debug"));

    if config.pretty_logs {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().pretty())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().json())
            .init();
    }
}
