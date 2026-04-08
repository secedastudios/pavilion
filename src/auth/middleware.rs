use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::header::COOKIE;
use axum::http::request::Parts;

use crate::auth::claims::{Claims, verify_token};
use crate::error::AppError;
use crate::router::AppState;

/// Extractor that requires a valid JWT. Use in handler signatures to
/// enforce authentication.
impl FromRequestParts<Arc<AppState>> for Claims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        // Try Authorization: Bearer <token> header first
        if let Some(auth_header) = parts.headers.get("authorization")
            && let Ok(value) = auth_header.to_str()
            && let Some(token) = value.strip_prefix("Bearer ")
        {
            return verify_token(token.trim(), &state.config.jwt_secret)
                .map_err(|_| AppError::Unauthorized);
        }

        // Fall back to cookie
        if let Some(cookie_header) = parts.headers.get(COOKIE)
            && let Ok(cookies) = cookie_header.to_str()
        {
            for cookie in cookies.split(';') {
                let cookie = cookie.trim();
                if let Some(token) = cookie.strip_prefix("pavilion_token=") {
                    return verify_token(token.trim(), &state.config.jwt_secret)
                        .map_err(|_| AppError::Unauthorized);
                }
            }
        }

        Err(AppError::Unauthorized)
    }
}

/// Optional claims extractor — returns None if not authenticated instead of
/// rejecting. Useful for pages that work both logged-in and logged-out.
pub struct OptionalClaims(pub Option<Claims>);

impl FromRequestParts<Arc<AppState>> for OptionalClaims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        match Claims::from_request_parts(parts, state).await {
            Ok(claims) => Ok(OptionalClaims(Some(claims))),
            Err(_) => Ok(OptionalClaims(None)),
        }
    }
}
