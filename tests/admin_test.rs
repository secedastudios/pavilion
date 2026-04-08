mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

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

#[tokio::test]
async fn admin_requires_auth() {
    let app = build_app().await;
    let resp = app
        .oneshot(Request::get("/admin").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_requires_admin_role() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "user@test.com").await;

    let resp = app
        .oneshot(
            Request::get("/admin")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn showcase_loads_without_auth() {
    let app = build_app().await;

    let resp = app
        .oneshot(Request::get("/showcase").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let html = body_string(resp).await;
    assert!(html.contains("Pavilion Showcase"));
}

#[tokio::test]
async fn security_headers_present() {
    let app = build_app().await;

    let resp = app
        .oneshot(Request::get("/healthcheck").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert!(resp.headers().contains_key("x-content-type-options"));
    assert!(resp.headers().contains_key("x-frame-options"));
    assert!(resp.headers().contains_key("content-security-policy"));
    assert_eq!(
        resp.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
}
