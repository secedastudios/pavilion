//! Abstract `PaymentProvider` trait with Stripe and no-op implementations.
//! Defines the interface for Connect account creation, checkout sessions,
//! and webhook signature verification used across the payments module.

use serde::{Deserialize, Serialize};

/// Configurable payment provider trait. Stripe is the default implementation,
/// but self-hosters can implement their own or disable payments entirely.
pub trait PaymentProvider: Send + Sync {
    /// Create a Connect account for a curator (Stripe Connect, etc.)
    fn create_connect_account(
        &self,
        platform_name: &str,
        return_url: &str,
        refresh_url: &str,
    ) -> impl std::future::Future<Output = Result<ConnectAccountResult, PaymentError>> + Send;

    /// Create a checkout session for a viewer payment.
    fn create_checkout_session(
        &self,
        params: CheckoutParams,
    ) -> impl std::future::Future<Output = Result<CheckoutResult, PaymentError>> + Send;

    /// Verify a webhook signature and parse the event.
    fn verify_webhook(&self, payload: &[u8], signature: &str)
    -> Result<WebhookEvent, PaymentError>;

    /// Provider name (e.g., "stripe", "mock").
    fn name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct ConnectAccountResult {
    pub account_id: String,
    pub onboarding_url: String,
}

#[derive(Debug, Clone)]
pub struct CheckoutParams {
    pub connected_account_id: String,
    pub line_items: Vec<LineItem>,
    pub success_url: String,
    pub cancel_url: String,
    pub metadata: std::collections::HashMap<String, String>,
    pub application_fee_pct: f64,
}

#[derive(Debug, Clone)]
pub struct LineItem {
    pub name: String,
    pub description: String,
    pub amount_cents: i64,
    pub currency: String,
    pub quantity: i64,
}

#[derive(Debug, Clone)]
pub struct CheckoutResult {
    pub session_id: String,
    pub checkout_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub event_type: String,
    pub external_id: String,
    pub data: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum PaymentError {
    #[error("Payment provider not configured")]
    NotConfigured,
    #[error("Provider error: {0}")]
    Provider(String),
    #[error("Invalid webhook: {0}")]
    InvalidWebhook(String),
}

/// No-op provider for self-hosted instances without payment processing.
pub struct NoopProvider;

impl PaymentProvider for NoopProvider {
    async fn create_connect_account(
        &self,
        _platform_name: &str,
        _return_url: &str,
        _refresh_url: &str,
    ) -> Result<ConnectAccountResult, PaymentError> {
        Err(PaymentError::NotConfigured)
    }

    async fn create_checkout_session(
        &self,
        _params: CheckoutParams,
    ) -> Result<CheckoutResult, PaymentError> {
        Err(PaymentError::NotConfigured)
    }

    fn verify_webhook(
        &self,
        _payload: &[u8],
        _signature: &str,
    ) -> Result<WebhookEvent, PaymentError> {
        Err(PaymentError::NotConfigured)
    }

    fn name(&self) -> &str {
        "none"
    }
}
