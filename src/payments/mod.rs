//! Payment processing, provider abstraction, and viewer entitlements.
//! Wraps Stripe Connect for marketplace payments and provides a trait-based
//! provider interface so self-hosters can swap in alternative backends.

pub mod entitlements;
pub mod provider;
pub mod stripe;
