//! actant-runtime — consolidated runtime primitives.
//!
//! Five previously separate crates were merged here behind feature flags so
//! the workspace can carry one observability surface instead of five. Each
//! module is gated by the feature of the same name and is enabled by
//! default. See the original sub-crate READMEs for design notes.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Tracing / OpenTelemetry helpers (W3C trace + span id minters).
#[cfg(feature = "trace")]
pub mod trace;

/// Content-keyed semantic cache.
#[cfg(feature = "cache")]
pub mod cache;

/// Prompt + template registry.
#[cfg(feature = "prompts")]
pub mod prompts;

/// Model registry + routing.
#[cfg(feature = "models")]
pub mod models;

/// Protocol adapters: MCP / A2A / AP2.
#[cfg(feature = "protocol")]
pub mod protocol;
