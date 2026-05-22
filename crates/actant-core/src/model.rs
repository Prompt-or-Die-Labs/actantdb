//! Shared model types — enums and structs that map 1:1 to the SQL schema.

use serde::{Deserialize, Serialize};

use crate::ids::*;

/// Actor kind. Drives Guard's authority checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    /// A human user.
    Human,
    /// A top-level agent.
    Agent,
    /// A sub-agent spawned by an agent.
    Subagent,
    /// A model treated as an actor for attribution.
    Model,
    /// A tool actor (rarely used).
    Tool,
    /// A worker process.
    Worker,
    /// The system itself.
    System,
}

/// Sensitivity classification of any payload or content item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sensitivity {
    /// Safe to expose anywhere.
    Public,
    /// Low-risk data; default.
    Low,
    /// Attributable but not high-impact.
    Medium,
    /// PII or business-sensitive.
    High,
    /// Secrets, tokens, keys.
    Secret,
    /// Regulated data (HIPAA, GDPR, etc).
    Regulated,
}

impl Sensitivity {
    /// Stable ordering from least to most sensitive.
    pub fn rank(self) -> u8 {
        match self {
            Self::Public => 0,
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::Secret => 4,
            Self::Regulated => 5,
        }
    }
}

/// Risk classification used by tools and effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// Read-only or trivial mutations.
    Low,
    /// Bounded mutations; recoverable.
    Medium,
    /// Wide-blast-radius mutations.
    High,
    /// Out-of-process or irreversible.
    Critical,
}

/// Causality classification on `agent_event.causality_kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CausalityKind {
    /// Something observed (input arrived).
    Observation,
    /// A planned action (intent).
    Intent,
    /// A side-effect record.
    Effect,
    /// Control-plane action (approval, cancel, …).
    Control,
    /// Audit record.
    Audit,
}

/// Status enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    /// Active session accepting events.
    Active,
    /// Paused — no new events expected without intervention.
    Paused,
    /// Closed; immutable.
    Closed,
}

/// Status of an idempotent command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandStatus {
    /// Server received but not yet committed.
    Received,
    /// Successfully committed.
    Committed,
    /// Rejected (e.g. permission, validation).
    Rejected,
}

/// Status of an effect on the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectStatus {
    /// Awaiting a worker.
    Pending,
    /// A worker has claimed it.
    Claimed,
    /// Currently running.
    Running,
    /// Succeeded.
    Succeeded,
    /// Failed (terminal after max attempts).
    Failed,
    /// Cancelled.
    Cancelled,
    /// Holding for approval.
    AwaitingApproval,
}

/// Status of a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    /// Just requested.
    Requested,
    /// Pending an approver decision.
    PendingApproval,
    /// Approved.
    Approved,
    /// Denied.
    Denied,
    /// Currently executing.
    Running,
    /// Completed successfully.
    Completed,
    /// Failed.
    Failed,
    /// Cancelled before completion.
    Cancelled,
}

/// Status of an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalStatus {
    /// Waiting on an approver.
    Pending,
    /// Granted.
    Approved,
    /// Refused.
    Denied,
    /// Timed out.
    Expired,
    /// Escalated to a higher approver.
    Escalated,
    /// Cancelled by the requester.
    Cancelled,
}

/// Status of a memory candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCandidateStatus {
    /// Proposed automatically.
    Proposed,
    /// Held for human review.
    PendingReview,
    /// Accepted into the memory store.
    Approved,
    /// Rejected — won't be re-proposed verbatim.
    Rejected,
    /// Accepted with edits.
    Edited,
}

/// One row from the `workspace` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Workspace id.
    pub id: WorkspaceId,
    /// Display name.
    pub name: String,
    /// RFC3339 creation time.
    pub created_at: String,
    /// RFC3339 archive time (if archived).
    pub archived_at: Option<String>,
}

/// One row from `actor`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    /// Actor id.
    pub id: ActorId,
    /// Owning workspace.
    pub workspace_id: WorkspaceId,
    /// Kind.
    pub kind: ActorKind,
    /// Display name.
    pub display_name: String,
    /// Created at.
    pub created_at: String,
    /// Disabled at.
    pub disabled_at: Option<String>,
}

