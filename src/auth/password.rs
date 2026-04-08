//! Argon2id password hashing and verification.
//!
//! Uses the default Argon2 parameters which are tuned for a balance of
//! security and performance. Each hash includes a random salt generated
//! from the OS CSPRNG.

use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};

/// Hash a plaintext password using Argon2id with a random salt.
///
/// The returned string is in PHC format and can be stored directly in
/// the database. It includes the algorithm, parameters, salt, and hash
/// so verification is self-contained.
///
/// # Errors
///
/// Returns an error if the Argon2 hasher fails (e.g., out of memory
/// for the requested parameters — extremely unlikely with defaults).
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string();
    Ok(hash)
}

/// Verify a plaintext password against a stored Argon2 hash.
///
/// Returns `Ok(true)` if the password matches, `Ok(false)` if it doesn't.
/// Returns `Err` only if the stored hash is malformed (not a valid PHC string).
pub fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    let parsed_hash = PasswordHash::new(hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}
