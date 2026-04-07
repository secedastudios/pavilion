mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use surrealdb::types::RecordId;
use tower::ServiceExt;

use pavilion::revenue::splits;
use pavilion::revenue::stats;
use pavilion::router::{self, AppState};

async fn build_app() -> axum::Router {
    let db = common::setup_test_db().await;
    let config = common::test_config();
    router::build_router(AppState { db, config, storage: common::test_storage() })
}

async fn body_string(response: axum::http::Response<Body>) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn register_person(app: &mut axum::Router, email: &str) -> String {
    let body = format!(
        "email={}&name=Test&password=password123&password_confirm=password123\
         &accept_terms=yes&accept_no_porn=yes&accept_copyright=yes&accept_talent=yes",
        email.replace('@', "%40")
    );
    let resp = app.clone().oneshot(
        Request::post("/register")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body)).unwrap(),
    ).await.unwrap();
    resp.headers().get("set-cookie").unwrap().to_str().unwrap()
        .split(';').next().unwrap().to_string()
}

// ── Transaction recording ──────────────────────────────────

#[tokio::test]
async fn record_transaction_with_splits() {
    let db = common::setup_test_db().await;

    let platform_id = RecordId::new("platform", "plat1");
    let filmmaker_id = RecordId::new("person", "filmmaker1");
    let buyer_id = RecordId::new("person", "buyer1");

    let txn = splits::record_transaction(
        &db,
        "purchase",
        999, // $9.99
        "usd",
        Some(RecordId::new("film", "film1")),
        platform_id,
        Some(buyer_id),
        Some("ch_test123".into()),
        5.0,  // 5% facilitation fee
        Some(filmmaker_id),
        Some(60.0), // 60% filmmaker share
    )
    .await
    .unwrap();

    assert_eq!(txn.transaction_type, "purchase");
    assert_eq!(txn.amount_cents, 999);
    assert_eq!(txn.status, "completed");

    // Fee: 999 * 5% = 50 (rounded)
    // After fee: 999 - 50 = 949
    // Filmmaker: 949 * 60% = 569 (rounded)
    // Curator: 949 - 569 = 380
}

#[tokio::test]
async fn record_transaction_zero_fee() {
    let db = common::setup_test_db().await;

    let txn = splits::record_transaction(
        &db,
        "rental",
        399,
        "usd",
        Some(RecordId::new("film", "film1")),
        RecordId::new("platform", "plat1"),
        None,
        None,
        0.0, // No facilitation fee (self-hosted)
        Some(RecordId::new("person", "filmmaker1")),
        Some(50.0),
    )
    .await
    .unwrap();

    assert_eq!(txn.amount_cents, 399);
}

// ── Stats queries ──────────────────────────────────────────

#[tokio::test]
async fn filmmaker_revenue_starts_at_zero() {
    let db = common::setup_test_db().await;
    let person_id = RecordId::new("person", "filmmaker1");

    let overview = stats::filmmaker_revenue(&db, &person_id).await.unwrap();
    assert_eq!(overview.total_earned_cents, 0);
    assert_eq!(overview.total_transactions, 0);
}

#[tokio::test]
async fn platform_revenue_starts_at_zero() {
    let db = common::setup_test_db().await;
    let platform_id = RecordId::new("platform", "plat1");

    let overview = stats::platform_revenue(&db, &platform_id).await.unwrap();
    assert_eq!(overview.total_revenue_cents, 0);
    assert_eq!(overview.subscriber_count, 0);
    assert_eq!(overview.total_views, 0);
}

// ── Integration: revenue dashboard ─────────────────────────

#[tokio::test]
async fn revenue_dashboard_requires_auth() {
    let app = build_app().await;

    let resp = app.oneshot(
        Request::get("/revenue").body(Body::empty()).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn revenue_dashboard_loads() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "filmmaker@test.com").await;

    let resp = app.oneshot(
        Request::get("/revenue")
            .header("cookie", &cookie)
            .body(Body::empty()).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Revenue"));
    assert!(html.contains("$0.00"));
}
