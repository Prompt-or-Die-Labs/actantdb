//! Top-level error type for the ActantDB substrate.

use thiserror::Error;

/// Errors surfaced through the public API.
#[derive(Debug, Error)]
pub enum ActantError {
    /// Storage-layer error (SQL, IO).
    #[error("storage error: {0}")]
    Storage(String),

    /// Command was rejected because input failed validation.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Command was rejected because the actor lacks authority.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Command was rejected because an approval is required and not granted.
    #[error("approval required: {0}")]
    ApprovalRequired(String),

    /// Command was rejected because an approval has been denied.
    #[error("approval denied: {0}")]
    ApprovalDenied(String),

    /// Referenced entity does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// Optimistic concurrency conflict.
    #[error("conflict: {0}")]
    Conflict(String),

    /// Idempotency replay: same key resolves to a previously committed result.
    #[error("idempotent replay: {0}")]
    IdempotentReplay(String),

    /// Policy halted the run.
    #[error("policy halt: {0}")]
    PolicyHalt(String),

    /// Internal invariant violation.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for ActantError {
    fn from(value: serde_json::Error) -> Self {
        Self::InvalidInput(value.to_string())
    }
}
