mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use surrealdb::types::RecordId;
use tower::ServiceExt;

use pavilion::payments::entitlements;
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

// ── Entitlement unit tests ─────────────────────────────────

#[tokio::test]
async fn free_license_grants_access_without_entitlement() {
    let db = common::setup_test_db().await;
    let person = RecordId::new("person", "viewer1");
    let film = RecordId::new("film", "film1");
    let platform = RecordId::new("platform", "plat1");

    let result = entitlements::check_entitlement(&db, &person, &film, &platform, "avod").await.unwrap();
    assert_eq!(result, Some("free_access".to_string()));
}

#[tokio::test]
async fn cc_license_grants_access_without_entitlement() {
    let db = common::setup_test_db().await;
    let person = RecordId::new("person", "viewer1");
    let film = RecordId::new("film", "film1");
    let platform = RecordId::new("platform", "plat1");

    let result = entitlements::check_entitlement(&db, &person, &film, &platform, "cc").await.unwrap();
    assert_eq!(result, Some("free_access".to_string()));
}

#[tokio::test]
async fn tvod_requires_entitlement() {
    let db = common::setup_test_db().await;
    let person = RecordId::new("person", "viewer1");
    let film = RecordId::new("film", "film1");
    let platform = RecordId::new("platform", "plat1");

    let result = entitlements::check_entitlement(&db, &person, &film, &platform, "tvod").await.unwrap();
    assert_eq!(result, None); // No entitlement = no access
}

#[tokio::test]
async fn purchase_entitlement_grants_access() {
    let db = common::setup_test_db().await;
    let person = RecordId::new("person", "viewer1");
    let film = RecordId::new("film", "film1");
    let platform = RecordId::new("platform", "plat1");

    entitlements::grant_entitlement(
        &db, person.clone(), film.clone(), platform.clone(),
        "purchase", None, None,
    ).await.unwrap();

    let result = entitlements::check_entitlement(&db, &person, &film, &platform, "tvod").await.unwrap();
    assert_eq!(result, Some("purchase".to_string()));
}

#[tokio::test]
async fn rental_entitlement_with_future_expiry_grants_access() {
    let db = common::setup_test_db().await;
    let person = RecordId::new("person", "viewer1");
    let film = RecordId::new("film", "film1");
    let platform = RecordId::new("platform", "plat1");

    let expires = chrono::Utc::now() + chrono::Duration::hours(48);
    entitlements::grant_entitlement(
        &db, person.clone(), film.clone(), platform.clone(),
        "rental", Some(expires), None,
    ).await.unwrap();

    let result = entitlements::check_entitlement(&db, &person, &film, &platform, "tvod").await.unwrap();
    assert_eq!(result, Some("rental".to_string()));
}

// ── Integration: payments disabled ─────────────────────────

#[tokio::test]
async fn payment_settings_without_stripe_shows_disabled() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "curator@test.com").await;

    // Create platform
    let resp = app.clone().oneshot(
        Request::post("/platforms")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cookie)
            .body(Body::from("name=Pay+Channel")).unwrap(),
    ).await.unwrap();
    let plat_id = resp.headers().get("location").unwrap().to_str().unwrap()
        .strip_prefix("/platforms/").unwrap().to_string();

    let resp = app.oneshot(
        Request::get(&format!("/platforms/{plat_id}/payments"))
            .header("cookie", &cookie)
            .body(Body::empty()).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("not configured"));
}

#[tokio::test]
async fn checkout_without_stripe_shows_disabled() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "viewer@test.com").await;

    let resp = app.oneshot(
        Request::post("/p/any-platform/checkout")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cookie)
            .body(Body::from("film_id=test&checkout_type=rental&amount_cents=399")).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("not available"));
}

#[tokio::test]
async fn webhook_without_signature_fails() {
    let app = build_app().await;

    let resp = app.oneshot(
        Request::post("/webhooks/stripe")
            .header("content-type", "application/json")
            .body(Body::from("{}")).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
