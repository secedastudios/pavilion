mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use surrealdb::types::RecordId;
use tower::ServiceExt;

use pavilion::models::dmca;
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

// ── DMCA form (public) ────────────────────────────────────

#[tokio::test]
async fn dmca_form_loads_without_auth() {
    let app = build_app().await;

    let resp = app.oneshot(
        Request::get("/dmca").body(Body::empty()).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("DMCA Takedown Request"));
    assert!(html.contains("good_faith"));
    assert!(html.contains("perjury"));
}

#[tokio::test]
async fn dmca_agent_page_loads() {
    let app = build_app().await;

    let resp = app.oneshot(
        Request::get("/dmca/agent").body(Body::empty()).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Designated Agent"));
}

#[tokio::test]
async fn submit_claim_requires_declarations() {
    let app = build_app().await;

    let body = "claimant_name=John+Doe&claimant_email=john%40example.com\
        &film_id=some-film&description=My+copyrighted+work";
    let resp = app.oneshot(
        Request::post("/dmca")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body)).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("good faith statement"));
}

#[tokio::test]
async fn submit_claim_success() {
    let app = build_app().await;

    let body = "claimant_name=John+Doe&claimant_email=john%40example.com\
        &film_id=some-film&description=My+copyrighted+work\
        &good_faith=yes&perjury=yes";
    let resp = app.oneshot(
        Request::post("/dmca")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(body)).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("Claim submitted"));
}

// ── Film has active claim check ───────────────────────────

#[tokio::test]
async fn film_has_no_claims_initially() {
    let db = common::setup_test_db().await;
    let film_id = RecordId::new("film", "test-film");

    let has_claim = dmca::film_has_active_claim(&db, &film_id).await.unwrap();
    assert!(!has_claim);
}

#[tokio::test]
async fn filed_claim_blocks_film() {
    let db = common::setup_test_db().await;
    let film_id = RecordId::new("film", "test-film");

    let _: Option<dmca::DmcaClaim> = db
        .create("dmca_claim")
        .content(dmca::CreateDmcaClaim {
            claimant_name: "John".into(),
            claimant_email: "john@example.com".into(),
            claimant_company: None,
            film: film_id.clone(),
            description: "My work".into(),
            evidence_url: None,
            good_faith_statement: true,
            perjury_declaration: true,
        })
        .await
        .unwrap();

    let has_claim = dmca::film_has_active_claim(&db, &film_id).await.unwrap();
    assert!(has_claim);
}

// ── Filmmaker views claims ────────────────────────────────

#[tokio::test]
async fn filmmaker_sees_claims() {
    let mut app = build_app().await;
    let cookie = register_person(&mut app, "filmmaker@test.com").await;

    // Create film
    let resp = app.clone().oneshot(
        Request::post("/films")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("cookie", &cookie)
            .body(Body::from("title=Claimed+Film&declare_copyright=yes&declare_talent=yes&declare_no_prohibited=yes")).unwrap(),
    ).await.unwrap();
    let film_id = resp.headers().get("location").unwrap().to_str().unwrap()
        .strip_prefix("/films/").unwrap().to_string();

    let resp = app.oneshot(
        Request::get(&format!("/films/{film_id}/claims"))
            .header("cookie", &cookie)
            .body(Body::empty()).unwrap(),
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let html = body_string(resp).await;
    assert!(html.contains("DMCA Claims"));
}
