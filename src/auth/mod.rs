//! Authentication and authorization.
//!
//! Handles the full auth lifecycle:
//!
//! - **Password hashing** with Argon2 (`password` module)
//! - **JWT token** issuance and verification (`claims` module)
//! - **Request extractors** that pull claims from cookies or Bearer headers (`middleware` module)
//!
//! # Authentication flow
//!
//! 1. Person registers or logs in — server verifies password with Argon2
//! 2. Server issues a JWT containing person ID, name, and roles
//! 3. JWT is stored as an HTTP-only cookie (`pavilion_token`)
//! 4. On every authenticated request, the `Claims` extractor reads the
//!    cookie or `Authorization: Bearer` header and validates the JWT
//! 5. Handlers receive `Claims` as a typed parameter — if missing, returns 401

/// JWT claims structure, token issuance, and verification.
pub mod claims;

/// Axum request extractors for authentication.
pub mod middleware;

/// Argon2 password hashing and verification.
pub mod password;
