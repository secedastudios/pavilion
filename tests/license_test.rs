mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use pavilion::models::license::{CreateLicense, validate_license};
use pavilion::router::{self, AppState};

// ── Validation unit tests ──────────────────────────────────

#[test]
fn tvod_requires_rental_or_purchase_price() {
    let license = CreateLicense {
        license_type: "tvod".into(),
        territories: vec![],
        window_start: None,
        window_end: None,
        approval_required: false,
        active: true,
        rental_price_cents: None,
        rental_duration_hours: None,
        purchase_price_cents: None,
        flat_fee_monthly_cents: None,
        revenue_share_pct: None,
        event_flat_fee_cents: None,
        ticket_split_pct: None,
        max_attendees: None,
        institution_types: None,
        pricing_tier: None,
        cc_license_type: None,
    };
    assert!(validate_license(&license).is_err());
}

#[test]
fn tvod_rental_requires_duration() {
    let license = CreateLicense {
        license_type: "tvod".into(),
        territories: vec![],
        window_start: None,
        window_end: None,
        approval_required: false,
        active: true,
        rental_price_cents: Some(399),
        rental_duration_hours: None,
        purchase_price_cents: None,
        flat_fee_monthly_cents: None,
        revenue_share_pct: None,
        event_flat_fee_cents: None,
        ticket_split_pct: None,
        max_attendees: None,
        institution_types: None,
        pricing_tier: None,
        cc_license_type: None,
    };
    assert!(validate_license(&license).is_err());
}

#[test]
fn tvod_valid_with_rental_and_duration() {
    let license = CreateLicense {
        license_type: "tvod".into(),
        territories: vec!["US".into()],
        window_start: None,
        window_end: None,
        approval_required: false,
        active: true,
        rental_price_cents: Some(399),
        rental_duration_hours: Some(48),
        purchase_price_cents: None,
        flat_fee_monthly_cents: None,
        revenue_share_pct: None,
        event_flat_fee_cents: None,
        ticket_split_pct: None,
        max_attendees: None,
        institution_types: None,
        pricing_tier: None,
        cc_license_type: None,
    };
    assert!(validate_license(&license).is_ok());
}

#[test]
fn svod_requires_fee_or_share() {
    let license = CreateLicense {
        license_type: "svod".into(),
        territories: vec![],
        window_start: None,
        window_end: None,
        approval_required: false,
        active: true,
        rental_price_cents: None,
        rental_duration_hours: None,
        purchase_price_cents: None,
        flat_fee_monthly_cents: None,
        revenue_share_pct: None,
        event_flat_fee_cents: None,
        ticket_split_pct: None,
        max_attendees: None,
        institution_types: None,
        pricing_tier: None,
        cc_license_type: None,
    };
    assert!(validate_license(&license).is_err());
}

#[test]
fn avod_requires_revenue_share() {
    let mut license = CreateLicense {
        license_type: "avod".into(),
        territories: vec![],
        window_start: None,
        window_end: None,
        approval_required: false,
        active: true,
        rental_price_cents: None,
        rental_duration_hours: None,
        purchase_price_cents: None,
        flat_fee_monthly_cents: None,
        revenue_share_pct: None,
        event_flat_fee_cents: None,
        ticket_split_pct: None,
        max_attendees: None,
        institution_types: None,
        pricing_tier: None,
        cc_license_type: None,
    };
    assert!(validate_license(&license).is_err());
    license.revenue_share_pct = Some(60.0);
    assert!(validate_license(&license).is_ok());
}

#[test]
fn cc_requires_license_type() {
    let mut license = CreateLicense {
        license_type: "cc".into(),
        territories: vec![],
        window_start: None,
        window_end: None,
        approval_required: false,
        active: true,
        rental_price_cents: None,
        rental_duration_hours: None,
        purchase_price_cents: None,
        flat_fee_monthly_cents: None,
        revenue_share_pct: None,
        event_flat_fee_cents: None,
        ticket_split_pct: None,
        max_attendees: None,
        institution_types: None,
        pricing_tier: None,
        cc_license_type: None,
    };
    assert!(validate_license(&license).is_err());
    license.cc_license_type = Some("CC-BY".into());
    assert!(validate_license(&license).is_ok());
}

