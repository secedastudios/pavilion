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

async fn create_platform(app: &mut axum::Router, cookie: &str) -> String {
    let body = "name=Test+Channel&description=A+test+platform&monetization_model=subscription\
        &primary_color=%232563eb&dark_mode=yes";
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

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    location.strip_prefix("/platforms/").unwrap().to_string()
}

// ── Tests ──────────────────────────────────────────────────

#[tokio::test]
async fn platforms_require_auth() {
    let app = build_app().await;

    let resp = app
        .oneshot(Request::get("/platforms").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_platform_success() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "curator@test.com").await;

    let platform_id = create_platform(&mut app, &cookie).await;
    assert!(!platform_id.is_empty());

    // Dashboard should load
    let resp = app
        .oneshot(
            Request::get(&format!("/platforms/{platform_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Test Channel"));
    assert!(html.contains("setup")); // Initial status
}

#[tokio::test]
async fn platform_index_shows_owned() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "curator@test.com").await;
    create_platform(&mut app, &cookie).await;

    let resp = app
        .oneshot(
            Request::get("/platforms")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Test Channel"));
}

#[tokio::test]
async fn activate_platform() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "curator@test.com").await;
    let platform_id = create_platform(&mut app, &cookie).await;

    let resp = app
        .clone()
        .oneshot(
            Request::post(&format!("/platforms/{platform_id}/activate"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // Verify active
    let resp = app
        .oneshot(
            Request::get(&format!("/platforms/{platform_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let html = body_string(resp).await;
    assert!(html.contains("active"));
}

#[tokio::test]
async fn public_platform_requires_active() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "curator@test.com").await;
    create_platform(&mut app, &cookie).await;

    // Platform is in "setup" status, public page should 404
    let resp = app
        .oneshot(Request::get("/p/test-channel").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn public_platform_renders_when_active() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "curator@test.com").await;
    let platform_id = create_platform(&mut app, &cookie).await;

    // Activate
    app.clone()
        .oneshot(
            Request::post(&format!("/platforms/{platform_id}/activate"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let resp = app
        .oneshot(Request::get("/p/test-channel").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Test Channel"));
    // Theme CSS should be injected (dark mode was enabled)
    assert!(html.contains("--color-background: #0f172a"));
}

#[tokio::test]
async fn other_curator_cannot_edit_platform() {
    let mut app = build_app().await;
    let cookie1 = register_person(&mut app, "curator1@test.com").await;
    let platform_id = create_platform(&mut app, &cookie1).await;

    let cookie2 = register_person(&mut app, "curator2@test.com").await;

    let resp = app
        .oneshot(
            Request::get(&format!("/platforms/{platform_id}/edit"))
                .header("cookie", &cookie2)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
