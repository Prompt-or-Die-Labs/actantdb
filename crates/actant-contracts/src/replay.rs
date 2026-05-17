//! Replay types: checkpoints, overrides, diffs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A checkpoint that captures everything needed to re-run from an event.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CheckpointRef {
    /// Event the checkpoint anchors to.
    pub event_id: String,
    /// Run identifier.
    pub run_id: String,
    /// Hash of the context manifest at that point.
    pub manifest_hash: String,
    /// Hash of the policy active at that point.
    pub policy_hash: String,
    /// Hash of the memory-set ids included at that point.
    pub memory_set_hash: String,
    /// Identifier(s) of prior tool results in scope, in order.
    #[serde(default)]
    pub prior_tool_results: Vec<String>,
}

/// Replay overrides. Each is optional and applied independently.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ReplayOverrides {
    /// Alternate policy hash to evaluate Guard under.
    #[serde(default)]
    pub policy: Option<String>,
    /// Memory ids to exclude from the rebuilt manifest.
    #[serde(default)]
    pub without_memory: Vec<String>,
    /// Alternate model id.
    #[serde(default)]
    pub model: Option<String>,
}

/// One replay run record.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReplayRun {
    /// Replay run identifier.
    pub id: String,
    /// Source checkpoint event id.
    pub from_event: String,
    /// Original run this replay derives from.
    pub original_run: String,
    /// Overrides applied.
    pub overrides: ReplayOverrides,
    /// RFC3339 creation timestamp.
    pub created_at: String,
    /// Events produced by the replay, in order.
    pub events: Vec<crate::ActantEvent>,
}

/// Kind of a diff entry between two event streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    /// Identical payload at this causal slot.
    Identical,
    /// Payload differs but the event kind is the same.
    Changed,
    /// Present in `a`, missing in `b`.
    Missing,
    /// Present in `b`, not in `a`.
    Extra,
}

/// One row of a replay diff (causally aligned).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiffEntry {
    /// Event kind for this row.
    pub kind: String,
    /// Outcome.
    pub diff: DiffKind,
    /// Side A payload (original).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<serde_json::Value>,
    /// Side B payload (replay).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b: Option<serde_json::Value>,
}

/// A full diff between two runs (or a run and a replay).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReplayDiff {
    /// Run id of side A.
    pub a: String,
    /// Run id of side B.
    pub b: String,
    /// Per-event entries in causal order.
    pub entries: Vec<DiffEntry>,
}
