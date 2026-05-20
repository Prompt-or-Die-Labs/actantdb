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

    /// Feature recognized but not implemented on the active backend.
    #[error("not implemented: {0}")]
    NotImplemented(String),
}

impl From<serde_json::Error> for ActantError {
    fn from(value: serde_json::Error) -> Self {
        Self::InvalidInput(value.to_string())
    }
}

impl ActantError {
    /// Stable machine-readable code for public API clients.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Storage(_) => "storage_error",
            Self::InvalidInput(_) => "invalid_input",
            Self::PermissionDenied(_) => "permission_denied",
            Self::ApprovalRequired(_) => "approval_required",
            Self::ApprovalDenied(_) => "approval_denied",
            Self::NotFound(_) => "not_found",
            Self::Conflict(_) => "conflict",
            Self::IdempotentReplay(_) => "idempotent_replay",
            Self::PolicyHalt(_) => "policy_halt",
            Self::Internal(_) => "internal_error",
            Self::NotImplemented(_) => "not_implemented",
        }
    }

    /// Short operator-facing hint suitable for SDK and CLI output.
    pub fn hint(&self) -> &'static str {
        match self {
            Self::Storage(_) => "The ledger backend rejected the operation.",
            Self::InvalidInput(_) => "Check the request payload and required fields.",
            Self::PermissionDenied(_) => {
                "Grant a matching authority scope or use an actor with permission."
            }
            Self::ApprovalRequired(_) => {
                "Approve the pending request before replaying the command."
            }
            Self::ApprovalDenied(_) => "The command was reviewed and denied.",
            Self::NotFound(_) => "Check the identifier and workspace.",
            Self::Conflict(_) => "Reload the latest state and retry the operation.",
            Self::IdempotentReplay(_) => "The idempotency key already resolved to a prior command.",
            Self::PolicyHalt(_) => "The active policy halted this run.",
            Self::Internal(_) => "Open a bug with the request id and server logs.",
            Self::NotImplemented(_) => {
                "This feature is recognized but not available on this surface."
            }
        }
    }

    /// Optional direct fix when the next action is deterministic.
    pub fn fix(&self) -> Option<&'static str> {
        match self {
            Self::ApprovalRequired(_) => Some("Call approve_tool_call, then retry."),
            Self::InvalidInput(_) => {
                Some("Run `actantdb metadata commands` to inspect accepted command inputs.")
            }
            Self::NotImplemented(_) => {
                Some("Use the documented supported surface for this backend.")
            }
            _ => None,
        }
    }
}
