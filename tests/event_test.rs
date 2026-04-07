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
            .body(Body::from(body)).unwrap(),
    ).await.unwrap();
    resp.headers().get("set-cookie").unwrap().to_str().unwrap()
        .split(';').next().unwrap().to_string()
}

/// Create a platform and return (cookie, platform_id).
async fn create_platform(app: &mut axum::Router, cookie: &str) -> String {
    let resp = app.clone().oneshot(
        Request::post("/platforms")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", cookie)
            .body(Body::from("name=Event+Channel")).unwrap(),
    ).await.unwrap();
    resp.headers().get("location").unwrap().to_str().unwrap()
        .strip_prefix("/platforms/").unwrap().to_string()
}

/// Create a film and return the film_id.
async fn create_film(app: &mut axum::Router, cookie: &str) -> String {
    let resp = app.clone().oneshot(
        Request::post("/films")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", cookie)
            .body(Body::from("title=Event+Film&declare_copyright=yes&declare_talent=yes&declare_no_prohibited=yes")).unwrap(),
    ).await.unwrap();
    resp.headers().get("location").unwrap().to_str().unwrap()
        .strip_prefix("/films/").unwrap().to_string()
}

#[tokio::test]
async fn events_require_curator() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "random@test.com").await;
    let plat_id = create_platform(&mut app, &cookie).await;

    // Owner can access
    let resp = app.clone().oneshot(
        Request::get(&format!("/platforms/{plat_id}/events"))
            .header("cookie", &cookie)
            .body(Body::empty()).unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Other person cannot
    let other_cookie = register_person(&mut app, "other@test.com").await;
    let resp = app.oneshot(
        Request::get(&format!("/platforms/{plat_id}/events"))
            .header("cookie", &other_cookie)
            .body(Body::empty()).unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn create_event_success() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "curator@test.com").await;
    let plat_id = create_platform(&mut app, &cookie).await;
    let film_id = create_film(&mut app, &cookie).await;

    let body = format!(
        "title=Film+Premiere&event_type=premiere&film_id={film_id}\
         &start_time=2026-12-25T20%3A00&max_attendees=100&ticket_price=5.00"
    );
    let resp = app.clone().oneshot(
        Request::post(&format!("/platforms/{plat_id}/events"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cookie)
            .body(Body::from(body)).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.starts_with("/events/"));

    // View the event
    let resp = app.oneshot(
        Request::get(location).body(Body::empty()).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Film Premiere"));
    assert!(html.contains("upcoming"));
    assert!(html.contains("$5.00"));
}

#[tokio::test]
async fn purchase_ticket_and_cap() {
    let mut app = build_app().await;
    let curator_cookie = register_person(&mut app, "curator@test.com").await;
    let plat_id = create_platform(&mut app, &curator_cookie).await;
    let film_id = create_film(&mut app, &curator_cookie).await;

    // Create event with max 1 attendee
    let body = format!(
        "title=Tiny+Event&event_type=screening&film_id={film_id}\
         &start_time=2026-12-25T20%3A00&max_attendees=1"
    );
    let resp = app.clone().oneshot(
        Request::post(&format!("/platforms/{plat_id}/events"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &curator_cookie)
            .body(Body::from(body)).unwrap(),
    ).await.unwrap();
    let event_url = resp.headers().get("location").unwrap().to_str().unwrap().to_string();
    let event_id = event_url.strip_prefix("/events/").unwrap();

    // First attendee registers
    let viewer1 = register_person(&mut app, "viewer1@test.com").await;
    let resp = app.clone().oneshot(
        Request::post(&format!("/events/{event_id}/tickets"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &viewer1)
            .body(Body::empty()).unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // Second attendee gets rejected (sold out)
    let viewer2 = register_person(&mut app, "viewer2@test.com").await;
    let resp = app.oneshot(
        Request::post(&format!("/events/{event_id}/tickets"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &viewer2)
            .body(Body::empty()).unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn event_status_transitions() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "curator@test.com").await;
    let plat_id = create_platform(&mut app, &cookie).await;
    let film_id = create_film(&mut app, &cookie).await;

    let body = format!(
        "title=Live+Event&event_type=screening&film_id={film_id}&start_time=2026-12-25T20%3A00"
    );
    let resp = app.clone().oneshot(
        Request::post(&format!("/platforms/{plat_id}/events"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cookie)
            .body(Body::from(body)).unwrap(),
    ).await.unwrap();
    let event_id = resp.headers().get("location").unwrap().to_str().unwrap()
        .strip_prefix("/events/").unwrap().to_string();

    // upcoming → live
    let resp = app.clone().oneshot(
        Request::post(&format!("/events/{event_id}/status"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cookie)
            .body(Body::from("status=live")).unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // live → ended
    let resp = app.clone().oneshot(
        Request::post(&format!("/events/{event_id}/status"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cookie)
            .body(Body::from("status=ended")).unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // ended → live is invalid
    let resp = app.oneshot(
        Request::post(&format!("/events/{event_id}/status"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cookie)
            .body(Body::from("status=live")).unwrap(),
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn event_detail_public() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "curator@test.com").await;
    let plat_id = create_platform(&mut app, &cookie).await;
    let film_id = create_film(&mut app, &cookie).await;

    let body = format!(
        "title=Public+Event&event_type=screening&film_id={film_id}&start_time=2026-12-25T20%3A00"
    );
    let resp = app.clone().oneshot(
        Request::post(&format!("/platforms/{plat_id}/events"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cookie)
            .body(Body::from(body)).unwrap(),
    ).await.unwrap();
    let event_url = resp.headers().get("location").unwrap().to_str().unwrap().to_string();

    // Public (no auth) can view event detail
    let resp = app.oneshot(
        Request::get(&event_url).body(Body::empty()).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Public Event"));
    assert!(html.contains("Register to attend"));
}
