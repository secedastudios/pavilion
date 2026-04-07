use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Platform {
    pub id: RecordId,
    pub name: String,
    pub slug: String,
    pub domain: Option<String>,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub theme: Option<PlatformTheme>,
    pub monetization_model: Option<String>,
    pub subscription_price_cents: Option<i64>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone, Default)]
pub struct PlatformTheme {
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub accent_color: Option<String>,
    pub font_heading: Option<String>,
    pub font_body: Option<String>,
    pub border_radius: Option<String>,
    pub dark_mode: Option<bool>,
}

impl PlatformTheme {
    /// Generate CSS custom property overrides for this theme.
    pub fn to_css_overrides(&self) -> String {
        let mut props = Vec::new();
        if let Some(c) = &self.primary_color { props.push(format!("--color-primary: {c};")); }
        if let Some(c) = &self.secondary_color { props.push(format!("--color-secondary: {c};")); }
        if let Some(c) = &self.accent_color { props.push(format!("--color-accent: {c};")); }
        if let Some(f) = &self.font_heading { props.push(format!("--font-heading: {f};")); }
        if let Some(f) = &self.font_body { props.push(format!("--font-body: {f};")); }
        if let Some(r) = &self.border_radius { props.push(format!("--radius-md: {r};")); }
        if self.dark_mode == Some(true) {
            props.push("--color-background: #0f172a;".into());
            props.push("--color-surface: #1e293b;".into());
            props.push("--color-text: #f1f5f9;".into());
            props.push("--color-text-muted: #94a3b8;".into());
            props.push("--color-border: #334155;".into());
        }
        if props.is_empty() { String::new() } else { format!(":root {{ {} }}", props.join(" ")) }
    }
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreatePlatform {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub monetization_model: Option<String>,
    pub status: String,
    pub theme: PlatformTheme,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlatformView {
    pub id: RecordId,
    pub key_str: String,
    pub name: String,
    pub slug: String,
    pub domain: Option<String>,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub theme: Option<PlatformTheme>,
    pub monetization_model: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

impl From<Platform> for PlatformView {
    fn from(p: Platform) -> Self {
        let key_str = crate::util::record_id_key_string(&p.id.key);
        Self {
            id: p.id,
            key_str,
            name: p.name,
            slug: p.slug,
            domain: p.domain,
            description: p.description,
            logo_url: p.logo_url,
            theme: p.theme,
            monetization_model: p.monetization_model,
            status: p.status,
            created_at: p.created_at,
        }
    }
}

/// A film carried by a platform (for content management views).
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct CarriedFilm {
    pub id: RecordId,
    pub position: Option<i64>,
    pub featured: bool,
    pub added_at: DateTime<Utc>,
}
