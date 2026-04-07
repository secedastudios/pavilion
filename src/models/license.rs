use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct License {
    pub id: RecordId,
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

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

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

/// View model with string key for templates.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LicenseView {
    pub id: RecordId,
    pub key_str: String,
    pub license_type: String,
    pub license_type_label: String,
    pub territories: Vec<String>,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
    pub approval_required: bool,
    pub active: bool,
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
            if parts.is_empty() { "TVOD (pricing TBD)".into() } else { parts.join(" | ") }
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
        "educational" => {
            l.pricing_tier.clone().unwrap_or_else(|| "Educational (terms TBD)".into())
        }
        "cc" => {
            l.cc_license_type.clone().unwrap_or_else(|| "Creative Commons".into())
        }
        "hybrid" => "Hybrid / Split-Rights".into(),
        _ => "Unknown".into(),
    }
}

/// Validate that the license has the required fields for its type.
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
                return Err("SVOD requires either a flat monthly fee or revenue share percentage.".into());
            }
        }
        "avod" => {
            if l.revenue_share_pct.is_none() {
                return Err("AVOD requires a revenue share percentage.".into());
            }
        }
        "event" => {
            if l.event_flat_fee_cents.is_none() && l.ticket_split_pct.is_none() {
                return Err("Event licensing requires a flat fee or ticket split percentage.".into());
            }
        }
        "educational" => {}
        "cc" => {
            if l.cc_license_type.is_none() {
                return Err("Creative Commons requires a license type (e.g. BY, BY-SA, BY-NC).".into());
            }
        }
        "hybrid" => {}
        other => return Err(format!("Unknown license type: {other}")),
    }

    if let Some(pct) = l.revenue_share_pct {
        if !(0.0..=100.0).contains(&pct) {
            return Err("Revenue share must be between 0 and 100.".into());
        }
    }
    if let Some(pct) = l.ticket_split_pct {
        if !(0.0..=100.0).contains(&pct) {
            return Err("Ticket split must be between 0 and 100.".into());
        }
    }

    Ok(())
}
