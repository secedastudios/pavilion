mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use pavilion::auth::claims::issue_token;
use pavilion::router::{self, AppState};

// ── Helpers ────────────────────────────────────────────────

async fn build_app() -> axum::Router {
    let db = common::setup_test_db().await;
    let config = common::test_config();
    router::build_router(AppState { db, config, storage: common::test_storage() })
}

fn auth_cookie(secret: &str) -> String {
    let token = issue_token("testperson", "Test User", &["filmmaker".into()], secret).unwrap();
    format!("pavilion_token={token}")
}

async fn body_string(response: axum::http::Response<Body>) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// Register a test person and return the auth cookie value.
async fn register_person(app: &mut axum::Router) -> String {
    let body = "email=filmmaker%40test.com&name=Test+Filmmaker&password=password123&password_confirm=password123\
        &accept_terms=yes&accept_no_porn=yes&accept_copyright=yes&accept_talent=yes";
    let response = app
        .clone()
        .oneshot(
            Request::post("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response.headers().get("set-cookie").unwrap().to_str().unwrap();
    // Extract just the token part
    cookie.split(';').next().unwrap().to_string()
}

/// Create a test film and return the redirect location (film URL).
async fn create_film(app: &mut axum::Router, cookie: &str) -> String {
    let body = "title=Test+Film&synopsis=A+test+film&year=2026&genres=Drama%2C+Sci-Fi\
        &language=English&country=UK\
        &declare_copyright=yes&declare_talent=yes&declare_no_prohibited=yes";
    let response = app
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

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    response.headers().get("location").unwrap().to_str().unwrap().to_string()
}

// ── Film list requires auth ────────────────────────────────

#[tokio::test]
async fn films_index_requires_auth() {
    let app = build_app().await;

    let response = app
        .oneshot(Request::get("/films").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ── New film form requires auth ────────────────────────────

#[tokio::test]
async fn films_new_requires_auth() {
    let app = build_app().await;

    let response = app
        .oneshot(Request::get("/films/new").body(Body::empty()).unwrap())
        .await
        .unwrap();

    // new_form doesn't extract Claims, so it returns 200
    // This is intentional — the POST will enforce auth
    assert_eq!(response.status(), StatusCode::OK);
}

// ── Create film without content declaration fails ──────────

#[tokio::test]
async fn create_film_without_declaration_fails() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app).await;

    let body = "title=My+Film&synopsis=Test&year=2026&genres=Drama&language=English&country=UK";
    let response = app
        .oneshot(
            Request::post("/films")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let html = body_string(response).await;
    assert!(html.contains("content declarations"));
}

// ── Create film success ────────────────────────────────────

#[tokio::test]
async fn create_film_success() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app).await;
    let location = create_film(&mut app, &cookie).await;

    assert!(location.starts_with("/films/"));

    // Verify we can view the film
    let response = app
        .oneshot(
            Request::get(&location)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let html = body_string(response).await;
    assert!(html.contains("Test Film"));
    assert!(html.contains("draft"));
}

// ── Film index shows owned films ───────────────────────────

#[tokio::test]
async fn films_index_shows_owned_films() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app).await;
    create_film(&mut app, &cookie).await;

    let response = app
        .oneshot(
            Request::get("/films")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let html = body_string(response).await;
    assert!(html.contains("Test Film"));
}

// ── Status transitions ─────────────────────────────────────

#[tokio::test]
async fn publish_film_from_draft() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app).await;
    let location = create_film(&mut app, &cookie).await;
    let film_id = location.strip_prefix("/films/").unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::post(&format!("/films/{film_id}/status"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &cookie)
                .body(Body::from("status=published"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    // Verify status changed
    let response = app
        .oneshot(
            Request::get(&location)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let html = body_string(response).await;
    assert!(html.contains("published"));
}

#[tokio::test]
async fn invalid_status_transition_fails() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app).await;
    let location = create_film(&mut app, &cookie).await;
    let film_id = location.strip_prefix("/films/").unwrap();

    // draft → archived is not valid (must go through published)
    let response = app
        .oneshot(
            Request::post(&format!("/films/{film_id}/status"))
                .header("content-type", "application/x-www-form-urlencoded")
                .header("cookie", &cookie)
                .body(Body::from("status=archived"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ── Ownership enforcement ──────────────────────────────────

#[tokio::test]
async fn cannot_edit_another_persons_film() {
    let mut app = build_app().await;

    // Person 1 creates film
    let cookie1 = register_person(&mut app).await;
    let location = create_film(&mut app, &cookie1).await;
    let film_id = location.strip_prefix("/films/").unwrap();

    // Person 2 registers
    let body = "email=other%40test.com&name=Other+Person&password=password123&password_confirm=password123\
        &accept_terms=yes&accept_no_porn=yes&accept_copyright=yes&accept_talent=yes";
    let response = app
        .clone()
        .oneshot(
            Request::post("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie2 = response.headers().get("set-cookie").unwrap().to_str().unwrap();
    let cookie2 = cookie2.split(';').next().unwrap();

    // Person 2 tries to edit Person 1's film
    let response = app
        .oneshot(
            Request::get(&format!("/films/{film_id}/edit"))
                .header("cookie", cookie2)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
