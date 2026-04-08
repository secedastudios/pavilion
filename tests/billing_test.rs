mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use surrealdb::types::RecordId;
use tower::ServiceExt;

use pavilion::billing::{credits, metering, tiers};
use pavilion::router::{self, AppState};

async fn build_app() -> axum::Router {
    let db = common::setup_test_db().await;
    let config = common::test_config();
    router::build_router(AppState {
        db,
        config,
        storage: common::test_storage(),
    })
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

// ── Storage metering ───────────────────────────────────────

#[tokio::test]
async fn storage_starts_at_zero() {
    let db = common::setup_test_db().await;
    let person = RecordId::new("person", "filmmaker1");

    let usage = metering::get_usage(&db, &person).await.unwrap();
    assert_eq!(usage.total_bytes, 0);
    assert_eq!(usage.film_count, 0);
}

#[tokio::test]
async fn record_upload_increases_totals() {
    let db = common::setup_test_db().await;
    let person = RecordId::new("person", "filmmaker1");

    // Initialize
    let _ = metering::get_usage(&db, &person).await.unwrap();

    // Upload a master (1 GB)
    metering::record_upload(&db, &person, 1_073_741_824, true)
        .await
        .unwrap();

    let usage = metering::get_usage(&db, &person).await.unwrap();
    assert_eq!(usage.total_bytes, 1_073_741_824);
    assert_eq!(usage.master_bytes, 1_073_741_824);
    assert_eq!(usage.rendition_bytes, 0);
    assert_eq!(usage.asset_count, 1);

    // Upload a rendition (200 MB)
    metering::record_upload(&db, &person, 209_715_200, false)
        .await
        .unwrap();

    let usage = metering::get_usage(&db, &person).await.unwrap();
    assert_eq!(usage.total_bytes, 1_283_457_024);
    assert_eq!(usage.rendition_bytes, 209_715_200);
    assert_eq!(usage.asset_count, 2);
}

#[tokio::test]
async fn format_bytes_works() {
    assert_eq!(metering::format_bytes(500), "500 B");
    assert_eq!(metering::format_bytes(1536), "1.5 KB");
    assert_eq!(metering::format_bytes(10_485_760), "10.0 MB");
    assert_eq!(metering::format_bytes(1_073_741_824), "1.00 GB");
}

// ── Pricing tiers ──────────────────────────────────────────

#[test]
fn default_tiers_exist() {
    let t = tiers::default_tiers();
    assert_eq!(t.len(), 4);
    assert_eq!(t[0].name, "Free");
    assert_eq!(t[0].price_cents_monthly, 0);
}

// ── Credits ────────────────────────────────────────────────

#[tokio::test]
async fn credit_balance_starts_at_zero() {
    let db = common::setup_test_db().await;
    let person = RecordId::new("person", "curator1");

    let balance = credits::get_balance(&db, &person).await.unwrap();
    assert_eq!(balance, 0);
}

#[tokio::test]
async fn add_and_deduct_credits() {
    let db = common::setup_test_db().await;
    let person = RecordId::new("person", "curator1");

    // Initialize
    let _ = credits::get_balance(&db, &person).await.unwrap();

    let balance = credits::add_credits(&db, &person, 5000, "Purchased $50 credits")
        .await
        .unwrap();
    assert_eq!(balance, 5000);

    let balance = credits::deduct_credits(&db, &person, 2000, "Film acquisition")
        .await
        .unwrap();
    assert_eq!(balance, 3000);
}

#[tokio::test]
async fn deduct_fails_with_insufficient_balance() {
    let db = common::setup_test_db().await;
    let person = RecordId::new("person", "curator1");

    let _ = credits::get_balance(&db, &person).await.unwrap();
    let result = credits::deduct_credits(&db, &person, 1000, "Too much").await;
    assert!(result.is_err());
}

// ── Billing dashboard ──────────────────────────────────────

#[tokio::test]
async fn billing_requires_auth() {
    let app = build_app().await;

    let resp = app
        .oneshot(Request::get("/billing").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn billing_dashboard_loads() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "filmmaker@test.com").await;

    let resp = app
        .oneshot(
            Request::get("/billing")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Billing"));
    assert!(html.contains("Storage Usage"));
    assert!(html.contains("0 B"));
}
