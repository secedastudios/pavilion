mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use pavilion::router::{self, AppState};

// ── Helpers ────────────────────────────────────────────────

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
        urlencoding(email)
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
    resp.headers().get("set-cookie").unwrap().to_str().unwrap()
        .split(';').next().unwrap().to_string()
}

fn urlencoding(s: &str) -> String {
    s.replace('@', "%40")
}

async fn create_published_film_with_license(app: &mut axum::Router, cookie: &str) -> String {
    // Create film
    let body = "title=Catalog+Film&synopsis=A+great+film&year=2026&genres=Drama\
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
    let film_url = resp.headers().get("location").unwrap().to_str().unwrap().to_string();
    let film_id = film_url.strip_prefix("/films/").unwrap().to_string();

    // Publish
    app.clone()
        .oneshot(
            Request::post(&format!("/films/{film_id}/status"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", cookie)
                .body(Body::from("status=published"))
                .unwrap(),
        )
        .await
        .unwrap();

    // Add AVOD license (no approval required)
    let license_body = "license_type=avod&revenue_share_pct=60";
    app.clone()
        .oneshot(
            Request::post(&format!("/films/{film_id}/licenses"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", cookie)
                .body(Body::from(license_body))
                .unwrap(),
        )
        .await
        .unwrap();

    film_id
}

// ── Tests ──────────────────────────────────────────────────

#[tokio::test]
async fn catalog_page_loads() {
    let app = build_app().await;

    let resp = app
        .oneshot(Request::get("/catalog").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Film Catalog"));
}

#[tokio::test]
async fn catalog_shows_published_films_with_licenses() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "filmmaker@test.com").await;
    create_published_film_with_license(&mut app, &cookie).await;

    let resp = app
        .oneshot(Request::get("/catalog").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Catalog Film"));
}

#[tokio::test]
async fn catalog_hides_draft_films() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "filmmaker@test.com").await;

    // Create film but don't publish
    let body = "title=Draft+Film&synopsis=Hidden&year=2026&genres=Drama\
        &language=English&country=UK\
        &declare_copyright=yes&declare_talent=yes&declare_no_prohibited=yes";
    app.clone()
        .oneshot(
            Request::post("/films")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let resp = app
        .oneshot(Request::get("/catalog").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let html = body_string(resp).await;
    assert!(!html.contains("Draft Film"));
}

#[tokio::test]
async fn catalog_film_detail_shows_licenses() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "filmmaker@test.com").await;
    let film_id = create_published_film_with_license(&mut app, &cookie).await;

    let resp = app
        .oneshot(
            Request::get(&format!("/catalog/{film_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Catalog Film"));
    assert!(html.contains("Ad-Supported (AVOD)"));
}

#[tokio::test]
async fn acquire_license_without_approval() {
    let mut app = build_app().await;

    // Filmmaker creates film
    let filmmaker_cookie = register_person(&mut app, "filmmaker@test.com").await;
    let film_id = create_published_film_with_license(&mut app, &filmmaker_cookie).await;

    // Register curator first, then view catalog detail with their cookie
    let curator_cookie = register_person(&mut app, "curator@test.com").await;

    let resp = app
        .clone()
        .oneshot(
            Request::get(&format!("/catalog/{film_id}"))
                .header("cookie", &curator_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = body_string(resp).await;

    // Extract license_id from hidden form field
    let license_id = html
        .split("name=\"license_id\" value=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap();
    let resp = app
        .oneshot(
            Request::post(&format!("/catalog/{film_id}/acquire"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &curator_cookie)
                .body(Body::from(format!("license_id={license_id}")))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("License acquired"));
}

#[tokio::test]
async fn acquire_requires_auth() {
    let mut app = build_app().await;

    let resp = app
        .oneshot(
            Request::post("/catalog/fake/acquire")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("license_id=fake"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn filmmaker_sees_requests() {
    let mut app = build_app().await;
    let filmmaker_cookie = register_person(&mut app, "filmmaker@test.com").await;
    let film_id = create_published_film_with_license(&mut app, &filmmaker_cookie).await;

    let resp = app
        .oneshot(
            Request::get(&format!("/films/{film_id}/requests"))
                .header("cookie", &filmmaker_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("License requests"));
}
