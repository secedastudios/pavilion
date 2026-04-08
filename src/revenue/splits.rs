//! Transaction creation with three-way revenue splits.
//! Records each payment as a transaction divided between the filmmaker,
//! curator (platform owner), and the Pavilion platform fee.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

use crate::db::Db;
use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Transaction {
    pub id: RecordId,
    pub transaction_type: String,
    pub amount_cents: i64,
    pub currency: String,
    pub film: Option<RecordId>,
    pub platform: RecordId,
    pub person: Option<RecordId>,
    pub external_id: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateTransaction {
    pub transaction_type: String,
    pub amount_cents: i64,
    pub currency: String,
    pub film: Option<RecordId>,
    pub platform: RecordId,
    pub person: Option<RecordId>,
    pub external_id: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct RevenueSplit {
    pub id: RecordId,
    pub transaction: RecordId,
    pub recipient: RecordId,
    pub role: String,
    pub amount_cents: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateRevenueSplit {
    pub transaction: RecordId,
    pub recipient: RecordId,
    pub role: String,
    pub amount_cents: i64,
}

/// Parameters for recording a transaction with revenue splits.
///
/// Used by [`record_transaction`] to capture all the details of a payment
/// event and how the revenue should be divided.
pub struct RecordTransactionParams {
    /// Type of transaction: `"rental"`, `"purchase"`, `"subscription"`, etc.
    pub transaction_type: String,
    /// Total payment amount in cents.
    pub amount_cents: i64,
    /// ISO 4217 currency code (e.g., `"usd"`).
    pub currency: String,
    /// The film involved, if applicable.
    pub film_id: Option<RecordId>,
    /// The platform where the transaction occurred.
    pub platform_id: RecordId,
    /// The person who made the payment.
    pub buyer_id: Option<RecordId>,
    /// External reference (e.g., Stripe charge ID) for auditability.
    pub external_id: Option<String>,
    /// Percentage of the transaction kept as a platform fee (0.0 to disable).
    pub facilitation_fee_pct: f64,
    /// The filmmaker who should receive their share.
    pub filmmaker_id: Option<RecordId>,
    /// Filmmaker's share as a percentage of the post-fee amount.
    pub filmmaker_share_pct: Option<f64>,
}

/// Record a transaction and calculate the 3-way revenue split.
///
/// Splits the `amount_cents` into:
/// 1. **Platform fee** — `facilitation_fee_pct` of the total
/// 2. **Filmmaker share** — `filmmaker_share_pct` of the remainder
/// 3. **Curator share** — whatever is left
///
/// Each split is stored as a separate `revenue_split` record linked
/// to the transaction for full auditability.
///
/// # Errors
///
/// Returns [`AppError`] if any database operation fails.
pub async fn record_transaction(
    db: &Db,
    params: RecordTransactionParams,
) -> Result<Transaction, AppError> {
    // Create the transaction
    let txn: Option<Transaction> = db
        .create("transaction")
        .content(CreateTransaction {
            transaction_type: params.transaction_type,
            amount_cents: params.amount_cents,
            currency: params.currency,
            film: params.film_id,
            platform: params.platform_id.clone(),
            person: params.buyer_id,
            external_id: params.external_id,
            status: "completed".to_string(),
        })
        .await?;

    let txn =
        txn.ok_or_else(|| AppError::Internal(anyhow::anyhow!("Failed to create transaction")))?;

    // Calculate splits
    let fee_amount =
        ((params.amount_cents as f64) * params.facilitation_fee_pct / 100.0).round() as i64;
    let after_fee = params.amount_cents - fee_amount;

    let filmmaker_amount = if let Some(pct) = params.filmmaker_share_pct {
        ((after_fee as f64) * pct / 100.0).round() as i64
    } else {
        0
    };
    let curator_amount = after_fee - filmmaker_amount;

    // Record platform fee split (if > 0)
    if fee_amount > 0 {
        let _: Option<RevenueSplit> = db
            .create("revenue_split")
            .content(CreateRevenueSplit {
                transaction: txn.id.clone(),
                recipient: params.platform_id.clone(),
                role: "platform_fee".to_string(),
                amount_cents: fee_amount,
            })
            .await?;
    }

    // Record filmmaker split
    if let Some(fm_id) = params.filmmaker_id
        && filmmaker_amount > 0
    {
        let _: Option<RevenueSplit> = db
            .create("revenue_split")
            .content(CreateRevenueSplit {
                transaction: txn.id.clone(),
                recipient: fm_id,
                role: "filmmaker".to_string(),
                amount_cents: filmmaker_amount,
            })
            .await?;
    }

    // Record curator split (whoever owns the platform)
    // Find curator via graph
    let curators: Vec<serde_json::Value> = db
        .query("SELECT in FROM curator_of WHERE out = $platform AND role = 'owner' LIMIT 1")
        .bind(("platform", params.platform_id))
        .await?
        .take(0)?;

    if let Some(curator) = curators.first()
        && let Some(curator_id_str) = curator["in"].as_str()
    {
        // Parse the RecordId from the string
        if curator_amount > 0 {
            let _: Option<RevenueSplit> = db
                .create("revenue_split")
                .content(CreateRevenueSplit {
                    transaction: txn.id.clone(),
                    recipient: RecordId::new("person", curator_id_str),
                    role: "curator".to_string(),
                    amount_cents: curator_amount,
                })
                .await?;
        }
    }

    tracing::info!(
        txn_type = %txn.transaction_type,
        amount = txn.amount_cents,
        fee = fee_amount,
        filmmaker = filmmaker_amount,
        curator = curator_amount,
        "Transaction recorded with splits"
    );

    Ok(txn)
}
