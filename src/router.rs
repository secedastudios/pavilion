use std::sync::Arc;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::controllers::{acquisitions, admin, auth, billing, catalog, dmca, enrichment, events, films, home, legal, licenses, payments, platforms, player, profile, ratings, revenue, settings, showcase, transcode, upload};
use crate::db::Db;

pub struct AppState {
    pub db: Db,
    pub config: Config,
    pub storage: pavilion_media::storage::StorageClient,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Home
        .route("/", get(home::index))
        // Health
        .route("/healthcheck", get(healthcheck))
        // Auth
        .route("/register", get(auth::register_page).post(auth::register_submit))
        .route("/login", get(auth::login_page).post(auth::login_submit))
        .route("/logout", post(auth::logout))
        .route("/auth/slatehub", get(auth::slatehub_oauth_start))
        .route("/auth/slatehub/callback", get(auth::slatehub_oauth_callback))
        // Legal pages
        .route("/terms", get(legal::terms))
        .route("/privacy", get(legal::privacy))
        .route("/content-policy", get(legal::content_policy))
        // Profile
        .route("/profile", get(profile::show).put(profile::update))
        .route("/profile/edit", get(profile::edit))
        // GDPR / Settings
        .route(
            "/settings/privacy",
            get(settings::privacy_settings).put(settings::update_consent),
        )
        .route("/settings/data-export", get(settings::data_export))
        .route("/settings/delete-account", post(settings::delete_account))
        // Films
        .route("/films", get(films::index).post(films::create))
        .route("/films/new", get(films::new_form))
        .route("/films/{id}", get(films::show).put(films::update).delete(films::archive))
        .route("/films/{id}/edit", get(films::edit))
        .route("/films/{id}/status", post(films::update_status))
        .route("/films/{id}/upload", post(upload::upload_film))
        .route("/films/{id}/poster", post(upload::upload_poster))
        // Enrichment (TMDB + IMDB)
        .route("/films/{id}/enrich", get(enrichment::search_tmdb))
        .route("/films/{id}/enrich/preview", post(enrichment::preview_tmdb))
        .route("/films/{id}/enrich/apply", post(enrichment::apply_tmdb))
        .route("/films/{id}/enrich/imdb", post(enrichment::enrich_imdb))
        // Transcoding
        .route("/films/{id}/transcode", post(transcode::start_transcode).get(transcode::jobs_list))
        .route("/films/{film_id}/transcode/{job_id}/progress", get(transcode::job_progress))
        // Licenses
        .route("/films/{id}/licenses", get(licenses::index).post(licenses::create))
        .route("/films/{id}/licenses/new", get(licenses::new_form))
        .route("/films/{film_id}/licenses/{license_id}", get(licenses::edit).put(licenses::update))
        .route("/films/{film_id}/licenses/{license_id}/deactivate", post(licenses::deactivate))
        // Catalog (public browsing)
        .route("/catalog", get(catalog::browse))
        .route("/catalog/{id}", get(catalog::film_detail))
        // Acquisitions
        .route("/catalog/{id}/acquire", post(acquisitions::acquire))
        .route("/films/{id}/requests", get(acquisitions::film_requests))
        .route("/films/{film_id}/requests/{request_id}/approve", post(acquisitions::approve_request))
        .route("/films/{film_id}/requests/{request_id}/reject", post(acquisitions::reject_request))
        // Platforms (curator management)
        .route("/platforms", get(platforms::index).post(platforms::create))
        .route("/platforms/new", get(platforms::new_form))
        .route("/platforms/{id}", get(platforms::dashboard).put(platforms::update))
        .route("/platforms/{id}/edit", get(platforms::edit))
        .route("/platforms/{id}/activate", post(platforms::activate))
        .route("/platforms/{id}/content", post(platforms::add_film))
        .route("/platforms/{id}/content/{film_id}/remove", post(platforms::remove_film))
        // Public platform sites
        .route("/p/{slug}", get(platforms::public_home))
        .route("/p/{slug}/{film_slug}", get(platforms::public_film))
        // Video player & secure delivery
        .route("/watch/{platform_slug}/{film_slug}", get(player::player_page))
        .route("/watch/{platform_slug}/{film_slug}/manifest.m3u8", get(player::hls_manifest))
        .route("/watch/{platform_slug}/{film_slug}/manifest.mpd", get(player::dash_manifest))
        .route("/watch/{platform_slug}/{film_slug}/heartbeat", post(player::playhead_heartbeat))
        .route("/segments/{token}", get(player::segment_proxy))
        // Ratings
        .route("/p/{platform_slug}/films/{film_id}/rate", post(ratings::submit_rating).delete(ratings::delete_rating))
        .route("/p/{platform_slug}/films/{film_id}/ratings", get(ratings::list_ratings))
        .route("/p/{platform_slug}/ratings/{rating_id}/hide", post(ratings::hide_rating))
        // Payments
        .route("/platforms/{id}/payments", get(payments::payment_settings))
        .route("/platforms/{id}/payments/connect", post(payments::start_connect_onboarding))
        .route("/platforms/{id}/payments/callback", get(payments::connect_callback))
        .route("/p/{platform_slug}/checkout", post(payments::create_checkout))
        .route("/webhooks/stripe", post(payments::stripe_webhook))
        // Revenue & Analytics
        .route("/revenue", get(revenue::filmmaker_dashboard))
        .route("/platforms/{id}/analytics", get(revenue::platform_analytics))
        // DMCA
        .route("/dmca", get(dmca::dmca_form).post(dmca::submit_claim))
        .route("/dmca/agent", get(dmca::dmca_agent))
        .route("/films/{id}/claims", get(dmca::film_claims))
        .route("/films/{film_id}/claims/{claim_id}/counter", post(dmca::counter_claim))
        .route("/admin/dmca/{claim_id}", post(dmca::review_claim))
        // Events
        .route("/platforms/{id}/events", get(events::index).post(events::create))
        .route("/platforms/{id}/events/new", get(events::new_form))
        .route("/events/{id}", get(events::detail))
        .route("/events/{id}/tickets", post(events::purchase_ticket))
        .route("/events/{id}/status", post(events::update_status))
        // Billing
        .route("/billing", get(billing::dashboard))
        // Admin
        .route("/admin", get(admin::dashboard))
        .route("/admin/persons", get(admin::persons))
        .route("/admin/persons/{id}/roles", post(admin::update_roles))
        .route("/admin/persons/{id}/export", get(admin::gdpr_export))
        .route("/admin/persons/{id}/delete", post(admin::gdpr_delete))
        .route("/admin/dmca", get(admin::dmca_list))
        // Showcase (reference streaming site)
        .route("/showcase", get(showcase::home))
        // Static files
        .nest_service("/static", ServeDir::new("static"))
        .layer(axum::middleware::from_fn(crate::middleware::security_headers))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(Arc::new(state))
}

async fn healthcheck(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db_ok = state.db.query("RETURN true").await.is_ok();
    let status = if db_ok { "ok" } else { "degraded" };

    Json(serde_json::json!({
        "status": status,
        "version": env!("CARGO_PKG_VERSION"),
        "services": {
            "database": if db_ok { "connected" } else { "unreachable" },
        }
    }))
}
