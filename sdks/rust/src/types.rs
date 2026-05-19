//! Wire-level request and response shapes.
//!
//! These match what `actant-server` actually serializes (`/v1/command`,
//! `/v1/events`, `/v1/approvals`, etc.). Domain types (`Sensitivity`, `Risk`,
//! `ReplayDiff`, …) live in `actant-contracts` and are re-exported from the
//! crate root.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `POST /v1/command` request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    /// Workspace this command belongs to.
    pub workspace_id: String,
    /// Actor attribution.
    pub actor_id: String,
    /// Registered command type (see `/v1/metadata/commands`).
    pub command_type: String,
    /// Free-form input the command expects.
    pub input: Value,
    /// Idempotency key. Repeating a command with the same key returns the
    /// original result via [`crate::ActantError::IdempotentReplay`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `POST /v1/command` success body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    /// Server-assigned command id.
    pub command_id: String,
    /// Chronicle event id this command appended, if any.
    #[serde(default)]
    pub event_id: Option<String>,
    /// Per-command result payload (varies by `command_type`).
    pub result: Value,
}

/// `GET /v1/healthz[/{startup,live,ready}]` body.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Healthz {
    /// `"ok"` for `/v1/healthz`. Empty for the phase probes.
    #[serde(default)]
    pub status: Option<String>,
    /// `"startup" | "live" | "ready"` for the phase probes.
    #[serde(default)]
    pub phase: Option<String>,
    /// `true` for an instantaneously-healthy probe.
    #[serde(default)]
    pub ok: Option<bool>,
    /// RFC3339 timestamp emitted by some variants.
    #[serde(default)]
    pub time: Option<String>,
    /// Optional human reason when `ok = false`.
    #[serde(default)]
    pub error: Option<String>,
}

impl Healthz {
    /// True when the response indicates a healthy state.
    pub fn is_healthy(&self) -> bool {
        if matches!(self.status.as_deref(), Some("ok")) {
            return true;
        }
        self.ok.unwrap_or(false)
    }
}

/// `GET /v1/events` wrapper.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventsResponse {
    /// Events in causal order.
    #[serde(default)]
    pub events: Vec<AgentEvent>,
}

/// One row from `agent_event` — the Chronicle ledger as the server
/// serializes it. Defined here (not in `actant-contracts`) because it is the
/// storage-shape `actant-core::AgentEvent`, a substrate concern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    /// Event id (ULID).
    pub id: String,
    /// Workspace id.
    pub workspace_id: String,
    /// Attribution actor.
    pub actor_id: String,
    /// Optional session anchor.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Causal parent.
    #[serde(default)]
    pub parent_event_id: Option<String>,
    /// Stringly-typed event_type (see spec 04).
    pub event_type: String,
    /// Causality kind (free-form string, e.g. `"command"`, `"effect"`).
    pub causality_kind: String,
    /// Sensitivity classification (`public|low|medium|high|secret`).
    pub sensitivity: String,
    /// Optional authority scope.
    #[serde(default)]
    pub authority_scope_id: Option<String>,
    /// Inline canonical JSON payload (string form).
    #[serde(default)]
    pub payload_inline: Option<String>,
    /// Reference to a large payload artifact.
    #[serde(default)]
    pub payload_ref: Option<String>,
    /// SHA-256 of the canonical payload JSON.
    pub payload_hash: String,
    /// Hash linking to the previous event.
    pub event_hash: String,
    /// RFC3339 created-at.
    pub created_at: String,
    /// Optional backref ids; all `Option<String>` because the server emits
    /// them as such.
    #[serde(default)]
    pub model_call_id: Option<String>,
    /// Tool call backref.
    #[serde(default)]
    pub tool_call_id: Option<String>,
    /// Workflow run backref.
    #[serde(default)]
    pub workflow_run_id: Option<String>,
    /// Memory backref.
    #[serde(default)]
    pub memory_id: Option<String>,
    /// Artifact backref.
    #[serde(default)]
    pub artifact_id: Option<String>,
    /// Command backref.
    #[serde(default)]
    pub command_id: Option<String>,
    /// Effect backref.
    #[serde(default)]
    pub effect_id: Option<String>,
}

/// `GET /v1/approvals` wrapper.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApprovalsResponse {
    /// Pending approvals.
    #[serde(default)]
    pub approvals: Vec<PendingApproval>,
}

/// One row from `approval_request` (status = `pending`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    /// Approval row id.
    pub id: String,
    /// Tool call this approval gates.
    pub tool_call_id: String,
    /// Actor that requested the call.
    pub requested_by: String,
    /// Risk level (`low|medium|high|destructive`).
    pub risk_level: String,
    /// One-line Guard summary.
    pub summary: String,
    /// Status (`pending|approved|denied`).
    pub status: String,
}

/// `POST /v1/replay/checkpoint` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayCheckpointResponse {
    /// Server-assigned checkpoint id.
    pub checkpoint_id: String,
}

/// Replay mode parameter for `/v1/replay/run`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplayMode {
    /// Replay against the recorded tool results (no real execution).
    Recorded,
    /// Re-invoke the model only.
    Model,
    /// Re-evaluate policy / Guard only.
    Policy,
    /// Rebuild the memory set only.
    Memory,
}

impl ReplayMode {
    /// Wire string accepted by the server.
    pub fn as_str(&self) -> &'static str {
        match self {
            ReplayMode::Recorded => "recorded",
            ReplayMode::Model => "model",
            ReplayMode::Policy => "policy",
            ReplayMode::Memory => "memory",
        }
    }
}

/// `POST /v1/sync/since` response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncSinceResponse {
    /// Events strictly after `since_event_id`, in ULID order.
    #[serde(default)]
    pub events: Vec<SyncEvent>,
    /// Cursor to pass back as the next `since_event_id`. `None` when no
    /// further events exist.
    #[serde(default)]
    pub next_since: Option<String>,
}

/// One row in [`SyncSinceResponse`]. Different shape from [`AgentEvent`]
/// because `/v1/sync/since` projects a narrower column set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEvent {
    /// Event id.
    pub id: String,
    /// Event type.
    pub event_type: String,
    /// Actor id.
    pub actor_id: String,
    /// Payload hash.
    pub payload_hash: String,
    /// Inline payload (canonical JSON string).
    #[serde(default)]
    pub payload_inline: Option<String>,
    /// RFC3339 created-at.
    pub created_at: String,
}

/// Topic addressed by a [`crate::SubscriptionMessage`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionTopic {
    /// Workspace id.
    pub workspace_id: String,
    /// Optional session scope.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Topic kind (`"events"`, `"approvals"`, …).
    pub kind: String,
}

/// A row returned from `GET /v1/memories`. The server emits a discriminated
/// union via the underlying `MemoryRow` enum; we keep it as a free-form value
/// here so the client compiles against future memory-row variants.
pub type MemoryRow = serde_json::Value;
