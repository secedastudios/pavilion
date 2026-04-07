use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Form;
use serde::Deserialize;
use surrealdb::types::{RecordId, SurrealValue};

use crate::auth::claims::Claims;
use crate::auth::middleware::OptionalClaims;
use crate::delivery::{audit, manifest, token};
use crate::error::AppError;
use crate::models::film::{Film, FilmView};
use crate::models::platform::{Platform, PlatformView};
use crate::router::AppState;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/player.html")]
struct PlayerTemplate {
    platform: PlatformView,
    film: FilmView,
    manifest_url: String,
    theme_css: String,
    resume_position: i64,
}

// ── Rights enforcement chain ───────────────────────────────

struct EnforcementContext {
    platform: Platform,
    film: Film,
    person_id: String,
}

/// The full rights enforcement chain. Every manifest/player request must pass this.
async fn enforce_rights(
    state: &AppState,
    platform_slug: &str,
    film_slug: &str,
    claims: &Claims,
) -> Result<EnforcementContext, AppError> {
    // 1. Verify platform exists and is active
    let platforms: Vec<Platform> = state
        .db
        .query("SELECT * FROM platform WHERE slug = $slug AND status = 'active' LIMIT 1")
        .bind(("slug", platform_slug.to_string()))
        .await?
        .take(0)?;
    let platform = platforms.into_iter().next().ok_or(AppError::NotFound)?;

    // 2. Verify film exists and is published
    let films: Vec<Film> = state
        .db
        .query("SELECT * FROM film WHERE slug = $slug AND status = 'published' LIMIT 1")
        .bind(("slug", film_slug.to_string()))
        .await?
        .take(0)?;
    let film = films.into_iter().next().ok_or(AppError::NotFound)?;

    // 3. Verify platform carries this film
    let carried: Vec<serde_json::Value> = state
        .db
        .query("SELECT id FROM carries WHERE in = $pid AND out = $fid LIMIT 1")
        .bind(("pid", platform.id.clone()))
        .bind(("fid", film.id.clone()))
        .await?
        .take(0)?;

    if carried.is_empty() {
        audit::log_access(
            &state.db,
            Some(claims.person_id()),
            Some(film.id.clone()),
            Some(platform.id.clone()),
            "manifest",
            "denied",
            Some("Platform does not carry this film"),
        )
        .await;
        return Err(AppError::LicenseViolation(
            "This film is not available on this platform.".into(),
        ));
    }

    // 4. Verify active license exists for this film
    let has_license = crate::licensing::rights::film_has_any_license(&state.db, &film.id).await?;
    if !has_license {
        audit::log_access(
            &state.db,
            Some(claims.person_id()),
            Some(film.id.clone()),
            Some(platform.id.clone()),
            "manifest",
            "denied",
            Some("No active license"),
        )
        .await;
        return Err(AppError::LicenseViolation("No active license for this film.".into()));
    }

    // 5. Check DMCA status
    let has_claim = crate::models::dmca::film_has_active_claim(&state.db, &film.id).await?;
    if has_claim {
        audit::log_access(
            &state.db,
            Some(claims.person_id()),
            Some(film.id.clone()),
            Some(platform.id.clone()),
            "manifest",
            "denied",
            Some("Active DMCA claim"),
        )
        .await;
        return Err(AppError::LicenseViolation("This film is currently subject to a copyright claim.".into()));
    }

    // 6. Check viewer entitlement
    // Determine if the active license requires payment (TVOD/SVOD) or is free (AVOD/CC)
    let licenses = crate::licensing::rights::licenses_for_film(&state.db, &film.id).await?;
    let license_types: Vec<&str> = licenses.iter()
        .filter(|l| l.active)
        .map(|l| l.license_type.as_str())
        .collect();

    // If all active licenses require payment, check entitlement
    let needs_entitlement = !license_types.is_empty()
        && !license_types.iter().any(|t| matches!(*t, "avod" | "cc" | "free"));

    if needs_entitlement {
        let entitlement = crate::payments::entitlements::check_entitlement(
            &state.db,
            &claims.person_id(),
            &film.id,
            &platform.id,
            license_types.first().unwrap_or(&"tvod"),
        ).await?;

        if entitlement.is_none() {
            audit::log_access(
                &state.db,
                Some(claims.person_id()),
                Some(film.id.clone()),
                Some(platform.id.clone()),
                "manifest",
                "denied",
                Some("No entitlement (payment required)"),
            )
            .await;
            return Err(AppError::LicenseViolation(
                "Payment required to access this content.".into(),
            ));
        }
    }

    let person_id = crate::util::record_id_key_string(&claims.person_id().key);

    Ok(EnforcementContext {
        platform,
        film,
        person_id,
    })
}

