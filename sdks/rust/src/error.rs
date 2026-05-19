//! Errors surfaced by [`crate::ActantClient`].
//!
//! Mirrors the server's typed-error wire shape. The server uses the same
//! `{"error":"<kind>","message":"..."}` envelope for some 2xx codes
//! (`202 approval_required`, `200 idempotent_replay`); the client must peek
//! at the body's `error` field regardless of status code. See
//! `actant-server::err_response` and the Swift `ActantError.from(status:body:)`.
//!
//! Variant kinds:
//!
//! | server `error` key      | HTTP   | variant                |
//! |-------------------------|--------|------------------------|
//! | `invalid_input`         | 400    | `InvalidInput`         |
//! | `not_found`             | 404    | `NotFound`             |
//! | `permission_denied`     | 403    | `PermissionDenied`     |
//! | `approval_required`     | **202**| `ApprovalRequired`     |
//! | `approval_denied`       | 403    | `ApprovalDenied`       |
//! | `idempotent_replay`     | **200**| `IdempotentReplay`     |
//! | `rate_limited`          | 429    | `RateLimited`          |
//! | `internal`              | 500    | `Internal`             |
//! | `missing_authorization` | 401    | `MissingAuthorization` |
//! | `invalid_token`         | 401    | `InvalidToken`         |
//! | `workspace_mismatch`    | 403    | `WorkspaceMismatch`    |
//! | _other_                 | _any_  | `Http`                 |

use thiserror::Error;

/// Result alias used throughout the crate.
pub type Result<T, E = ActantError> = std::result::Result<T, E>;

/// Typed error surface for [`crate::ActantClient`] calls.
#[derive(Debug, Error)]
pub enum ActantError {
    /// `400 invalid_input` — the request body or query was rejected.
    #[error("invalid_input: {message}")]
    InvalidInput {
        /// Server-supplied human message.
        message: String,
        /// Raw response body (for diagnostics).
        body: Vec<u8>,
    },

    /// `404 not_found` — the addressed resource doesn't exist.
    #[error("not_found: {message}")]
    NotFound {
        /// Server-supplied human message.
        message: String,
        /// Raw response body.
        body: Vec<u8>,
    },

    /// `403 permission_denied` — the caller lacks an authority scope.
    #[error("permission_denied: {message}")]
    PermissionDenied {
        /// Server-supplied human message.
        message: String,
        /// Raw response body.
        body: Vec<u8>,
    },

    /// `202 approval_required` — Guard requested an approval before the call
    /// can execute. The original response body is preserved verbatim so the
    /// caller can read the approval id / context.
    #[error("approval_required: {message}")]
    ApprovalRequired {
        /// Server-supplied human message.
        message: String,
        /// Raw response body (carries the approval request payload).
        body: Vec<u8>,
    },

    /// `403 approval_denied` — an approval decision rejected the call.
    #[error("approval_denied: {message}")]
    ApprovalDenied {
        /// Server-supplied human message.
        message: String,
        /// Raw response body.
        body: Vec<u8>,
    },

    /// `200 idempotent_replay` — the request is a replay of a prior command
    /// (same `idempotency_key`) and the recorded result is returned verbatim.
    /// Not a failure per se; surfaced as a typed signal so the caller knows
    /// the operation didn't run twice.
    #[error("idempotent_replay: {message}")]
    IdempotentReplay {
        /// Server-supplied human message.
        message: String,
        /// Raw response body (contains the original `CommandResponse`).
        body: Vec<u8>,
    },

    /// `429 rate_limited` — per-workspace token bucket exhausted.
    #[error("rate_limited: retry after {retry_after_seconds:?}s")]
    RateLimited {
        /// `retry_after_seconds` from the body when available.
        retry_after_seconds: Option<u64>,
        /// Raw response body.
        body: Vec<u8>,
    },

    /// `401 missing_authorization` — the route required a bearer/cookie and
    /// neither was supplied.
    #[error("missing_authorization: {message}")]
    MissingAuthorization {
        /// Server-supplied human message.
        message: String,
        /// Raw response body.
        body: Vec<u8>,
    },

