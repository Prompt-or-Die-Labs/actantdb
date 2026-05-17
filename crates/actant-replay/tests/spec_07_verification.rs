//! Spec 07 — workflows + replay verification.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn every_replay_mode_has_a_snapshot_ref() {
    // Spec 07: "Every replay mode is satisfiable by the four snapshot refs
    // on replay_checkpoint."
    let schema = read_repo("specs/02-data-model.sql");
    for col in [
        "state_snapshot_ref",
        "model_route_snapshot_ref",
        "permission_snapshot_ref",
        "memory_snapshot_ref",
    ] {
        assert!(
            schema.contains(col),
            "replay_checkpoint missing column {col}"
        );
    }
}

#[test]
fn every_diff_kind_in_code() {
    // Spec 07 §9: identical / changed / missing / extra.
    let src = read_repo("crates/actant-replay/src/lib.rs");
    for kind in ["identical", "changed", "missing", "extra"] {
        assert!(
            src.contains(&format!("\"{kind}\"")),
            "diff kind '{kind}' missing from actant-replay"
        );
    }
}

#[test]
fn align_streams_produces_all_four_kinds() {
    use actant_core::*;
    use actant_replay::align_streams;
    fn ev(event_type: &str, hash: &str) -> AgentEvent {
        AgentEvent {
            id: EventId::new(),
            workspace_id: WorkspaceId::new(),
            actor_id: ActorId::new(),
            session_id: None,
            parent_event_id: None,
            event_type: event_type.into(),
            causality_kind: CausalityKind::Audit,
            sensitivity: Sensitivity::Low,
            authority_scope_id: None,
            payload_ref: None,
            payload_inline: None,
            payload_hash: hash.into(),
            event_hash: hash.into(),
            created_at: now_rfc3339(),
            model_call_id: None,
            tool_call_id: None,
            workflow_run_id: None,
            memory_id: None,
            artifact_id: None,
            command_id: None,
            effect_id: None,
        }
    }
    let a = vec![ev("a", "h1"), ev("b", "h2"), ev("c", "h3")];
    let b = vec![ev("a", "h1"), ev("b", "h2-diff"), ev("d", "h4")];
    let diff = align_streams(&a, &b);
    let kinds: Vec<&str> = diff.iter().map(|d| d.kind.as_str()).collect();
    assert_eq!(kinds[0], "identical");
    assert_eq!(kinds[1], "changed");
    assert_eq!(kinds[2], "changed"); // c vs d at index 2 — both present, different hash
}

#[test]
fn align_streams_handles_missing_and_extra() {
    use actant_core::*;
    use actant_replay::align_streams;
    fn ev(et: &str) -> AgentEvent {
        AgentEvent {
            id: EventId::new(),
            workspace_id: WorkspaceId::new(),
            actor_id: ActorId::new(),
            session_id: None,
            parent_event_id: None,
            event_type: et.into(),
            causality_kind: CausalityKind::Audit,
            sensitivity: Sensitivity::Low,
            authority_scope_id: None,
            payload_ref: None,
            payload_inline: None,
            payload_hash: "h".into(),
            event_hash: "h".into(),
            created_at: now_rfc3339(),
            model_call_id: None,
            tool_call_id: None,
            workflow_run_id: None,
            memory_id: None,
            artifact_id: None,
            command_id: None,
            effect_id: None,
        }
    }
    let a = vec![ev("only_a")];
    let b: Vec<AgentEvent> = vec![];
    let diff = align_streams(&a, &b);
    assert_eq!(diff[0].kind, "missing");

    let a: Vec<AgentEvent> = vec![];
    let b = vec![ev("only_b")];
    let diff = align_streams(&a, &b);
    assert_eq!(diff[0].kind, "extra");
}

#[test]
fn every_replay_mode_named_in_enum() {
    // Spec 07 §6: recorded / experimental / policy / model / memory / tool / local_only
    let src = read_repo("crates/actant-replay/src/lib.rs");
    for mode in [
        "Recorded",
        "Experimental",
        "Policy",
        "Model",
        "Memory",
        "Tool",
        "LocalOnly",
    ] {
        assert!(src.contains(mode), "ReplayMode::{mode} missing");
    }
}

#[test]
fn no_workflow_step_performs_io_directly() {
    // Spec 07: "No workflow command directly performs I/O — every external
    // action is mediated by the Effect Engine."
    let src = read_repo("crates/actant-flow/src/lib.rs");
    for needle in ["tokio::process::", "std::fs::", "tokio::fs::", "reqwest::"] {
        assert!(
            !src.contains(needle),
            "actant-flow performs direct I/O via {needle}"
        );
    }
}
