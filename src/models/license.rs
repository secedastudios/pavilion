use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

/// A distribution license defining how a [`Film`](super::film::Film) may be offered on a platform.
///
/// This is a union struct: which fields are relevant depends on `license_type`.
/// Supported types: `"tvod"`, `"svod"`, `"avod"`, `"hybrid"`, `"event"`,
/// `"educational"`, `"cc"`. Use [`validate_license`] to enforce type-specific
/// required fields before creation.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct License {
    pub id: RecordId,
    /// Discriminator: `"tvod"`, `"svod"`, `"avod"`, `"hybrid"`, `"event"`, `"educational"`, `"cc"`.
    pub license_type: String,
    /// ISO 3166-1 territory codes where the license applies.
    pub territories: Vec<String>,
    /// Start of the availability window (inclusive).
    pub window_start: Option<DateTime<Utc>>,
    /// End of the availability window (inclusive). `None` means open-ended.
    pub window_end: Option<DateTime<Utc>>,
    /// Whether a platform must request approval before carrying the film.
    pub approval_required: bool,
    /// Whether this license is currently in effect.
    pub active: bool,
    // TVOD
    /// Rental price in cents (TVOD).
    pub rental_price_cents: Option<i64>,
    /// How many hours a rental lasts (TVOD).
    pub rental_duration_hours: Option<i64>,
    /// One-time purchase price in cents (TVOD).
    pub purchase_price_cents: Option<i64>,
    // SVOD / AVOD
    /// Monthly flat fee in cents paid to the filmmaker (SVOD).
    pub flat_fee_monthly_cents: Option<i64>,
    /// Percentage of revenue shared with the filmmaker (SVOD/AVOD), 0-100.
    pub revenue_share_pct: Option<f64>,
    // Event
    /// Flat screening fee in cents (Event).
    pub event_flat_fee_cents: Option<i64>,
    /// Percentage of ticket revenue shared with the filmmaker (Event), 0-100.
    pub ticket_split_pct: Option<f64>,
    /// Maximum number of attendees allowed per screening (Event).
    pub max_attendees: Option<i64>,
    // Educational
    /// Allowed institution types, e.g. `["university", "library"]` (Educational).
    pub institution_types: Option<Vec<String>>,
    /// Named pricing tier such as `"small"`, `"medium"`, `"large"` (Educational).
    pub pricing_tier: Option<String>,
    // Creative Commons
    /// CC license variant, e.g. `"BY"`, `"BY-SA"`, `"BY-NC"` (Creative Commons).
    pub cc_license_type: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Payload for creating a new [`License`]. Validate with [`validate_license`] before insertion.
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreateLicense {
    pub license_type: String,
    pub territories: Vec<String>,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
    pub approval_required: bool,
    pub active: bool,
    // TVOD
    pub rental_price_cents: Option<i64>,
    pub rental_duration_hours: Option<i64>,
    pub purchase_price_cents: Option<i64>,
    // SVOD / AVOD
    pub flat_fee_monthly_cents: Option<i64>,
    pub revenue_share_pct: Option<f64>,
    // Event
    pub event_flat_fee_cents: Option<i64>,
    pub ticket_split_pct: Option<f64>,
    pub max_attendees: Option<i64>,
    // Educational
    pub institution_types: Option<Vec<String>>,
    pub pricing_tier: Option<String>,
    // Creative Commons
    pub cc_license_type: Option<String>,
}

/// Template-safe projection of [`License`] with pre-computed display strings.
///
/// Includes `license_type_label` (human-readable name) and `pricing_summary`
/// (formatted pricing for the license type) so templates need no business logic.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LicenseView {
    pub id: RecordId,
    /// String representation of the record key for use in URLs and templates.
    pub key_str: String,
    /// Raw license type discriminator (e.g. `"tvod"`).
    pub license_type: String,
    /// Human-readable label, e.g. `"Transactional (TVOD)"`.
    pub license_type_label: String,
    pub territories: Vec<String>,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
    pub approval_required: bool,
    pub active: bool,
    /// Pre-formatted pricing text, e.g. `"Rent: $3.99 / 48h | Buy: $9.99"`.
    pub pricing_summary: String,
    pub created_at: DateTime<Utc>,
}

impl From<License> for LicenseView {
    fn from(l: License) -> Self {
        let key_str = crate::util::record_id_key_string(&l.id.key);
        let label = license_type_label(&l.license_type);
        let pricing = pricing_summary(&l);
        Self {
            id: l.id,
            key_str,
            license_type: l.license_type,
            license_type_label: label,
            territories: l.territories,
            window_start: l.window_start,
            window_end: l.window_end,
            approval_required: l.approval_required,
            active: l.active,
            pricing_summary: pricing,
            created_at: l.created_at,
        }
    }
}

