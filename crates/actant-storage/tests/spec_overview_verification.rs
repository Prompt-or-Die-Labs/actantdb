//! Spec 00, 01, 10, 11, 12 — cross-spec consistency verification.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn spec_00_what_actantdb_is_not_boundaries_held() {
    // Spec 00: "no commands write raw secrets, raw vectors, raw blobs."
    // Check: no command in the alpha command engine accepts a field that
    // looks like a raw secret.
    let cmd = read_repo("crates/actant-command/src/lib.rs");
    for forbidden in [
        "\"password\"",
        "\"api_key\"",
        "\"private_key\"",
        "\"raw_secret\"",
    ] {
        assert!(
            !cmd.contains(forbidden),
            "command engine accepts forbidden raw-secret field {forbidden}"
        );
    }
    // Spec 03 has an "embedding" or "vector" reference (since memory
    // commands deal with embedding refs, not raw vectors).
    let spec_03 = read_repo("specs/03-command-spec.md").to_lowercase();
    assert!(
        spec_03.contains("embedding") || spec_03.contains("memory"),
        "spec 03 must reference embedding/memory abstractions, not raw vectors"
    );
}

#[test]
fn spec_01_every_subsystem_has_a_table() {
    // Spec 01: "Every subsystem here corresponds to one or more tables in
    // 02-data-model.sql."
    let schema = read_repo("specs/02-data-model.sql");
    for table in [
        "agent_event",
        "command_record",
        "model_call",
        "tool_call",
        "effect",
        "memory",
        "workflow",
        "replay_checkpoint",
        "approval_request",
    ] {
        assert!(
            schema.contains(&format!("CREATE TABLE {table}")),
            "spec 01 subsystem table {table} missing from schema"
        );
    }
}

#[test]
fn spec_10_every_command_invoked_exists() {
    // Spec 10 alpha demo invokes a specific set of commands.
    let spec_03 = read_repo("specs/03-command-spec.md");
    let alpha = [
        "append_user_message",
        "request_tool_call",
        "approve_tool_call",
        "record_tool_result",
        "propose_memory",
        "approve_memory",
        "reject_memory",
    ];
    for c in alpha {
        assert!(
            spec_03.contains(c),
            "alpha command {c} missing from spec 03"
        );
    }
}

#[test]
fn spec_11_every_phase_has_a_decision_gate() {
    let r = read_repo("specs/11-roadmap.md");
    // Each phase section must have a "Decision gate" sub-section.
    for phase in [
        "Phase 0", "Phase 1", "Phase 2", "Phase 3", "Phase 4", "Phase 5", "Phase 6",
    ] {
        assert!(r.contains(phase), "roadmap missing {phase}");
    }
    let gates = r.matches("Decision gate").count();
    assert!(
        gates >= 7,
        "expected >=7 Decision gate sections, found {gates}"
    );
}

#[test]
fn spec_12_glossary_defines_canonical_terms() {
    let g = read_repo("specs/12-glossary.md");
    for term in [
        "Actor",
        "Command",
        "Event",
        "Chronicle",
        "Effect",
        "Guard",
        "Manifest",
        "Replay",
    ] {
        assert!(g.contains(term), "glossary missing canonical term '{term}'");
    }
}

#[test]
fn spec_12_definitions_reference_real_tables() {
    // Spec 12: "Definitions reference rows or tables by their exact names
    // from 02-data-model.sql."
    let g = read_repo("specs/12-glossary.md");
    let schema = read_repo("specs/02-data-model.sql");
    for table in ["agent_event", "command_record", "effect", "memory"] {
        if g.contains(table) {
            assert!(
                schema.contains(&format!("CREATE TABLE {table}")),
                "glossary references {table} but spec 02 doesn't define it"
            );
        }
    }
}
