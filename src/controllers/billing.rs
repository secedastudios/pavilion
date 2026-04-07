use std::sync::Arc;

use askama::Template;
use axum::extract::State;
use axum::response::Response;

use crate::auth::claims::Claims;
use crate::billing::{credits, metering, tiers};
use crate::error::AppError;
use crate::router::AppState;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/billing.html")]
struct BillingTemplate {
    storage_display: String,
    master_display: String,
    rendition_display: String,
    film_count: i64,
    asset_count: i64,
    estimated_cost_display: String,
    current_tier: String,
    credit_balance_display: String,
}

// ── Handlers ───────────────────────────────────────────────

pub async fn dashboard(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Response, AppError> {
    let person_id = claims.person_id();

    let usage = metering::get_usage(&state.db, &person_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Metering error: {e}")))?;

    let tier_list = tiers::list_tiers(&state.db)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Tiers error: {e}")))?;

    let current_tier = tiers::recommended_tier(&usage, &tier_list)
        .map(|t| t.name)
        .unwrap_or_else(|| "Custom".into());

    let estimated_cost = tiers::estimate_monthly_cost(&usage, &tier_list);

    let credit_balance = credits::get_balance(&state.db, &person_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Credits error: {e}")))?;

    render_or_error(&BillingTemplate {
        storage_display: metering::format_bytes(usage.total_bytes),
        master_display: metering::format_bytes(usage.master_bytes),
        rendition_display: metering::format_bytes(usage.rendition_bytes),
        film_count: usage.film_count,
        asset_count: usage.asset_count,
        estimated_cost_display: format!("${:.2}", estimated_cost as f64 / 100.0),
        current_tier,
        credit_balance_display: format!("${:.2}", credit_balance as f64 / 100.0),
    })
}