    /// `401 invalid_token` — the bearer token failed verification.
    #[error("invalid_token: {message}")]
    InvalidToken {
        /// Server-supplied human message.
        message: String,
        /// Raw response body.
        body: Vec<u8>,
    },

    /// `403 workspace_mismatch` — the auth context didn't match the
    /// `workspace_id` on the request.
    #[error("workspace_mismatch: {message}")]
    WorkspaceMismatch {
        /// Server-supplied human message.
        message: String,
        /// Raw response body.
        body: Vec<u8>,
    },

    /// `500 internal` or any unrecognised typed kind from the server.
    #[error("internal: {message}")]
    Internal {
        /// Server-supplied human message.
        message: String,
        /// Raw response body.
        body: Vec<u8>,
    },

    /// Catch-all for any other HTTP status / error kind combination we don't
    /// explicitly model. The `kind` mirrors the server's `error` field.
    #[error("http {status} {kind}: {message}")]
    Http {
        /// HTTP status code returned by the server.
        status: u16,
        /// Server's `error` key (or `"http_<status>"` if missing).
        kind: String,
        /// Server's `message` (or empty).
        message: String,
        /// Raw response body.
        body: Vec<u8>,
    },

    /// Transport-level failure (DNS, connect, TLS, socket close).
    #[error("transport: {0}")]
    Transport(String),

    /// Bad URL or scheme.
    #[error("invalid_url: {0}")]
    InvalidUrl(String),

    /// Response body could not be decoded as the expected type.
    #[error("decoding: {message}")]
    Decoding {
        /// Decoding error description.
        message: String,
        /// Body that failed to decode.
        body: Vec<u8>,
    },

    /// WebSocket error during a subscribe stream.
    #[error("web_socket: {0}")]
    WebSocket(String),

    /// The caller cancelled the in-flight request (e.g. tokio task abort).
    #[error("cancelled")]
    Cancelled,
}

impl ActantError {
    /// Build a typed error from a status code + body. Inspects the body for
    /// the server's `{"error":"<kind>","message":"..."}` envelope; falls back
    /// to a generic [`ActantError::Http`] when the body doesn't carry one.
    ///
    /// The status code is **not** the discriminator on its own — the server
    /// returns 200 + `idempotent_replay` and 202 + `approval_required`, so
    /// we always look at the `error` field first.
    pub fn from_response(status: u16, body: Vec<u8>) -> Self {
        #[derive(serde::Deserialize)]
        struct ErrBody {
            error: Option<String>,
            #[serde(default)]
            message: Option<String>,
            #[serde(default)]
            retry_after_seconds: Option<u64>,
        }
        let parsed: ErrBody = serde_json::from_slice(&body).unwrap_or(ErrBody {
            error: None,
            message: None,
            retry_after_seconds: None,
        });
        let kind = parsed.error.unwrap_or_else(|| format!("http_{status}"));
        let message = parsed
            .message
            .unwrap_or_else(|| String::from_utf8_lossy(&body).into_owned());
        match kind.as_str() {
            "invalid_input" => ActantError::InvalidInput { message, body },
            "not_found" => ActantError::NotFound { message, body },
            "permission_denied" => ActantError::PermissionDenied { message, body },
            "approval_required" => ActantError::ApprovalRequired { message, body },
            "approval_denied" => ActantError::ApprovalDenied { message, body },
            "idempotent_replay" => ActantError::IdempotentReplay { message, body },
            "rate_limited" => ActantError::RateLimited {
                retry_after_seconds: parsed.retry_after_seconds,
                body,
            },
            "missing_authorization" => ActantError::MissingAuthorization { message, body },
            "invalid_token" => ActantError::InvalidToken { message, body },
            "workspace_mismatch" => ActantError::WorkspaceMismatch { message, body },
            "internal" => ActantError::Internal { message, body },
            _ => ActantError::Http {
                status,
                kind,
                message,
                body,
            },
        }
    }
}

impl From<reqwest::Error> for ActantError {
    fn from(e: reqwest::Error) -> Self {
        ActantError::Transport(e.to_string())
    }
}

impl From<url::ParseError> for ActantError {
    fn from(e: url::ParseError) -> Self {
        ActantError::InvalidUrl(e.to_string())
    }
}
