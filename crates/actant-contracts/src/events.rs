//! Events emitted onto the Chronicle ledger. Every causally meaningful step
//! of an agent run becomes one of these.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Sensitivity classification carried with every event payload.
/// Drives Studio rendering and replay redaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Sensitivity {
    /// Safe to expose anywhere (logs, UI, sharing).
    Public,
    /// Low-risk data; default for non-PII operational events.
    Low,
    /// User-attributable but not high-impact.
    Medium,
    /// Personally identifying or business-sensitive.
    High,
    /// Secrets, tokens, keys. Never displayed in Studio without unmask.
    Secret,
}

/// Risk classification for tool calls. Drives Guard verdicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    /// Read-only or trivial mutations.
    Low,
    /// Bounded mutations; recoverable.
    Medium,
    /// Wide-blast-radius mutations.
    High,
    /// Irreversible or out-of-process mutations (shell, network writes).
    Destructive,
}

/// Causal kind for each event written to the ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Agent run started.
    AgentRunStarted,
    /// Inbound user message attached to the run.
    UserMessageReceived,
    /// Model planner / generator was called.
    ModelCall,
    /// A tool call was proposed.
    ToolCallRequested,
    /// Guard issued a verdict on a tool call.
    GuardVerdict,
    /// An approval is required for a tool call.
    ApprovalRequired,
    /// An approval decision was recorded (approve / deny / constrain).
    ApprovalDecision,
    /// A tool call started executing (post-Guard, post-approval if any).
    ToolCallStarted,
    /// A tool call completed.
    ToolCallCompleted,
    /// The context manifest fed to the model was built.
    ContextBuild,
    /// An observation / effect was recorded back into the run.
    EffectObserved,
    /// An agent run finished.
    AgentRunFinished,
}

/// One event on the Chronicle ledger. Append-only, hash-chained downstream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantEvent {
    /// Unique event identifier (ULID; lexicographically sortable).
    pub id: String,
    /// Causal kind. Drives Studio rendering and replay routing.
    pub kind: EventKind,
    /// Owning project identifier.
    pub project: String,
    /// Run identifier (one agent invocation).
    pub run_id: String,
    /// Causal parent event, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_event_id: Option<String>,
    /// Event-specific payload. Schema varies by kind.
    pub payload: serde_json::Value,
    /// SHA-256 hex digest of the canonical payload JSON.
    pub payload_hash: String,
    /// Hash of (prev_chain_hash || payload_hash) — links events causally.
    pub chain_hash: String,
    /// Sensitivity classification.
    pub sensitivity: Sensitivity,
    /// RFC3339 timestamp.
    pub created_at: String,
}

/// One item in a context manifest. Mirrors what was actually fed to a model.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContextItem {
    /// Stable identifier for the source.
    pub id: String,
    /// Kind: memory, document, conversation, tool_doc, system.
    pub kind: String,
    /// Source description (URL, memory id, file path).
    pub source: String,
    /// Hash of the content as included.
    pub content_hash: String,
    /// Sensitivity of the content.
    pub sensitivity: Sensitivity,
    /// Free-form label for Studio display.
    pub label: String,
    /// Optional flags Studio can highlight (e.g. "stale", "suspect").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
}

/// The manifest of context items presented to a model call.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContextManifest {
    /// Hash of the included-set (drives replay matching).
    pub manifest_hash: String,
    /// Items included in the model prompt.
    pub included: Vec<ContextItem>,
    /// Items considered but excluded (with reason).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked: Vec<ContextItem>,
}

/// A model call event payload.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelCall {
    /// Model identifier (vendor:name:version).
    pub model: String,
    /// Logical role of the call within the run (e.g. "planner").
    pub role: String,
    /// Hash of the prompt actually sent.
    pub prompt_hash: String,
    /// Token usage if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u32>,
    /// Token usage if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u32>,
    /// One-line human summary of the model's output (for Studio).
    pub summary: String,
}

/// A tool call request payload.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolCallRequest {
    /// Stable identifier of this tool call.
    pub tool_call_id: String,
    /// Tool name (e.g. "shell.run", "file.write").
    pub tool: String,
    /// Tool arguments as proposed by the planner.
    pub args: serde_json::Value,
    /// Inferred risk from policy classification.
    pub risk: Risk,
}

/// A tool call completion payload.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolCallCompleted {
    /// Tool call identifier (matches the request).
    pub tool_call_id: String,
    /// Execution status.
    pub status: ToolCallStatus,
    /// Tool result, serialized.
    pub result: serde_json::Value,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Outcome of a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    /// Completed successfully.
    Ok,
    /// Failed during execution.
    Error,
    /// Skipped because Guard blocked it.
    Blocked,
    /// Skipped because approval was denied.
    Denied,
    /// Replayed against the recorded result (no real execution).
    Replayed,
}
