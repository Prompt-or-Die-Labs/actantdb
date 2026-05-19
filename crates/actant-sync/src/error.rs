//! [`SyncError`] — surfaces from destinations + runner.

use thiserror::Error;

/// Errors raised by a [`crate::Destination`] or the [`crate::SyncRunner`].
#[derive(Debug, Error)]
pub enum SyncError {
    /// The destination's persisted cursor did not match what the runner
    /// expected. Recoverable — the runner can re-query storage with the
    /// real cursor.
    #[error("cursor mismatch: destination has {persisted:?}, runner asked for {asked:?}")]
    CursorMismatch {
        /// What the destination knows about.
        persisted: Option<String>,
        /// What the runner thought was current.
        asked: Option<String>,
    },

    /// IO failure (filesystem read/write, file rename).
    #[error("io: {0}")]
    Io(String),

    /// Backend-specific failure (S3 returned 500, GCS auth refused, etc).
    /// The body is the upstream error message, no further structure assumed.
    #[error("backend ({backend}): {message}")]
    Backend {
        /// Destination name, e.g. `"s3"`, for log grouping.
        backend: String,
        /// Upstream error message.
        message: String,
    },

    /// Underlying storage query failed.
    #[error("storage: {0}")]
    Storage(String),

    /// Serialization of an event to JSON failed. Normally impossible because
    /// `AgentEvent` is `Serialize`, but surfaced as a soft error so the
    /// runner can record + skip rather than crash on a malformed row.
    #[error("serialize: {0}")]
    Serialize(String),

    /// Feature recognised but not compiled in (e.g. `s3` feature off).
    #[error("backend not available: {0}")]
    Unavailable(String),
}

impl From<std::io::Error> for SyncError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<serde_json::Error> for SyncError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialize(value.to_string())
    }
}

impl From<actant_objectstore::BlobError> for SyncError {
    fn from(value: actant_objectstore::BlobError) -> Self {
        Self::Backend {
            backend: "objectstore".into(),
            message: value.to_string(),
        }
    }
}

impl From<actant_core::ActantError> for SyncError {
    fn from(value: actant_core::ActantError) -> Self {
        Self::Storage(value.to_string())
    }
}
