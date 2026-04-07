use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, State};
use axum::response::Response;

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::revenue::stats::{self, FilmmakerRevenueOverview, PlatformRevenueOverview};
use crate::router::AppState;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/revenue.html")]
struct RevenueTemplate {
    overview: FilmmakerRevenueOverview,
    total_earned_display: String,
}

#[derive(Template)]
#[template(path = "pages/platform_analytics.html")]
struct PlatformAnalyticsTemplate {
    platform_name: String,
    platform_key_str: String,
    overview: PlatformRevenueOverview,
    total_revenue_display: String,
    curator_share_display: String,
}

// ── Handlers ───────────────────────────────────────────────

/// Filmmaker revenue dashboard.
pub async fn filmmaker_dashboard(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    let overview = stats::filmmaker_revenue(&state.db, &claims.person_id())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Revenue query error: {e}")))?;

    let total_earned_display = format_cents(overview.total_earned_cents);
    render_or_error(&RevenueTemplate { overview, total_earned_display })
}

/// Curator platform analytics dashboard.
pub async fn platform_analytics(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let platform = crate::controllers::platforms::get_platform(&state, &id).await?;
    crate::controllers::platforms::require_curator_public(&state, &claims, &platform).await?;

    let overview = stats::platform_revenue(&state.db, &platform.id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Analytics query error: {e}")))?;

    let key_str = crate::util::record_id_key_string(&platform.id.key);
    let total_revenue_display = format_cents(overview.total_revenue_cents);
    let curator_share_display = format_cents(overview.curator_share_cents);

    render_or_error(&PlatformAnalyticsTemplate {
        platform_name: platform.name,
        platform_key_str: key_str,
        overview,
        total_revenue_display,
        curator_share_display,
    })
}

fn format_cents(cents: i64) -> String {
    format!("${:.2}", cents as f64 / 100.0)
}
