mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

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
            .body(Body::from(body))
            .unwrap(),
    ).await.unwrap();
    resp.headers().get("set-cookie").unwrap().to_str().unwrap()
        .split(';').next().unwrap().to_string()
}

/// Create a published film with license, platform activated with film carried.
/// Returns (filmmaker_cookie, curator_cookie, platform_slug, film_id).
async fn setup_rated_film(app: &mut axum::Router) -> (String, String, String, String) {
    let fm = register_person(app, "filmmaker@test.com").await;

    // Create + publish film
    let resp = app.clone().oneshot(
        Request::post("/films")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &fm)
            .body(Body::from("title=Rated+Film&synopsis=Test&year=2026&genres=Drama&language=English&country=UK&declare_copyright=yes&declare_talent=yes&declare_no_prohibited=yes"))
            .unwrap(),
    ).await.unwrap();
    let film_id = resp.headers().get("location").unwrap().to_str().unwrap()
        .strip_prefix("/films/").unwrap().to_string();

    app.clone().oneshot(
        Request::post(&format!("/films/{film_id}/status"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &fm)
            .body(Body::from("status=published")).unwrap(),
    ).await.unwrap();

    // License
    app.clone().oneshot(
        Request::post(&format!("/films/{film_id}/licenses"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &fm)
            .body(Body::from("license_type=avod&revenue_share_pct=60")).unwrap(),
    ).await.unwrap();

    // Curator + platform
    let cu = register_person(app, "curator@test.com").await;
    let resp = app.clone().oneshot(
        Request::post("/platforms")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu)
            .body(Body::from("name=Rate+Channel&description=Test")).unwrap(),
    ).await.unwrap();
    let plat_id = resp.headers().get("location").unwrap().to_str().unwrap()
        .strip_prefix("/platforms/").unwrap().to_string();

    // Activate
    app.clone().oneshot(
        Request::post(&format!("/platforms/{plat_id}/activate"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu)
            .body(Body::empty()).unwrap(),
    ).await.unwrap();

    // Add film
    app.clone().oneshot(
        Request::post(&format!("/platforms/{plat_id}/content"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu)
            .body(Body::from(format!("film_id={film_id}"))).unwrap(),
    ).await.unwrap();

    (fm, cu, "rate-channel".to_string(), film_id)
}

#[tokio::test]
async fn submit_rating_requires_auth() {
    let app = build_app().await;

    let resp = app.oneshot(
        Request::post("/p/fake/films/fake/rate")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from("score=4")).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn submit_and_retrieve_rating() {
    let mut app = build_app().await;
    let (_fm, cu, plat_slug, film_id) = setup_rated_film(&mut app).await;

    // Submit rating
    let resp = app.clone().oneshot(
        Request::post(&format!("/p/{plat_slug}/films/{film_id}/rate"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu)
            .body(Body::from("score=4&review_text=Great+film")).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("4/5"));
    assert!(html.contains("Great film"));
}

#[tokio::test]
async fn rating_score_must_be_1_to_5() {
    let mut app = build_app().await;
    let (_fm, cu, plat_slug, film_id) = setup_rated_film(&mut app).await;

    let resp = app.oneshot(
        Request::post(&format!("/p/{plat_slug}/films/{film_id}/rate"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu)
            .body(Body::from("score=7")).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn update_existing_rating() {
    let mut app = build_app().await;
    let (_fm, cu, plat_slug, film_id) = setup_rated_film(&mut app).await;

    // First rating
    app.clone().oneshot(
        Request::post(&format!("/p/{plat_slug}/films/{film_id}/rate"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu)
            .body(Body::from("score=3")).unwrap(),
    ).await.unwrap();

    // Update
    let resp = app.oneshot(
        Request::post(&format!("/p/{plat_slug}/films/{film_id}/rate"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu)
            .body(Body::from("score=5&review_text=Changed+my+mind")).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("5/5"));
    assert!(html.contains("Changed my mind"));
}

#[tokio::test]
async fn per_platform_isolation() {
    let mut app = build_app().await;
    let (fm, cu, plat_slug, film_id) = setup_rated_film(&mut app).await;

    // Rate on first platform
    app.clone().oneshot(
        Request::post(&format!("/p/{plat_slug}/films/{film_id}/rate"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu)
            .body(Body::from("score=5")).unwrap(),
    ).await.unwrap();

    // Create second platform
    let cu2 = register_person(&mut app, "curator2@test.com").await;
    let resp = app.clone().oneshot(
        Request::post("/platforms")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu2)
            .body(Body::from("name=Other+Channel")).unwrap(),
    ).await.unwrap();
    let plat2_id = resp.headers().get("location").unwrap().to_str().unwrap()
        .strip_prefix("/platforms/").unwrap().to_string();
    app.clone().oneshot(
        Request::post(&format!("/platforms/{plat2_id}/activate"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu2)
            .body(Body::empty()).unwrap(),
    ).await.unwrap();
    app.clone().oneshot(
        Request::post(&format!("/platforms/{plat2_id}/content"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu2)
            .body(Body::from(format!("film_id={film_id}"))).unwrap(),
    ).await.unwrap();

    // Ratings on second platform should be empty
    let resp = app.oneshot(
        Request::get(&format!("/p/other-channel/films/{film_id}/ratings"))
            .header("cookie", &cu2)
            .body(Body::empty()).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("No ratings yet"));
}