// ── Handlers ───────────────────────────────────────────────

/// Player page — renders the video player with the manifest URL.
pub async fn player_page(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((platform_slug, film_slug)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let ctx = enforce_rights(&state, &platform_slug, &film_slug, &claims).await?;

    let manifest_url = format!(
        "/watch/{platform_slug}/{film_slug}/manifest.m3u8"
    );

    let theme_css = ctx.platform.theme.as_ref()
        .map(|t| t.to_css_overrides())
        .unwrap_or_default();

    // Get resume position from last watch session
    let resume = get_resume_position(&state, &claims, &ctx.film.id, &ctx.platform.id).await;

    audit::log_access(
        &state.db,
        Some(claims.person_id()),
        Some(ctx.film.id.clone()),
        Some(ctx.platform.id.clone()),
        "manifest",
        "allowed",
        None,
    )
    .await;

    render_or_error(&PlayerTemplate {
        platform: ctx.platform.into(),
        film: ctx.film.into(),
        manifest_url,
        theme_css,
        resume_position: resume,
    })
}

/// HLS manifest endpoint — returns rewritten .m3u8 with signed segment URLs.
pub async fn hls_manifest(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((platform_slug, film_slug)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let ctx = enforce_rights(&state, &platform_slug, &film_slug, &claims).await?;

    let film_key = crate::util::record_id_key_string(&ctx.film.id.key);
    let platform_key = crate::util::record_id_key_string(&ctx.platform.id.key);

    // Read HLS master manifest from RustFS
    let manifest_key = format!("videos/{film_key}/master.m3u8");
    let raw_manifest = match state.storage.get_bytes(&manifest_key).await {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(_) => {
            // Fallback: generate a placeholder manifest for films not yet transcoded
            format!(
                "#EXTM3U\n#EXT-X-VERSION:7\n\
                 #EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360\n\
                 {film_key}/360p/360p.m3u8\n\
                 #EXT-X-STREAM-INF:BANDWIDTH=2800000,RESOLUTION=1280x720\n\
                 {film_key}/720p/720p.m3u8\n\
                 #EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080\n\
                 {film_key}/1080p/1080p.m3u8\n"
            )
        }
    };

    let signed = manifest::rewrite_hls_manifest(
        &raw_manifest,
        &ctx.person_id,
        &film_key,
        &platform_key,
        &state.config.jwt_secret,
        300,
        "/segments/",
    );

    Ok((
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "application/vnd.apple.mpegurl".to_string()),
            (axum::http::header::CACHE_CONTROL, "no-store".to_string()),
        ],
        signed,
    )
        .into_response())
}

/// DASH manifest endpoint — returns rewritten .mpd with signed segment URLs.
pub async fn dash_manifest(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((platform_slug, film_slug)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let ctx = enforce_rights(&state, &platform_slug, &film_slug, &claims).await?;

    let film_key = crate::util::record_id_key_string(&ctx.film.id.key);
    let platform_key = crate::util::record_id_key_string(&ctx.platform.id.key);

    // Read DASH manifest from RustFS
    let mpd_key = format!("videos/{film_key}/manifest.mpd");
    let raw_mpd = match state.storage.get_bytes(&mpd_key).await {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(_) => "<MPD></MPD>".to_string(),
    };

    let signed = manifest::rewrite_dash_manifest(
        &raw_mpd,
        &ctx.person_id,
        &film_key,
        &platform_key,
        &state.config.jwt_secret,
        300,
        "/segments/",
    );

    Ok((
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "application/dash+xml".to_string()),
            (axum::http::header::CACHE_CONTROL, "no-store".to_string()),
        ],
        signed,
    )
        .into_response())
}