/// One row from `session`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session id.
    pub id: SessionId,
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Optional title.
    pub title: Option<String>,
    /// Whoever initiated (often a human).
    pub initiator_actor_id: ActorId,
    /// Bound agent actor if known.
    pub agent_actor_id: Option<ActorId>,
    /// Status.
    pub status: SessionStatus,
    /// Created at.
    pub created_at: String,
    /// Closed at.
    pub closed_at: Option<String>,
}

/// One row from `agent_event` — the Chronicle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    /// Event id.
    pub id: EventId,
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Attribution.
    pub actor_id: ActorId,
    /// Optional session anchor.
    pub session_id: Option<SessionId>,
    /// Causal parent event.
    pub parent_event_id: Option<EventId>,
    /// Event type (a stringly-typed enum; see spec 04).
    pub event_type: String,
    /// Causality kind.
    pub causality_kind: CausalityKind,
    /// Sensitivity classification.
    pub sensitivity: Sensitivity,
    /// Optional authority scope this event was emitted under.
    pub authority_scope_id: Option<AuthorityScopeId>,
    /// Inline JSON payload.
    pub payload_inline: Option<String>,
    /// Reference to a large artifact payload.
    pub payload_ref: Option<String>,
    /// SHA-256 of the canonical payload JSON.
    pub payload_hash: String,
    /// Chain hash linking to the previous event in the same workspace/session.
    pub event_hash: String,
    /// RFC3339 created at.
    pub created_at: String,
    /// Optional backrefs.
    pub model_call_id: Option<ModelCallId>,
    /// Optional backref.
    pub tool_call_id: Option<ToolCallId>,
    /// Optional backref.
    pub workflow_run_id: Option<WorkflowRunId>,
    /// Optional backref.
    pub memory_id: Option<MemoryId>,
    /// Optional backref.
    pub artifact_id: Option<ArtifactId>,
    /// Optional backref.
    pub command_id: Option<CommandId>,
    /// Optional backref.
    pub effect_id: Option<EffectId>,
}

/// One row from `command_record`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRecord {
    /// Command id.
    pub id: CommandId,
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Actor.
    pub actor_id: ActorId,
    /// Optional session anchor.
    pub session_id: Option<SessionId>,
    /// Command type.
    pub command_type: String,
    /// Inline input JSON.
    pub input_inline: Option<String>,
    /// Hash of canonical input JSON.
    pub input_hash: String,
    /// Policy id at time of evaluation.
    pub policy_id: Option<PolicyId>,
    /// Status.
    pub status: CommandStatus,
    /// Optional error message.
    pub error: Option<String>,
    /// Created at.
    pub created_at: String,
    /// Committed at.
    pub committed_at: Option<String>,
}

/// One row from `approval_request`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Approval request id.
    pub id: ApprovalRequestId,
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Optional effect this gates.
    pub effect_id: Option<EffectId>,
    /// Optional tool call this gates.
    pub tool_call_id: Option<ToolCallId>,
    /// Requester.
    pub requested_by_actor_id: ActorId,
    /// Risk level.
    pub risk_level: RiskLevel,
    /// Permission name.
    pub required_permission: String,
    /// Human-readable summary.
    pub summary: String,
    /// Status.
    pub status: ApprovalStatus,
    /// Approval scope.
    pub scope_granted: Option<String>,
    /// Created at.
    pub created_at: String,
    /// Approval timestamp.
    pub approved_at: Option<String>,
    /// Approver id.
    pub approved_by_actor_id: Option<ActorId>,
    /// Denial reason.
    pub denied_reason: Option<String>,
}

