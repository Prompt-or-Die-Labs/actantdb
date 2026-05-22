//! Smoke tests for actant-contracts.
//!
//! - schema bundle is non-empty
//! - serde round-trip works for every top-level type the killer demo uses
//! - JSON Schema keys are stable (changes here require codegen-ts re-run)

use actant_contracts::schema::schemas;
use actant_contracts::*;
use serde_json::json;

#[test]
fn schema_bundle_has_all_expected_types() {
    let s = schemas();
    let obj = s.as_object().expect("schemas() is a json object");
    for expected in [
        "Sensitivity",
        "Risk",
        "EventKind",
        "ActantEvent",
        "ContextItem",
        "ContextManifest",
        "ModelCall",
        "ToolCallRequest",
        "ToolCallCompleted",
        "ToolCallStatus",
        "PolicyVerdict",
        "Policy",
        "ApprovalRequest",
        "ApprovalDecision",
        "CheckpointRef",
        "ReplayRun",
        "ReplayDiff",
        "DiffEntry",
        "DiffKind",
        "ReplayOverrides",
        "Embedding",
        "ActantHotToolCall",
        "ActantPromptVersion",
        "ActantPromptTemplate",
        "ActantModelInfo",
        "ActantModelRegistry",
        "ActantIndexedItem",
        "ActantHit",
        "ActantSearchMode",
        "ActantSearchOptions",
        "ActantSearchHit",
        "ActantEntityRelation",
        "ActantIndex",
        "ActantMcpServer",
        "ActantA2aCard",
        "ActantAp2Mandate",
        "MemoryAllowed",
        "ActantCapsule",
        "ActantTrustProfile",
    ] {
        assert!(obj.contains_key(expected), "missing schema: {expected}");
    }
}

#[test]
fn policy_verdict_serde_round_trip() {
    let v = PolicyVerdict::RequireApproval {
        reason: "rm -rf includes /dist".into(),
        policy_snapshot: "snap".into(),
        hint: Some("drop dist".into()),
        constrained_input: Some(json!({"command": "rm -rf build"})),
    };
    let s = serde_json::to_string(&v).unwrap();
    let r: PolicyVerdict = serde_json::from_str(&s).unwrap();
    assert_eq!(r, v);
    // tag-shaped JSON: { "decision": "require_approval", ... }
    let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed["decision"], "require_approval");
}

#[test]
fn approval_decision_serde_round_trip() {
    let d = ApprovalDecision::ApproveConstrained {
        approver: "wes".into(),
        scope: "once".into(),
        accepted_input: json!({"command": "rm -rf build"}),
    };
    let s = serde_json::to_string(&d).unwrap();
    let r: ApprovalDecision = serde_json::from_str(&s).unwrap();
    assert_eq!(r, d);
}

#[test]
fn event_kinds_are_snake_case() {
    let s = serde_json::to_string(&EventKind::ToolCallRequested).unwrap();
    assert_eq!(s, "\"tool_call_requested\"");
}

#[test]
fn risk_serde() {
    let s = serde_json::to_string(&Risk::Destructive).unwrap();
    assert_eq!(s, "\"destructive\"");
}

#[test]
fn capsule_memory_scope_serde() {
    let capsule = ActantCapsule::public_okay("public");
    let s = serde_json::to_string(&capsule).unwrap();
    assert!(s.contains("\"memory_allowed\":\"workspace\""));
    let r: ActantCapsule = serde_json::from_str(&s).unwrap();
    assert_eq!(r.memory_allowed, MemoryAllowed::Workspace);
}

#[test]
fn checkpoint_serde() {
    let cp = CheckpointRef {
        event_id: "e1".into(),
        run_id: "r1".into(),
        manifest_hash: "h".into(),
        policy_hash: "p".into(),
        memory_set_hash: "m".into(),
        prior_tool_results: vec!["t1".into()],
    };
    let s = serde_json::to_string(&cp).unwrap();
    let r: CheckpointRef = serde_json::from_str(&s).unwrap();
    assert_eq!(r.event_id, cp.event_id);
    assert_eq!(r.prior_tool_results, cp.prior_tool_results);
}
