//! Filmmaker and curator revenue aggregation queries.
//! Provides overview statistics including total earnings, transaction counts,
//! and per-film breakdowns for use in revenue dashboards.

use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

use crate::db::Db;

/// Revenue overview for a filmmaker across all their films.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FilmmakerRevenueOverview {
    pub total_earned_cents: i64,
    pub total_transactions: i64,
    pub by_film: Vec<FilmRevenue>,
    pub by_platform: Vec<PlatformRevenue>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct FilmRevenue {
    pub film: RecordId,
    pub total_cents: i64,
    pub transaction_count: i64,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct PlatformRevenue {
    pub platform: RecordId,
    pub total_cents: i64,
    pub transaction_count: i64,
}

/// Revenue overview for a curator's platform.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PlatformRevenueOverview {
    pub total_revenue_cents: i64,
    pub curator_share_cents: i64,
    pub total_transactions: i64,
    pub subscriber_count: i64,
    pub total_views: i64,
}

/// Get filmmaker revenue overview (sums from revenue_split where role = 'filmmaker').
pub async fn filmmaker_revenue(
    db: &Db,
    person_id: &RecordId,
) -> Result<FilmmakerRevenueOverview, surrealdb::Error> {
    // Total earned
    #[derive(Deserialize, SurrealValue)]
    struct TotalRow {
        total: Option<i64>,
        count: Option<i64>,
    }

    let totals: Vec<TotalRow> = db
        .query(
            "SELECT math::sum(amount_cents) AS total, count() AS count \
             FROM revenue_split WHERE recipient = $person AND role = 'filmmaker'",
        )
        .bind(("person", person_id.clone()))
        .await?
        .take(0)?;

    let (total_earned, total_txns) = totals
        .into_iter()
        .next()
        .map(|r| (r.total.unwrap_or(0), r.count.unwrap_or(0)))
        .unwrap_or((0, 0));

    // Aggregate by film
    let by_film: Vec<FilmRevenue> = db
        .query(
            "SELECT transaction.film AS film, math::sum(amount_cents) AS total_cents, count() AS transaction_count \
             FROM revenue_split WHERE recipient = $person AND role = 'filmmaker' \
             GROUP BY transaction.film"
        )
        .bind(("person", person_id.clone()))
        .await?
        .take(0)
        .unwrap_or_default();

    // Aggregate by platform
    let by_platform: Vec<PlatformRevenue> = db
        .query(
            "SELECT transaction.platform AS platform, math::sum(amount_cents) AS total_cents, count() AS transaction_count \
             FROM revenue_split WHERE recipient = $person AND role = 'filmmaker' \
             GROUP BY transaction.platform"
        )
        .bind(("person", person_id.clone()))
        .await?
        .take(0)
        .unwrap_or_default();

    Ok(FilmmakerRevenueOverview {
        total_earned_cents: total_earned,
        total_transactions: total_txns,
        by_film,
        by_platform,
    })
}

/// Get platform revenue overview for a curator.
pub async fn platform_revenue(
    db: &Db,
    platform_id: &RecordId,
) -> Result<PlatformRevenueOverview, surrealdb::Error> {
    #[derive(Deserialize, SurrealValue)]
    struct TotalRow {
        total: Option<i64>,
        count: Option<i64>,
    }

    // Total platform revenue
    let totals: Vec<TotalRow> = db
        .query(
            "SELECT math::sum(amount_cents) AS total, count() AS count \
             FROM transaction WHERE platform = $platform AND status = 'completed'",
        )
        .bind(("platform", platform_id.clone()))
        .await?
        .take(0)?;

    let (total_rev, total_txns) = totals
        .into_iter()
        .next()
        .map(|r| (r.total.unwrap_or(0), r.count.unwrap_or(0)))
        .unwrap_or((0, 0));

    // Curator's share
    let curator_totals: Vec<TotalRow> = db
        .query(
            "SELECT math::sum(amount_cents) AS total, count() AS count \
             FROM revenue_split WHERE role = 'curator' \
               AND transaction.platform = $platform",
        )
        .bind(("platform", platform_id.clone()))
        .await?
        .take(0)?;

    let curator_share = curator_totals
        .into_iter()
        .next()
        .and_then(|r| r.total)
        .unwrap_or(0);

    // Subscriber count
    #[derive(Deserialize, SurrealValue)]
    struct CountRow {
        count: Option<i64>,
    }

    let subs: Vec<CountRow> = db
        .query(
            "SELECT count() AS count FROM viewer_subscription \
             WHERE platform = $platform AND status = 'active'",
        )
        .bind(("platform", platform_id.clone()))
        .await?
        .take(0)?;

    let sub_count = subs.into_iter().next().and_then(|r| r.count).unwrap_or(0);

    // Total views
    let views: Vec<CountRow> = db
        .query("SELECT count() AS count FROM watch_session WHERE platform = $platform")
        .bind(("platform", platform_id.clone()))
        .await?
        .take(0)?;

    let view_count = views.into_iter().next().and_then(|r| r.count).unwrap_or(0);

    Ok(PlatformRevenueOverview {
        total_revenue_cents: total_rev,
        curator_share_cents: curator_share,
        total_transactions: total_txns,
        subscriber_count: sub_count,
        total_views: view_count,
    })
}
