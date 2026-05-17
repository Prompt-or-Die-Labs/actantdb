//! Schema export: emit a JSON Schema document for every public contract type.
//! The TS codegen consumes this same emit path.

use schemars::{schema_for, JsonSchema};
use serde_json::{json, Value};

fn one<T: JsonSchema>() -> Value {
    serde_json::to_value(schema_for!(T)).expect("schemars to_value")
}

/// Emit the full schema set as one JSON object keyed by type name.
pub fn schemas() -> Value {
    json!({
        "Sensitivity":         one::<crate::Sensitivity>(),
        "Risk":                one::<crate::Risk>(),
        "EventKind":           one::<crate::EventKind>(),
        "ActantEvent":         one::<crate::ActantEvent>(),
        "ContextItem":         one::<crate::ContextItem>(),
        "ContextManifest":     one::<crate::ContextManifest>(),
        "ModelCall":           one::<crate::ModelCall>(),
        "ToolCallRequest":     one::<crate::ToolCallRequest>(),
        "ToolCallCompleted":   one::<crate::ToolCallCompleted>(),
        "ToolCallStatus":      one::<crate::ToolCallStatus>(),
        "PolicyVerdict":       one::<crate::PolicyVerdict>(),
        "Policy":              one::<crate::Policy>(),
        "ArgDenyRule":         one::<crate::ArgDenyRule>(),
        "ToolRiskEntry":       one::<crate::ToolRiskEntry>(),
        "ApprovalRequest":     one::<crate::ApprovalRequest>(),
        "ApprovalDecision":    one::<crate::ApprovalDecision>(),
        "CheckpointRef":       one::<crate::CheckpointRef>(),
        "ReplayOverrides":     one::<crate::ReplayOverrides>(),
        "ReplayRun":           one::<crate::ReplayRun>(),
        "DiffKind":            one::<crate::DiffKind>(),
        "DiffEntry":           one::<crate::DiffEntry>(),
        "ReplayDiff":          one::<crate::ReplayDiff>(),
    })
}
