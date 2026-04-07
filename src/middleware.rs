//! Security headers middleware for Axum responses.
//! Applies Content-Security-Policy, X-Frame-Options, X-Content-Type-Options,
//! and other hardening headers to every outgoing HTTP response.

use axum::http::{HeaderValue, Response};
use axum::middleware::Next;
use axum::extract::Request;

/// Security headers middleware.
pub async fn security_headers(
    request: Request,
    next: Next,
) -> Response<axum::body::Body> {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "x-frame-options",
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        "x-xss-protection",
        HeaderValue::from_static("1; mode=block"),
    );
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self' https://cdn.jsdelivr.net https://js.stripe.com 'unsafe-inline'; \
             style-src 'self' 'unsafe-inline'; \
             img-src 'self' data: https:; \
             connect-src 'self' https://api.stripe.com; \
             frame-src https://js.stripe.com; \
             font-src 'self'"
        ),
    );

    response
}
