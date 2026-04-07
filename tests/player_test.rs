mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use pavilion::delivery::token::SegmentToken;
use pavilion::router::{self, AppState};

// ── Token unit tests ───────────────────────────────────────

#[test]
fn sign_and_verify_token() {
    let token = SegmentToken::new("person1", "film1", "platform1", "360p/seg_0001.m4s", 300);
    let signed = token.sign("test-secret");

    let verified = SegmentToken::verify(&signed, "test-secret").unwrap();
    assert_eq!(verified.subject, "person1");
    assert_eq!(verified.resource, "film1");
    assert_eq!(verified.scope, "platform1");
    assert_eq!(verified.segment_path, "360p/seg_0001.m4s");
}

#[test]
fn wrong_secret_fails_verification() {
    let token = SegmentToken::new("person1", "film1", "platform1", "seg.m4s", 300);
    let signed = token.sign("secret-a");
    assert!(SegmentToken::verify(&signed, "secret-b").is_err());
}

#[test]
fn tampered_token_fails() {
    let token = SegmentToken::new("person1", "film1", "platform1", "seg.m4s", 300);
    let signed = token.sign("test-secret");
    let tampered = format!("{signed}x");
    assert!(SegmentToken::verify(&tampered, "test-secret").is_err());
}

#[test]
fn person_mismatch_detected() {
    let token = SegmentToken::new("person1", "film1", "platform1", "seg.m4s", 300);
    assert!(token.matches_subject("person1"));
    assert!(!token.matches_subject("person2"));
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
    resp.headers().get("set-cookie").unwrap().to_str().unwrap()
        .split(';').next().unwrap().to_string()
}

/// Set up a full chain: filmmaker creates film, publishes, adds license,
/// curator creates platform, activates, adds film to platform.
/// Returns (filmmaker_cookie, curator_cookie, platform_slug, film_slug).
async fn setup_full_chain(app: &mut axum::Router) -> (String, String, String, String) {
    // Filmmaker
    let fm_cookie = register_person(app, "filmmaker@test.com").await;

    // Create film
    let body = "title=Watchable+Film&synopsis=Great&year=2026&genres=Drama\
        &language=English&country=UK\
        &declare_copyright=yes&declare_talent=yes&declare_no_prohibited=yes";
    let resp = app.clone().oneshot(
        Request::post("/films")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &fm_cookie)
            .body(Body::from(body))
            .unwrap(),
    ).await.unwrap();
    let film_url = resp.headers().get("location").unwrap().to_str().unwrap().to_string();
    let film_id = film_url.strip_prefix("/films/").unwrap().to_string();

    // Publish
    app.clone().oneshot(
        Request::post(&format!("/films/{film_id}/status"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &fm_cookie)
            .body(Body::from("status=published"))
            .unwrap(),
    ).await.unwrap();

    // Add AVOD license
    app.clone().oneshot(
        Request::post(&format!("/films/{film_id}/licenses"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &fm_cookie)
            .body(Body::from("license_type=avod&revenue_share_pct=60"))
            .unwrap(),
    ).await.unwrap();

    // Curator
    let cu_cookie = register_person(app, "curator@test.com").await;

    // Create platform
    let resp = app.clone().oneshot(
        Request::post("/platforms")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu_cookie)
            .body(Body::from("name=Watch+Channel&description=Test"))
            .unwrap(),
    ).await.unwrap();
    let plat_url = resp.headers().get("location").unwrap().to_str().unwrap().to_string();
    let plat_id = plat_url.strip_prefix("/platforms/").unwrap().to_string();

    // Activate
    app.clone().oneshot(
        Request::post(&format!("/platforms/{plat_id}/activate"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu_cookie)
            .body(Body::empty())
            .unwrap(),
    ).await.unwrap();

    // Add film to platform
    app.clone().oneshot(
        Request::post(&format!("/platforms/{plat_id}/content"))
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cu_cookie)
            .body(Body::from(format!("film_id={film_id}")))
            .unwrap(),
    ).await.unwrap();

    (fm_cookie, cu_cookie, "watch-channel".to_string(), "watchable-film".to_string())
}

#[tokio::test]
async fn player_requires_auth() {
    let app = build_app().await;

    let resp = app
        .oneshot(
            Request::get("/watch/some-platform/some-film")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn player_denies_nonexistent_platform() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "viewer@test.com").await;

    let resp = app
        .oneshot(
            Request::get("/watch/fake-platform/fake-film")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn player_loads_with_full_chain() {
    let mut app = build_app().await;
    let (_fm, cu, plat_slug, film_slug) = setup_full_chain(&mut app).await;

    let resp = app
        .oneshot(
            Request::get(&format!("/watch/{plat_slug}/{film_slug}"))
                .header("cookie", &cu)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("video-player"));
    assert!(html.contains("manifest.m3u8"));
    assert!(html.contains("Watchable Film"));
}

#[tokio::test]
async fn hls_manifest_returns_signed_urls() {
    let mut app = build_app().await;
    let (_fm, cu, plat_slug, film_slug) = setup_full_chain(&mut app).await;

    let resp = app
        .oneshot(
            Request::get(&format!("/watch/{plat_slug}/{film_slug}/manifest.m3u8"))
                .header("cookie", &cu)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("#EXTM3U"));
    // All segment lines should be signed /segments/ URLs
    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') && !trimmed.is_empty() {
            assert!(trimmed.starts_with("/segments/"), "Unsigned segment URL: {trimmed}");
        }
    }
}

#[tokio::test]
async fn manifest_denies_uncarried_film() {
    let mut app = build_app().await;
    let (fm, _cu, plat_slug, _) = setup_full_chain(&mut app).await;

    // Try to access a different film slug that doesn't exist
    let resp = app
        .oneshot(
            Request::get(&format!("/watch/{plat_slug}/nonexistent-film/manifest.m3u8"))
                .header("cookie", &fm)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn segment_proxy_rejects_invalid_token() {
    let app = build_app().await;

    let resp = app
        .oneshot(
            Request::get("/segments/invalid-token-garbage")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