fn license_type_label(t: &str) -> String {
    match t {
        "tvod" => "Transactional (TVOD)".into(),
        "svod" => "Subscription (SVOD)".into(),
        "avod" => "Ad-Supported (AVOD)".into(),
        "hybrid" => "Hybrid / Split-Rights".into(),
        "event" => "Event / Screening".into(),
        "educational" => "Educational / Institutional".into(),
        "cc" => "Creative Commons".into(),
        other => other.to_string(),
    }
}

fn pricing_summary(l: &License) -> String {
    match l.license_type.as_str() {
        "tvod" => {
            let mut parts = Vec::new();
            if let Some(rent) = l.rental_price_cents {
                let hours = l.rental_duration_hours.unwrap_or(48);
                parts.push(format!("Rent: ${:.2} / {}h", rent as f64 / 100.0, hours));
            }
            if let Some(buy) = l.purchase_price_cents {
                parts.push(format!("Buy: ${:.2}", buy as f64 / 100.0));
            }
            if parts.is_empty() {
                "TVOD (pricing TBD)".into()
            } else {
                parts.join(" | ")
            }
        }
        "svod" => {
            if let Some(fee) = l.flat_fee_monthly_cents {
                format!("Flat fee: ${:.2}/mo", fee as f64 / 100.0)
            } else if let Some(pct) = l.revenue_share_pct {
                format!("{:.0}% revenue share", pct)
            } else {
                "SVOD (terms TBD)".into()
            }
        }
        "avod" => {
            if let Some(pct) = l.revenue_share_pct {
                format!("{:.0}% ad revenue share", pct)
            } else {
                "AVOD (terms TBD)".into()
            }
        }
        "event" => {
            if let Some(fee) = l.event_flat_fee_cents {
                format!("Flat fee: ${:.2}", fee as f64 / 100.0)
            } else if let Some(pct) = l.ticket_split_pct {
                format!("{:.0}% ticket split", pct)
            } else {
                "Event (terms TBD)".into()
            }
        }
        "educational" => l
            .pricing_tier
            .clone()
            .unwrap_or_else(|| "Educational (terms TBD)".into()),
        "cc" => l
            .cc_license_type
            .clone()
            .unwrap_or_else(|| "Creative Commons".into()),
        "hybrid" => "Hybrid / Split-Rights".into(),
        _ => "Unknown".into(),
    }
}

/// Validate that a [`CreateLicense`] has the required fields for its `license_type`.
///
/// Each license type has different mandatory pricing/config fields. This function
/// also checks that percentage values (`revenue_share_pct`, `ticket_split_pct`)
/// fall within the 0-100 range.
///
/// # Errors
///
/// Returns an `Err(String)` describing the first validation failure encountered,
/// or if the `license_type` is unrecognized.
pub fn validate_license(l: &CreateLicense) -> Result<(), String> {
    match l.license_type.as_str() {
        "tvod" => {
            if l.rental_price_cents.is_none() && l.purchase_price_cents.is_none() {
                return Err("TVOD requires at least a rental or purchase price.".into());
            }
            if l.rental_price_cents.is_some() && l.rental_duration_hours.is_none() {
                return Err("TVOD rental requires a duration in hours.".into());
            }
        }
        "svod" => {
            if l.flat_fee_monthly_cents.is_none() && l.revenue_share_pct.is_none() {
                return Err(
                    "SVOD requires either a flat monthly fee or revenue share percentage.".into(),
                );
            }
        }
        "avod" => {
            if l.revenue_share_pct.is_none() {
                return Err("AVOD requires a revenue share percentage.".into());
            }
        }
        "event" => {
            if l.event_flat_fee_cents.is_none() && l.ticket_split_pct.is_none() {
                return Err(
                    "Event licensing requires a flat fee or ticket split percentage.".into(),
                );
            }
        }
        "educational" => {}
        "cc" => {
            if l.cc_license_type.is_none() {
                return Err(
                    "Creative Commons requires a license type (e.g. BY, BY-SA, BY-NC).".into(),
                );
            }
        }
        "hybrid" => {}
        other => return Err(format!("Unknown license type: {other}")),
    }

    if let Some(pct) = l.revenue_share_pct
        && !(0.0..=100.0).contains(&pct)
    {
        return Err("Revenue share must be between 0 and 100.".into());
    }
    if let Some(pct) = l.ticket_split_pct
        && !(0.0..=100.0).contains(&pct)
    {
        return Err("Ticket split must be between 0 and 100.".into());
    }

    Ok(())
}
