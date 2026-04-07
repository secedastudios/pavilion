use std::sync::Arc;

use askama::Template;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;
use surrealdb::types::RecordId;

use crate::auth::claims::Claims;
use crate::error::AppError;
use crate::payments::entitlements;
use crate::payments::provider::PaymentProvider;
use crate::router::AppState;
use crate::templates::render_or_error;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "pages/payment_settings.html")]
struct PaymentSettingsTemplate {
    platform_key_str: String,
    platform_name: String,
    onboarding_complete: bool,
    payments_enabled: bool,
}

#[derive(Template)]
#[template(path = "pages/checkout_disabled.html")]
struct CheckoutDisabledTemplate;

// ── Curator: Stripe Connect onboarding ─────────────────────

pub async fn payment_settings(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let platform = crate::controllers::platforms::get_platform(&state, &id).await?;
    crate::controllers::platforms::require_curator_public(&state, &claims, &platform).await?;

    let platform_id = RecordId::new("platform", id.as_str());
    let onboarding_complete = get_onboarding_status(&state, &platform_id).await;

    render_or_error(&PaymentSettingsTemplate {
        platform_key_str: id,
        platform_name: platform.name,
        onboarding_complete,
        payments_enabled: state.config.payments_enabled(),
    })
}

pub async fn start_connect_onboarding(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let platform = crate::controllers::platforms::get_platform(&state, &id).await?;
    crate::controllers::platforms::require_curator_public(&state, &claims, &platform).await?;

    if !state.config.payments_enabled() {
        return Err(AppError::Validation("Payments are not configured on this instance.".into()));
    }

    let provider = get_provider(&state)?;
    let return_url = format!("{}/platforms/{id}/payments/callback", state.config.base_url);
    let refresh_url = format!("{}/platforms/{id}/payments", state.config.base_url);

    let result = provider
        .create_connect_account(&platform.name, &return_url, &refresh_url)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Payment provider error: {e}")))?;

    // Store the connected account
    let platform_id = RecordId::new("platform", id.as_str());
    state
        .db
        .query(
            "UPSERT payment_account SET \
                platform = $platform, \
                provider = 'stripe', \
                external_account_id = $account_id, \
                onboarding_complete = false \
             WHERE platform = $platform"
        )
        .bind(("platform", platform_id))
        .bind(("account_id", result.account_id))
        .await?;

    Ok(Redirect::to(&result.onboarding_url).into_response())
}

pub async fn connect_callback(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let platform = crate::controllers::platforms::get_platform(&state, &id).await?;
    crate::controllers::platforms::require_curator_public(&state, &claims, &platform).await?;

    // Mark onboarding as complete
    let platform_id = RecordId::new("platform", id.as_str());
    state
        .db
        .query("UPDATE payment_account SET onboarding_complete = true WHERE platform = $platform")
        .bind(("platform", platform_id))
        .await?;

    Ok(Redirect::to(&format!("/platforms/{id}/payments")).into_response())
}

// ── Viewer: checkout flows ─────────────────────────────────

#[derive(Deserialize)]
pub struct CheckoutForm {
    pub film_id: String,
    pub checkout_type: String, // "rental", "purchase", "subscription"
    pub amount_cents: Option<i64>,
    pub rental_hours: Option<i64>,
}

pub async fn create_checkout(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(platform_slug): Path<String>,
    Form(form): Form<CheckoutForm>,
) -> Result<Response, AppError> {
    if !state.config.payments_enabled() {
        return render_or_error(&CheckoutDisabledTemplate);
    }

    let platforms: Vec<crate::models::platform::Platform> = state
        .db
        .query("SELECT * FROM platform WHERE slug = $slug AND status = 'active' LIMIT 1")
        .bind(("slug", platform_slug.clone()))
        .await?
        .take(0)?;
    let platform = platforms.into_iter().next().ok_or(AppError::NotFound)?;
    let platform_key = crate::util::record_id_key_string(&platform.id.key);

    // Get connected account
    let accounts: Vec<serde_json::Value> = state
        .db
        .query("SELECT external_account_id FROM payment_account WHERE platform = $pid AND onboarding_complete = true LIMIT 1")
        .bind(("pid", platform.id.clone()))
        .await?
        .take(0)?;

    let connected_account_id = accounts
        .first()
        .and_then(|a| a["external_account_id"].as_str())
        .ok_or_else(|| AppError::Validation("Platform has not connected payment processing.".into()))?
        .to_string();

    let provider = get_provider(&state)?;

    let amount = form.amount_cents.unwrap_or(999);
    let person_key = crate::util::record_id_key_string(&claims.person_id().key);

    let mut metadata = std::collections::HashMap::new();
    metadata.insert("person_id".into(), person_key);
    metadata.insert("film_id".into(), form.film_id.clone());
    metadata.insert("platform_id".into(), platform_key.clone());
    metadata.insert("checkout_type".into(), form.checkout_type.clone());
    if let Some(hours) = form.rental_hours {
        metadata.insert("rental_hours".into(), hours.to_string());
    }

    let result = provider
        .create_checkout_session(crate::payments::provider::CheckoutParams {
            connected_account_id,
            line_items: vec![crate::payments::provider::LineItem {
                name: format!("Film {} — {}", form.film_id, form.checkout_type),
                description: form.checkout_type.clone(),
                amount_cents: amount,
                currency: "usd".into(),
                quantity: 1,
            }],
            success_url: format!(
                "{}/p/{platform_slug}/checkout/success?session_id={{CHECKOUT_SESSION_ID}}",
                state.config.base_url
            ),
            cancel_url: format!("{}/p/{platform_slug}/{}", state.config.base_url, form.film_id),
            metadata,
            application_fee_pct: state.config.facilitation_fee_pct,
        })
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Checkout error: {e}")))?;

    Ok(Redirect::to(&result.checkout_url).into_response())
}

