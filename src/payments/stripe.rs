//! Stripe Connect API integration using `reqwest` for HTTP calls.
//! Implements the `PaymentProvider` trait with direct Stripe REST API usage,
//! including HMAC-SHA256 webhook signature verification.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::payments::provider::*;

type HmacSha256 = Hmac<Sha256>;

/// Stripe payment provider using direct API calls.
pub struct StripeProvider {
    secret_key: String,
    webhook_secret: String,
    client: reqwest::Client,
}

impl StripeProvider {
    pub fn new(secret_key: String, webhook_secret: String) -> Self {
        Self {
            secret_key,
            webhook_secret,
            client: reqwest::Client::new(),
        }
    }

    async fn stripe_post(
        &self,
        endpoint: &str,
        form: &[(&str, &str)],
    ) -> Result<serde_json::Value, PaymentError> {
        let url = format!("https://api.stripe.com/v1{endpoint}");
        let resp = self
            .client
            .post(&url)
            .basic_auth(&self.secret_key, Option::<&str>::None)
            .form(form)
            .send()
            .await
            .map_err(|e| PaymentError::Provider(format!("HTTP error: {e}")))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PaymentError::Provider(format!("JSON error: {e}")))?;

        if !status.is_success() {
            let msg = body["error"]["message"]
                .as_str()
                .unwrap_or("Unknown Stripe error");
            return Err(PaymentError::Provider(msg.to_string()));
        }

        Ok(body)
    }
}

impl PaymentProvider for StripeProvider {
    async fn create_connect_account(
        &self,
        platform_name: &str,
        return_url: &str,
        refresh_url: &str,
    ) -> Result<ConnectAccountResult, PaymentError> {
        // Create a connected account
        let account = self
            .stripe_post(
                "/accounts",
                &[
                    ("type", "express"),
                    ("business_profile[name]", platform_name),
                ],
            )
            .await?;

        let account_id = account["id"]
            .as_str()
            .ok_or_else(|| PaymentError::Provider("No account ID returned".into()))?
            .to_string();

        // Create an account onboarding link
        let link = self
            .stripe_post(
                "/account_links",
                &[
                    ("account", &account_id),
                    ("refresh_url", refresh_url),
                    ("return_url", return_url),
                    ("type", "account_onboarding"),
                ],
            )
            .await?;

        let url = link["url"]
            .as_str()
            .ok_or_else(|| PaymentError::Provider("No onboarding URL returned".into()))?
            .to_string();

        Ok(ConnectAccountResult {
            account_id,
            onboarding_url: url,
        })
    }

    async fn create_checkout_session(
        &self,
        params: CheckoutParams,
    ) -> Result<CheckoutResult, PaymentError> {
        let mut form: Vec<(&str, String)> = vec![
            ("mode", "payment".into()),
            ("success_url", params.success_url),
            ("cancel_url", params.cancel_url),
        ];

        for (i, item) in params.line_items.iter().enumerate() {
            form.push((&leak(format!("line_items[{i}][price_data][currency]")), item.currency.clone()));
            form.push((&leak(format!("line_items[{i}][price_data][unit_amount]")), item.amount_cents.to_string()));
            form.push((&leak(format!("line_items[{i}][price_data][product_data][name]")), item.name.clone()));
            form.push((&leak(format!("line_items[{i}][quantity]")), item.quantity.to_string()));
        }

        for (key, val) in &params.metadata {
            form.push((&leak(format!("metadata[{key}]")), val.clone()));
        }

        if params.application_fee_pct > 0.0 {
            let total: i64 = params.line_items.iter().map(|i| i.amount_cents * i.quantity).sum();
            let fee = ((total as f64) * params.application_fee_pct / 100.0).round() as i64;
            form.push(("payment_intent_data[application_fee_amount]", fee.to_string()));
        }

        let form_refs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let session = self
            .stripe_post("/checkout/sessions", &form_refs)
            .await?;

        let session_id = session["id"].as_str().unwrap_or_default().to_string();
        let checkout_url = session["url"].as_str().unwrap_or_default().to_string();

        Ok(CheckoutResult {
            session_id,
            checkout_url,
        })
    }

    fn verify_webhook(
        &self,
        payload: &[u8],
        signature: &str,
    ) -> Result<WebhookEvent, PaymentError> {
        // Parse Stripe-Signature header: t=timestamp,v1=signature
        let mut timestamp = "";
        let mut sig_v1 = "";
        for part in signature.split(',') {
            if let Some(t) = part.strip_prefix("t=") {
                timestamp = t;
            }
            if let Some(s) = part.strip_prefix("v1=") {
                sig_v1 = s;
            }
        }

        if timestamp.is_empty() || sig_v1.is_empty() {
            return Err(PaymentError::InvalidWebhook("Missing signature components".into()));
        }

        // Compute expected signature
        let signed_payload = format!("{timestamp}.{}", String::from_utf8_lossy(payload));
        let mut mac = HmacSha256::new_from_slice(self.webhook_secret.as_bytes())
            .map_err(|_| PaymentError::InvalidWebhook("HMAC key error".into()))?;
        mac.update(signed_payload.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());

        if !constant_time_eq(sig_v1.as_bytes(), expected.as_bytes()) {
            return Err(PaymentError::InvalidWebhook("Signature mismatch".into()));
        }

        // Parse the event
        let event: serde_json::Value = serde_json::from_slice(payload)
            .map_err(|e| PaymentError::InvalidWebhook(format!("JSON error: {e}")))?;

        Ok(WebhookEvent {
            event_type: event["type"].as_str().unwrap_or_default().to_string(),
            external_id: event["id"].as_str().unwrap_or_default().to_string(),
            data: event["data"]["object"].clone(),
        })
    }

    fn name(&self) -> &str {
        "stripe"
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Leak a string to get a &'static str for form fields.
/// This is used for dynamic Stripe form field names in a single request.
fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}
