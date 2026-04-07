mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use pavilion::auth::claims::{issue_token, verify_token};
use pavilion::auth::password::{hash_password, verify_password};
use pavilion::router::{self, AppState};

// ── Password hashing ───────────────────────────────────────

#[test]
fn hash_and_verify_password() {
    let hash = hash_password("testpassword123").unwrap();
    assert!(verify_password("testpassword123", &hash).unwrap());
    assert!(!verify_password("wrongpassword", &hash).unwrap());
}

// ── JWT tokens ─────────────────────────────────────────────

#[test]
fn issue_and_verify_jwt() {
    let secret = "test-secret";
    let roles = vec!["filmmaker".to_string()];
    let token = issue_token("abc123", "Test User", &roles, secret).unwrap();
    let claims = verify_token(&token, secret).unwrap();

    assert_eq!(claims.sub, "abc123");
    assert_eq!(claims.name, "Test User");
    assert_eq!(claims.roles, vec!["filmmaker"]);
}

#[test]
fn verify_jwt_with_wrong_secret_fails() {
    let roles = vec!["filmmaker".to_string()];
    let token = issue_token("abc123", "Test", &roles, "secret1").unwrap();
    assert!(verify_token(&token, "secret2").is_err());
}

// ── Registration ───────────────────────────────────────────

#[tokio::test]
async fn register_page_returns_200() {
    let app = build_app().await;

    let response = app
        .oneshot(Request::get("/register").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("Create your account"));
    assert!(body.contains("accept_terms"));
    assert!(body.contains("accept_no_porn"));
    assert!(body.contains("accept_copyright"));
    assert!(body.contains("accept_talent"));
}

#[tokio::test]
async fn register_without_terms_fails() {
    let app = build_app().await;

    let body = "email=test%40example.com&name=Test&password=password123&password_confirm=password123";
    let response = app
        .oneshot(
            Request::post("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should re-render form with error (200, not redirect)
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_string(response).await;
    assert!(html.contains("must accept all terms"));
}

#[tokio::test]
async fn register_with_short_password_fails() {
    let app = build_app().await;

    let body = "email=test%40example.com&name=Test&password=short&password_confirm=short\
        &accept_terms=yes&accept_no_porn=yes&accept_copyright=yes&accept_talent=yes";
    let response = app
        .oneshot(
            Request::post("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let html = body_string(response).await;
    assert!(html.contains("at least 8 characters"));
}

#[tokio::test]
async fn register_with_mismatched_passwords_fails() {
    let app = build_app().await;

    let body = "email=test%40example.com&name=Test&password=password123&password_confirm=different123\
        &accept_terms=yes&accept_no_porn=yes&accept_copyright=yes&accept_talent=yes";
    let response = app
        .oneshot(
            Request::post("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let html = body_string(response).await;
    assert!(html.contains("do not match"));
}

#[tokio::test]
async fn register_success_sets_cookie_and_redirects() {
    let app = build_app().await;

    let body = "email=newuser%40example.com&name=New+User&password=password123&password_confirm=password123\
        &accept_terms=yes&accept_no_porn=yes&accept_copyright=yes&accept_talent=yes";
    let response = app
        .oneshot(
            Request::post("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers().get("location").unwrap().to_str().unwrap();
    assert_eq!(location, "/profile");

    let cookie = response.headers().get("set-cookie").unwrap().to_str().unwrap();
    assert!(cookie.starts_with("pavilion_token="));
}

// ── Login ──────────────────────────────────────────────────

#[tokio::test]
async fn login_page_returns_200() {
    let app = build_app().await;

    let response = app
        .oneshot(Request::get("/login").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("Log in"));
}

#[tokio::test]
async fn login_with_invalid_credentials_fails() {
    let app = build_app().await;

    let body = "email=nobody%40example.com&password=wrongpassword";
    let response = app
        .oneshot(
            Request::post("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let html = body_string(response).await;
    assert!(html.contains("Invalid email or password"));
}

// ── Protected routes without auth ──────────────────────────

#[tokio::test]
async fn profile_without_auth_returns_unauthorized() {
    let app = build_app().await;

    let response = app
        .oneshot(Request::get("/profile").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn settings_privacy_without_auth_returns_unauthorized() {
    let app = build_app().await;

    let response = app
        .oneshot(Request::get("/settings/privacy").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn data_export_without_auth_returns_unauthorized() {
    let app = build_app().await;

    let response = app
        .oneshot(Request::get("/settings/data-export").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ── Legal pages ────────────────────────────────────────────

#[tokio::test]
async fn terms_page_returns_200() {
    let app = build_app().await;

    let response = app
        .oneshot(Request::get("/terms").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("Terms of Use"));
}

#[tokio::test]
async fn privacy_page_returns_200() {
    let app = build_app().await;

    let response = app
        .oneshot(Request::get("/privacy").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("Privacy Policy"));
}

#[tokio::test]
async fn content_policy_page_returns_200() {
    let app = build_app().await;

    let response = app
        .oneshot(Request::get("/content-policy").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(body.contains("Content Policy"));
}

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
