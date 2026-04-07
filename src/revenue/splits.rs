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

/// Record a transaction and calculate revenue splits.
///
/// Split logic:
/// - Platform facilitation fee (configurable %)
/// - Filmmaker share per license terms (revenue_share_pct)
/// - Curator gets the remainder
pub async fn record_transaction(
    db: &Db,
    transaction_type: &str,
    amount_cents: i64,
    currency: &str,
    film_id: Option<RecordId>,
    platform_id: RecordId,
    buyer_id: Option<RecordId>,
    external_id: Option<String>,
    facilitation_fee_pct: f64,
    filmmaker_id: Option<RecordId>,
    filmmaker_share_pct: Option<f64>,
) -> Result<Transaction, AppError> {
    // Create the transaction
    let txn: Option<Transaction> = db
        .create("transaction")
        .content(CreateTransaction {
            transaction_type: transaction_type.to_string(),
            amount_cents,
            currency: currency.to_string(),
            film: film_id,
            platform: platform_id.clone(),
            person: buyer_id,
            external_id,
            status: "completed".to_string(),
        })
        .await?;

    let txn = txn.ok_or_else(|| AppError::Internal(anyhow::anyhow!("Failed to create transaction")))?;

    // Calculate splits
    let fee_amount = ((amount_cents as f64) * facilitation_fee_pct / 100.0).round() as i64;
    let after_fee = amount_cents - fee_amount;

    let filmmaker_amount = if let Some(pct) = filmmaker_share_pct {
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
                recipient: platform_id.clone(), // Fee goes to Pavilion operator
                role: "platform_fee".to_string(),
                amount_cents: fee_amount,
            })
            .await?;
    }

    // Record filmmaker split
    if let Some(fm_id) = filmmaker_id {
        if filmmaker_amount > 0 {
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
    }

    // Record curator split (whoever owns the platform)
    // Find curator via graph
    let curators: Vec<serde_json::Value> = db
        .query("SELECT in FROM curator_of WHERE out = $platform AND role = 'owner' LIMIT 1")
        .bind(("platform", platform_id))
        .await?
        .take(0)?;

    if let Some(curator) = curators.first() {
        if let Some(curator_id_str) = curator["in"].as_str() {
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
    }

    tracing::info!(
        txn_type = %transaction_type,
        amount = amount_cents,
        fee = fee_amount,
        filmmaker = filmmaker_amount,
        curator = curator_amount,
        "Transaction recorded with splits"
    );

    Ok(txn)
}
