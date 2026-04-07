//! Shared test utilities for Pavilion integration tests.
//!
//! Every test file should import `mod common;` and use these helpers
//! instead of redefining them.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use pavilion::config::Config;
use pavilion::db::Db;
use pavilion::router::{self, AppState};
use pavilion_media::config::StorageConfig;
use pavilion_media::storage::StorageClient;

/// Connect to an in-memory SurrealDB and apply the full schema.
pub async fn setup_test_db() -> Db {
    let db = surrealdb::engine::any::connect("mem://")
        .await
        .expect("Failed to connect to in-memory SurrealDB");

    db.use_ns("test")
        .use_db("test")
        .await
        .expect("Failed to set namespace/database");

    let schema = include_str!("../../db/schema.surql");
    db.query(schema).await.expect("Failed to apply schema");

    db
}

/// Build a test Pavilion Config with safe defaults (no real services).
pub fn test_config() -> Config {
    Config {
        database_url: "mem://".to_string(),
        database_ns: "test".to_string(),
        database_db: "test".to_string(),
        database_user: "root".to_string(),
        database_pass: "root".to_string(),
        jwt_secret: "test-secret".to_string(),
        rustfs_endpoint: "http://localhost:9999".to_string(),
        rustfs_access_key: "test".to_string(),
        rustfs_secret_key: "test".to_string(),
        rustfs_bucket: "test".to_string(),
        qdrant_endpoint: "http://localhost:6336".to_string(),
        host: "127.0.0.1".to_string(),
        port: 0,
        pretty_logs: false,
        base_url: "http://localhost:3000".to_string(),
        stripe_secret_key: None,
        stripe_publishable_key: None,
        stripe_webhook_secret: None,
        facilitation_fee_pct: 0.0,
    }
}

/// Create a StorageClient pointing at a non-existent endpoint (tests don't hit real storage).
pub fn test_storage() -> StorageClient {
    StorageClient::new(&StorageConfig {
        endpoint: "http://localhost:9999".into(),
        access_key: "test".into(),
        secret_key: "test".into(),
        bucket: "test".into(),
        region: "us-east-1".into(),
        path_style: true,
    })
    .expect("Failed to create test storage client")
}

/// Build a full Pavilion Router with in-memory DB and test config.
pub async fn build_app() -> axum::Router {
    let db = setup_test_db().await;
    let config = test_config();
    router::build_router(AppState {
        db,
        config,
        storage: test_storage(),
    })
}

/// Extract the body of an HTTP response as a String.
pub async fn body_string(response: axum::http::Response<Body>) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// Register a test person and return the auth cookie string.
///
/// Accepts all terms, uses "password123" as password.
pub async fn register_person(app: &mut axum::Router, email: &str) -> String {
    let body = format!(
        "email={}&name=Test&password=password123&password_confirm=password123\
         &accept_terms=yes&accept_no_porn=yes&accept_copyright=yes&accept_talent=yes",
        email.replace('@', "%40")
    );
    let resp = app
        .clone()
        .oneshot(
            Request::post("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "Registration failed for {email}"
    );

    resp.headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

/// Create a film with content declarations and return the film ID.
pub async fn create_test_film(app: &mut axum::Router, cookie: &str) -> String {
    let body = "title=Test+Film&synopsis=A+test+film&year=2026&genres=Drama\
        &language=English&country=UK\
        &declare_copyright=yes&declare_talent=yes&declare_no_prohibited=yes";
    let resp = app
        .clone()
        .oneshot(
            Request::post("/films")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "Film creation failed");

    resp.headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("/films/")
        .unwrap()
        .to_string()
}

/// Publish a film (changes status from draft to published).
pub async fn publish_film(app: &mut axum::Router, cookie: &str, film_id: &str) {
    let resp = app
        .clone()
        .oneshot(
            Request::post(&format!("/films/{film_id}/status"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", cookie)
                .body(Body::from("status=published"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "Film publish failed for {film_id}"
    );
}

/// Create a platform and return the platform ID.
pub async fn create_test_platform(app: &mut axum::Router, cookie: &str, name: &str) -> String {
    let body = format!("name={}", name.replace(' ', "+"));
    let resp = app
        .clone()
        .oneshot(
            Request::post("/platforms")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "Platform creation failed"
    );

    resp.headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("/platforms/")
        .unwrap()
        .to_string()
}

/// Activate a platform (changes status from setup to active).
pub async fn activate_platform(app: &mut axum::Router, cookie: &str, platform_id: &str) {
    app.clone()
        .oneshot(
            Request::post(&format!("/platforms/{platform_id}/activate"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
}