/// Segment proxy — validates token and streams the segment.
pub async fn segment_proxy(
    State(state): State<Arc<AppState>>,
    OptionalClaims(claims): OptionalClaims,
    Path(token_str): Path<String>,
) -> Result<Response, AppError> {
    let seg_token = token::SegmentToken::verify(&token_str, &state.config.jwt_secret)
        .map_err(|e| {
            tracing::warn!(error = %e, "Invalid segment token");
            AppError::Unauthorized
        })?;

    // Verify person matches if authenticated
    if let Some(ref c) = claims {
        let person_key = crate::util::record_id_key_string(&c.person_id().key);
        if !seg_token.matches_subject(&person_key) {
            tracing::warn!(
                token_subject = %seg_token.subject,
                request_person = %person_key,
                "Segment token person mismatch"
            );
            return Err(AppError::Unauthorized);
        }
    }

    // Stream segment from RustFS
    let (bytes, content_type) = state
        .storage
        .get_stream(&seg_token.segment_path)
        .await
        .map_err(|e| {
            tracing::warn!(path = %seg_token.segment_path, error = %e, "Segment not found");
            AppError::NotFound
        })?;

    Ok((
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, content_type),
            (axum::http::header::CACHE_CONTROL, "private, max-age=300".to_string()),
        ],
        bytes,
    )
        .into_response())
}

/// Playhead heartbeat — viewer reports current position.
#[derive(Deserialize)]
pub struct HeartbeatForm {
    pub position: i64,
    pub duration: Option<i64>,
}

pub async fn playhead_heartbeat(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path((platform_slug, film_slug)): Path<(String, String)>,
    Form(form): Form<HeartbeatForm>,
) -> Result<Response, AppError> {
    let platforms: Vec<Platform> = state
        .db
        .query("SELECT * FROM platform WHERE slug = $slug LIMIT 1")
        .bind(("slug", platform_slug.to_string()))
        .await?
        .take(0)?;
    let platform = platforms.into_iter().next().ok_or(AppError::NotFound)?;

    let films: Vec<Film> = state
        .db
        .query("SELECT * FROM film WHERE slug = $slug LIMIT 1")
        .bind(("slug", film_slug.to_string()))
        .await?
        .take(0)?;
    let film = films.into_iter().next().ok_or(AppError::NotFound)?;

    let completed = form.duration
        .map(|d| d > 0 && form.position >= (d * 90 / 100))
        .unwrap_or(false);

    // Upsert watch session
    state
        .db
        .query(
            "UPSERT watch_session SET \
                person = $person, \
                film = $film, \
                platform = $platform, \
                progress_seconds = $position, \
                duration_seconds = $duration, \
                completed = $completed, \
                last_heartbeat = time::now() \
             WHERE person = $person AND film = $film AND platform = $platform"
        )
        .bind(("person", claims.person_id()))
        .bind(("film", film.id))
        .bind(("platform", platform.id))
        .bind(("position", form.position))
        .bind(("duration", form.duration))
        .bind(("completed", completed))
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

// ── Helpers ────────────────────────────────────────────────

async fn get_resume_position(
    state: &AppState,
    claims: &Claims,
    film_id: &RecordId,
    platform_id: &RecordId,
) -> i64 {
    #[derive(serde::Deserialize, SurrealValue)]
    struct ProgressRow {
        progress_seconds: i64,
    }

    let result: Result<Vec<ProgressRow>, _> = state
        .db
        .query(
            "SELECT progress_seconds FROM watch_session \
             WHERE person = $person AND film = $film AND platform = $platform \
             LIMIT 1"
        )
        .bind(("person", claims.person_id()))
        .bind(("film", film_id.clone()))
        .bind(("platform", platform_id.clone()))
        .await
        .and_then(|mut r| r.take(0));

    result
        .ok()
        .and_then(|rows| rows.into_iter().next())
        .map(|r| r.progress_seconds)
        .unwrap_or(0)
}
