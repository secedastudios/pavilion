//! Curator credit balance and transaction management.
//! Tracks credit balances for curator accounts and records credit
//! transactions for purchases, grants, and consumption.

use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

use crate::db::Db;
use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct CreditBalance {
    pub id: RecordId,
    pub person: RecordId,
    pub balance_cents: i64,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct CreditTransaction {
    pub id: RecordId,
    pub person: RecordId,
    pub amount_cents: i64,
    pub transaction_type: String,
    pub description: Option<String>,
}

/// Get credit balance for a person (creates if not exists).
pub async fn get_balance(db: &Db, person_id: &RecordId) -> Result<i64, surrealdb::Error> {
    let rows: Vec<CreditBalance> = db
        .query("SELECT * FROM credit_balance WHERE person = $person LIMIT 1")
        .bind(("person", person_id.clone()))
        .await?
        .take(0)?;

    if let Some(balance) = rows.into_iter().next() {
        return Ok(balance.balance_cents);
    }

    // Create with zero balance
    db.query("CREATE credit_balance SET person = $person, balance_cents = 0")
        .bind(("person", person_id.clone()))
        .await?;

    Ok(0)
}

/// Add credits (from a purchase).
pub async fn add_credits(
    db: &Db,
    person_id: &RecordId,
    amount_cents: i64,
    description: &str,
) -> Result<i64, AppError> {
    db.query("UPDATE credit_balance SET balance_cents += $amount WHERE person = $person")
        .bind(("amount", amount_cents))
        .bind(("person", person_id.clone()))
        .await?;

    db.query(
        "CREATE credit_transaction SET \
            person = $person, amount_cents = $amount, \
            transaction_type = 'purchase', description = $desc",
    )
    .bind(("person", person_id.clone()))
    .bind(("amount", amount_cents))
    .bind(("desc", description.to_string()))
    .await?;

    get_balance(db, person_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Credit balance error: {e}")))
}

/// Deduct credits (for a film acquisition or service).
pub async fn deduct_credits(
    db: &Db,
    person_id: &RecordId,
    amount_cents: i64,
    description: &str,
) -> Result<i64, AppError> {
    let balance = get_balance(db, person_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Credit balance error: {e}")))?;

    if balance < amount_cents {
        return Err(AppError::Validation(format!(
            "Insufficient credits. Balance: ${:.2}, required: ${:.2}",
            balance as f64 / 100.0,
            amount_cents as f64 / 100.0
        )));
    }

    db.query("UPDATE credit_balance SET balance_cents -= $amount WHERE person = $person")
        .bind(("amount", amount_cents))
        .bind(("person", person_id.clone()))
        .await?;

    db.query(
        "CREATE credit_transaction SET \
            person = $person, amount_cents = $amount, \
            transaction_type = 'deduction', description = $desc",
    )
    .bind(("person", person_id.clone()))
    .bind(("amount", amount_cents))
    .bind(("desc", description.to_string()))
    .await?;

    get_balance(db, person_id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Credit balance error: {e}")))
}

/// Get transaction history for a person.
pub async fn transaction_history(
    db: &Db,
    person_id: &RecordId,
) -> Result<Vec<CreditTransaction>, surrealdb::Error> {
    let txns: Vec<CreditTransaction> = db
        .query("SELECT * FROM credit_transaction WHERE person = $person ORDER BY created_at DESC LIMIT 50")
        .bind(("person", person_id.clone()))
        .await?
        .take(0)?;
    Ok(txns)
}