// ── Webhook handler ────────────────────────────────────────

pub async fn stripe_webhook(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Result<Response, AppError> {
    let signature = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let provider = get_provider(&state)?;
    let event = provider
        .verify_webhook(&body, signature)
        .map_err(|e| {
            tracing::warn!(error = %e, "Invalid Stripe webhook");
            AppError::Unauthorized
        })?;

    tracing::info!(event_type = %event.event_type, "Stripe webhook received");

    match event.event_type.as_str() {
        "checkout.session.completed" => {
            handle_checkout_completed(&state, &event.data).await?;
        }
        "customer.subscription.updated" | "customer.subscription.deleted" => {
            handle_subscription_change(&state, &event.data).await?;
        }
        _ => {
            tracing::debug!(event_type = %event.event_type, "Unhandled webhook event");
        }
    }

    Ok(StatusCode::OK.into_response())
}

async fn handle_checkout_completed(
    state: &AppState,
    data: &serde_json::Value,
) -> Result<(), AppError> {
    let metadata = &data["metadata"];
    let person_id_str = metadata["person_id"].as_str().unwrap_or_default();
    let film_id_str = metadata["film_id"].as_str().unwrap_or_default();
    let platform_id_str = metadata["platform_id"].as_str().unwrap_or_default();
    let checkout_type = metadata["checkout_type"].as_str().unwrap_or_default();

    if person_id_str.is_empty() || platform_id_str.is_empty() {
        tracing::warn!("Webhook missing metadata");
        return Ok(());
    }

    let person_id = RecordId::new("person", person_id_str);
    let platform_id = RecordId::new("platform", platform_id_str);

    match checkout_type {
        "subscription" => {
            // Create subscription record
            state
                .db
                .query(
                    "UPSERT viewer_subscription SET \
                        person = $person, platform = $platform, \
                        provider = 'stripe', status = 'active' \
                     WHERE person = $person AND platform = $platform"
                )
                .bind(("person", person_id.clone()))
                .bind(("platform", platform_id.clone()))
                .await?;

            entitlements::grant_subscription_entitlements(&state.db, &person_id, &platform_id)
                .await?;
        }
        "rental" => {
            let hours: i64 = metadata["rental_hours"]
                .as_str()
                .and_then(|h| h.parse().ok())
                .unwrap_or(48);
            let expires = chrono::Utc::now() + chrono::Duration::hours(hours);
            let film_id = RecordId::new("film", film_id_str);

            entitlements::grant_entitlement(
                &state.db,
                person_id,
                film_id,
                platform_id,
                "rental",
                Some(expires),
                data["id"].as_str().map(|s| s.to_string()),
            )
            .await?;
        }
        "purchase" => {
            let film_id = RecordId::new("film", film_id_str);

            entitlements::grant_entitlement(
                &state.db,
                person_id,
                film_id,
                platform_id,
                "purchase",
                None,
                data["id"].as_str().map(|s| s.to_string()),
            )
            .await?;
        }
        _ => {}
    }

    Ok(())
}

async fn handle_subscription_change(
    state: &AppState,
    data: &serde_json::Value,
) -> Result<(), AppError> {
    let external_id = data["id"].as_str().unwrap_or_default();
    let status = data["status"].as_str().unwrap_or("canceled");

    state
        .db
        .query("UPDATE viewer_subscription SET status = $status WHERE external_id = $ext_id")
        .bind(("status", status.to_string()))
        .bind(("ext_id", external_id.to_string()))
        .await?;

    // If canceled, revoke entitlements
    if status == "canceled" || status == "unpaid" {
        let subs: Vec<entitlements::ViewerSubscription> = state
            .db
            .query("SELECT * FROM viewer_subscription WHERE external_id = $ext_id LIMIT 1")
            .bind(("ext_id", external_id.to_string()))
            .await?
            .take(0)?;

        if let Some(sub) = subs.into_iter().next() {
            entitlements::revoke_subscription_entitlements(&state.db, &sub.person, &sub.platform)
                .await?;
        }
    }

    Ok(())
}

// ── Helpers ────────────────────────────────────────────────

fn get_provider(state: &AppState) -> Result<crate::payments::stripe::StripeProvider, AppError> {
    let secret = state
        .config
        .stripe_secret_key
        .as_ref()
        .ok_or_else(|| AppError::Validation("Payments not configured.".into()))?;
    let webhook_secret = state
        .config
        .stripe_webhook_secret
        .as_deref()
        .unwrap_or_default();

    Ok(crate::payments::stripe::StripeProvider::new(
        secret.clone(),
        webhook_secret.to_string(),
    ))
}

async fn get_onboarding_status(state: &AppState, platform_id: &RecordId) -> bool {
    use surrealdb::types::SurrealValue;
    #[derive(serde::Deserialize, SurrealValue)]
    struct Row {
        onboarding_complete: bool,
    }

    let result: Result<Vec<Row>, _> = state
        .db
        .query("SELECT onboarding_complete FROM payment_account WHERE platform = $pid LIMIT 1")
        .bind(("pid", platform_id.clone()))
        .await
        .and_then(|mut r| r.take(0));

    result
        .ok()
        .and_then(|rows| rows.into_iter().next())
        .map(|r| r.onboarding_complete)
        .unwrap_or(false)
}