#[test]
fn revenue_share_must_be_0_to_100() {
    let license = CreateLicense {
        license_type: "avod".into(),
        territories: vec![],
        window_start: None,
        window_end: None,
        approval_required: false,
        active: true,
        rental_price_cents: None,
        rental_duration_hours: None,
        purchase_price_cents: None,
        flat_fee_monthly_cents: None,
        revenue_share_pct: Some(150.0),
        event_flat_fee_cents: None,
        ticket_split_pct: None,
        max_attendees: None,
        institution_types: None,
        pricing_tier: None,
        cc_license_type: None,
    };
    assert!(validate_license(&license).is_err());
}

// ── Integration tests ──────────────────────────────────────

async fn build_app() -> axum::Router {
    let db = common::setup_test_db().await;
    let config = common::test_config();
    router::build_router(AppState { db, config, storage: common::test_storage() })
}

async fn body_string(response: axum::http::Response<Body>) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn register_and_create_film(app: &mut axum::Router) -> (String, String) {
    // Register
    let reg_body = "email=filmmaker%40test.com&name=Test&password=password123&password_confirm=password123\
        &accept_terms=yes&accept_no_porn=yes&accept_copyright=yes&accept_talent=yes";
    let resp = app
        .clone()
        .oneshot(
            Request::post("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(reg_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = resp.headers().get("set-cookie").unwrap().to_str().unwrap()
        .split(';').next().unwrap().to_string();

    // Create film
    let film_body = "title=Licensed+Film&synopsis=Test&year=2026&genres=Drama\
        &language=English&country=UK\
        &declare_copyright=yes&declare_talent=yes&declare_no_prohibited=yes";
    let resp = app
        .clone()
        .oneshot(
            Request::post("/films")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &cookie)
                .body(Body::from(film_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let film_url = resp.headers().get("location").unwrap().to_str().unwrap().to_string();
    let film_id = film_url.strip_prefix("/films/").unwrap().to_string();

    (cookie, film_id)
}

#[tokio::test]
async fn license_routes_require_auth() {
    let app = build_app().await;

    let resp = app
        .oneshot(Request::get("/films/fake/licenses").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_tvod_license() {
    let mut app = build_app().await;
    let (cookie, film_id) = register_and_create_film(&mut app).await;

    let body = format!(
        "license_type=tvod&territories=US%2C+GB&rental_price=3.99&rental_duration_hours=48&purchase_price=9.99"
    );
    let resp = app
        .clone()
        .oneshot(
            Request::post(&format!("/films/{film_id}/licenses"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // Verify license appears in list
    let resp = app
        .oneshot(
            Request::get(&format!("/films/{film_id}/licenses"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Transactional (TVOD)"));
    assert!(html.contains("US"));
}

#[tokio::test]
async fn create_license_with_invalid_type_fails_validation() {
    let mut app = build_app().await;
    let (cookie, film_id) = register_and_create_film(&mut app).await;

    // AVOD without revenue share
    let body = "license_type=avod";
    let resp = app
        .oneshot(
            Request::post(&format!("/films/{film_id}/licenses"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK); // Re-renders form with error
    let html = body_string(resp).await;
    assert!(html.contains("revenue share"));
}

#[tokio::test]
async fn deactivate_license() {
    let mut app = build_app().await;
    let (cookie, film_id) = register_and_create_film(&mut app).await;

    // Create a CC license
    let body = "license_type=cc&cc_license_type=CC-BY";
    let resp = app
        .clone()
        .oneshot(
            Request::post(&format!("/films/{film_id}/licenses"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // Get the license list to find the license ID
    let resp = app
        .clone()
        .oneshot(
            Request::get(&format!("/films/{film_id}/licenses"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = body_string(resp).await;

    // Extract license ID from the deactivate form action
    let deactivate_url = html
        .split("action=\"")
        .find(|s| s.contains("/deactivate"))
        .and_then(|s| s.split('"').next())
        .unwrap();

    let resp = app
        .oneshot(
            Request::post(deactivate_url)
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
}
