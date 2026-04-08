use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, SurrealValue};

/// A streaming platform (channel, festival hub, or branded site) that carries films.
///
/// Each platform has its own branding (`theme`), optional custom domain, and a
/// monetization model that determines how films are offered to viewers.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct Platform {
    pub id: RecordId,
    pub name: String,
    /// URL-safe identifier used in routes (e.g. `/platforms/my-channel`).
    pub slug: String,
    /// Custom domain for white-label access (e.g. `"films.example.com"`).
    pub domain: Option<String>,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    /// Visual theme overrides (colors, fonts, dark mode).
    pub theme: Option<PlatformTheme>,
    /// Business model: `"tvod"`, `"svod"`, `"avod"`, `"hybrid"`, `"free"`, etc.
    pub monetization_model: Option<String>,
    /// Monthly subscription price in cents (relevant for SVOD platforms).
    pub subscription_price_cents: Option<i64>,
    /// Lifecycle status: `"draft"`, `"active"`, `"suspended"`.
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Visual customization settings for a [`Platform`].
///
/// All fields are optional; unset values fall back to the global CSS defaults.
/// Call [`to_css_overrides`](PlatformTheme::to_css_overrides) to generate a
/// `:root { ... }` block of CSS custom properties.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone, Default)]
pub struct PlatformTheme {
    /// CSS color value for the primary brand color.
    pub primary_color: Option<String>,
    /// CSS color value for the secondary brand color.
    pub secondary_color: Option<String>,
    /// CSS color value for accent/highlight elements.
    pub accent_color: Option<String>,
    /// Font family for headings (e.g. `"Inter, sans-serif"`).
    pub font_heading: Option<String>,
    /// Font family for body text.
    pub font_body: Option<String>,
    /// CSS border-radius value (e.g. `"0.5rem"`).
    pub border_radius: Option<String>,
    /// Enable dark mode color scheme overrides.
    pub dark_mode: Option<bool>,
}

impl PlatformTheme {
    /// Generate CSS custom property overrides for this theme.
    pub fn to_css_overrides(&self) -> String {
        let mut props = Vec::new();
        if let Some(c) = &self.primary_color {
            props.push(format!("--color-primary: {c};"));
        }
        if let Some(c) = &self.secondary_color {
            props.push(format!("--color-secondary: {c};"));
        }
        if let Some(c) = &self.accent_color {
            props.push(format!("--color-accent: {c};"));
        }
        if let Some(f) = &self.font_heading {
            props.push(format!("--font-heading: {f};"));
        }
        if let Some(f) = &self.font_body {
            props.push(format!("--font-body: {f};"));
        }
        if let Some(r) = &self.border_radius {
            props.push(format!("--radius-md: {r};"));
        }
        if self.dark_mode == Some(true) {
            props.push("--color-background: #0f172a;".into());
            props.push("--color-surface: #1e293b;".into());
            props.push("--color-text: #f1f5f9;".into());
            props.push("--color-text-muted: #94a3b8;".into());
            props.push("--color-border: #334155;".into());
        }
        if props.is_empty() {
            String::new()
        } else {
            format!(":root {{ {} }}", props.join(" "))
        }
    }
}

/// Payload for creating a new [`Platform`] record.
#[derive(Debug, Serialize, Deserialize, SurrealValue)]
pub struct CreatePlatform {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub monetization_model: Option<String>,
    pub status: String,
    pub theme: PlatformTheme,
}

/// Template-safe projection of [`Platform`] with a `key_str` for URL rendering.
///
/// Excludes `subscription_price_cents` and `updated_at`.
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

/// Edge data on the `carries` graph relation (Platform -> Film).
///
/// Represents a film that has been added to a platform's catalog,
/// with optional ordering and feature-flag metadata.
#[derive(Debug, Serialize, Deserialize, SurrealValue, Clone)]
pub struct CarriedFilm {
    pub id: RecordId,
    /// Sort order within the platform's catalog. `None` means unordered.
    pub position: Option<i64>,
    /// Whether this film is promoted on the platform's homepage.
    pub featured: bool,
    pub added_at: DateTime<Utc>,
}
