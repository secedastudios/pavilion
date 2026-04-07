//! Pricing tier definitions and tier recommendation logic.
//! Defines the available billing plans with their storage and feature limits,
//! and recommends appropriate tiers based on current usage patterns.

use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

use crate::billing::metering::StorageUsage;
use crate::db::Db;

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct PricingTier {
    pub id: RecordId,
    pub name: String,
    pub max_storage_bytes: i64,
    pub max_films: i64,
    pub price_cents_monthly: i64,
    pub active: bool,
}

/// Default pricing tiers (seeded on first run or via db-seed).
pub fn default_tiers() -> Vec<TierDef> {
    vec![
        TierDef {
            name: "Free".into(),
            max_storage_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
            max_films: 1,
            price_cents_monthly: 0,
        },
        TierDef {
            name: "Starter".into(),
            max_storage_bytes: 50 * 1024 * 1024 * 1024, // 50 GB
            max_films: 5,
            price_cents_monthly: 999, // $9.99
        },
        TierDef {
            name: "Pro".into(),
            max_storage_bytes: 250 * 1024 * 1024 * 1024, // 250 GB
            max_films: 25,
            price_cents_monthly: 2999, // $29.99
        },
        TierDef {
            name: "Studio".into(),
            max_storage_bytes: 1024 * 1024 * 1024 * 1024, // 1 TB
            max_films: 100,
            price_cents_monthly: 9999, // $99.99
        },
    ]
}

pub struct TierDef {
    pub name: String,
    pub max_storage_bytes: i64,
    pub max_films: i64,
    pub price_cents_monthly: i64,
}

/// Get all active pricing tiers.
pub async fn list_tiers(db: &Db) -> Result<Vec<PricingTier>, surrealdb::Error> {
    let tiers: Vec<PricingTier> = db
        .query("SELECT * FROM pricing_tier WHERE active = true ORDER BY price_cents_monthly ASC")
        .await?
        .take(0)?;
    Ok(tiers)
}

/// Determine which tier a filmmaker's usage falls into (first tier that fits).
pub fn recommended_tier(usage: &StorageUsage, tiers: &[PricingTier]) -> Option<PricingTier> {
    tiers
        .iter()
        .find(|t| usage.total_bytes <= t.max_storage_bytes && usage.film_count <= t.max_films)
        .cloned()
}

/// Check if usage exceeds the given tier limits.
pub fn exceeds_tier(usage: &StorageUsage, tier: &PricingTier) -> bool {
    usage.total_bytes > tier.max_storage_bytes || usage.film_count > tier.max_films
}

/// Calculate estimated monthly cost based on usage and tiers.
pub fn estimate_monthly_cost(usage: &StorageUsage, tiers: &[PricingTier]) -> i64 {
    recommended_tier(usage, tiers)
        .map(|t| t.price_cents_monthly)
        .unwrap_or_else(|| tiers.last().map(|t| t.price_cents_monthly).unwrap_or(0))
}
