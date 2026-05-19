//! actant-reliability — reliability primitives, gathered into a single crate
//! with one feature flag per primitive:
//!
//! * `throttle` — token-bucket rate limiter.
//! * `circuit`  — circuit breaker (closed / open / half-open).
//! * `lock`     — lease-based distributed locks backed by the `lock` table.
//! * `ingress`  — idempotent webhook / external event ingestion.
//!
//! All four are enabled by default. Disable [default-features] and re-enable
//! only what a caller needs to keep the dependency surface minimal.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "throttle")]
pub mod throttle;

#[cfg(feature = "circuit")]
pub mod circuit;

#[cfg(feature = "lock")]
pub mod lock;

#[cfg(feature = "ingress")]
pub mod ingress;