/// One row from `tool_call`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool call id.
    pub id: ToolCallId,
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Optional session anchor.
    pub session_id: Option<SessionId>,
    /// Requesting actor.
    pub requested_by_actor_id: ActorId,
    /// Tool id (FK into `tool`).
    pub tool_id: ToolId,
    /// Schema version.
    pub schema_version: i64,
    /// Inline arguments JSON.
    pub arguments_inline: Option<String>,
    /// Argument hash.
    pub arguments_hash: String,
    /// Status.
    pub status: ToolCallStatus,
    /// Risk level.
    pub risk_level: RiskLevel,
    /// Approval id if any.
    pub approval_request_id: Option<ApprovalRequestId>,
    /// Effect id if dispatched.
    pub effect_id: Option<EffectId>,
    /// Inline result.
    pub result_ref: Option<String>,
    /// Result hash.
    pub result_hash: Option<String>,
    /// Optional error.
    pub error: Option<String>,
    /// Created at.
    pub created_at: String,
    /// Completed at.
    pub completed_at: Option<String>,
}

/// One row from `effect`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effect {
    /// Effect id.
    pub id: EffectId,
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Originating command.
    pub command_id: CommandId,
    /// Requester.
    pub requested_by_actor_id: ActorId,
    /// Effect type (matches a worker capability).
    pub effect_type: String,
    /// Status.
    pub status: EffectStatus,
    /// Optional permission required.
    pub required_permission: Option<String>,
    /// Risk classification.
    pub risk_level: RiskLevel,
    /// Idempotency key if any.
    pub idempotency_key: Option<String>,
    /// Inline input JSON.
    pub input_inline: Option<String>,
    /// Input hash.
    pub input_hash: String,
    /// Currently-assigned worker (if claimed).
    pub assigned_worker_id: Option<WorkerId>,
    /// Attempt count.
    pub attempt_count: i64,
    /// Maximum attempts.
    pub max_attempts: i64,
    /// Next attempt time.
    pub next_attempt_at: Option<String>,
    /// Started at.
    pub started_at: Option<String>,
    /// Finished at.
    pub finished_at: Option<String>,
    /// Inline result ref.
    pub result_ref: Option<String>,
    /// Result hash.
    pub result_hash: Option<String>,
    /// Optional error.
    pub error: Option<String>,
    /// Created at.
    pub created_at: String,
}

/// One row from `worker`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    /// Worker id.
    pub id: WorkerId,
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Actor that the worker authenticates as.
    pub actor_id: ActorId,
    /// Display name.
    pub name: String,
    /// Host.
    pub host: Option<String>,
    /// Version.
    pub version: Option<String>,
    /// Status: 'online'|'draining'|'offline'.
    pub status: String,
    /// Last heartbeat.
    pub last_heartbeat_at: Option<String>,
    /// Created at.
    pub created_at: String,
    /// Disabled at.
    pub disabled_at: Option<String>,
}

/// One row from `memory_candidate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    /// Candidate id.
    pub id: MemoryCandidateId,
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Proposer.
    pub proposed_by_actor_id: ActorId,
    /// JSON array of source event ids.
    pub source_event_ids: String,
    /// The memory text.
    pub text: String,
    /// Category.
    pub category: String,
    /// Confidence \[0,1].
    pub confidence: f64,
    /// Sensitivity.
    pub sensitivity: Sensitivity,
    /// Status.
    pub status: MemoryCandidateStatus,
    /// Optional review reason.
    pub review_reason: Option<String>,
    /// Created at.
    pub created_at: String,
}

/// One row from `memory`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Memory id.
    pub id: MemoryId,
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Memory text.
    pub text: String,
    /// Category.
    pub category: String,
    /// Sensitivity.
    pub sensitivity: Sensitivity,
    /// Confidence.
    pub confidence: Option<f64>,
    /// Scope.
    pub scope: String,
    /// Optional source candidate.
    pub source_candidate_id: Option<MemoryCandidateId>,
    /// JSON array of source event ids.
    pub source_event_ids: String,
    /// Optional embedding pointer.
    pub embedding_ref_id: Option<EmbeddingRefId>,
    /// Usage count.
    pub usage_count: i64,
    /// Last used.
    pub last_used_at: Option<String>,
    /// Expires.
    pub expires_at: Option<String>,
    /// Revoked.
    pub revoked_at: Option<String>,
    /// Deleted.
    pub deleted_at: Option<String>,
    /// Created at.
    pub created_at: String,
}
