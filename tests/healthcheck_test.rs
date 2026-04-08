mod common;

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;

use pavilion::router::{self, AppState};

#[tokio::test]
async fn healthcheck_returns_ok() {
    let db = common::setup_test_db().await;
    let config = common::test_config();
    let app = router::build_router(AppState {
        db,
        config,
        storage: common::test_storage(),
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthcheck")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["services"]["database"], "connected");
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let db = common::setup_test_db().await;
    let config = common::test_config();
    let app = router::build_router(AppState {
        db,
        config,
        storage: common::test_storage(),
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}
