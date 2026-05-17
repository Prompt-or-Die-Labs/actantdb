//! Spec 13 — Actant Contract verification.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn every_obligation_has_a_command_or_primitive() {
    // Spec 13 §4: eight obligations — actor / intent / context / capability
    // / consent / observation / commitment / reflection. Each must map to a
    // command in spec 03 OR an extended primitive in spec 14.
    let spec_03 = read_repo("specs/03-command-spec.md").to_lowercase();
    let spec_14 = read_repo("specs/14-extended-primitives.md").to_lowercase();
    let combined = format!("{spec_03}\n{spec_14}");
    // The eight obligations map onto these concrete concepts:
    let mappings: &[(&str, &[&str])] = &[
        ("actor", &["actor", "create_session", "register"]),
        ("intent", &["intent", "form_intent", "context_build"]),
        ("context", &["context_build", "context_item"]),
        ("capability", &["authority_scope", "permission", "scope"]),
        ("consent", &["approval", "approve_tool_call"]),
        (
            "observation",
            &["observation", "tool_call_finished", "record_tool_result"],
        ),
        ("commitment", &["effect", "command_record"]),
        ("reflection", &["replay", "regret", "drift"]),
    ];
    for (obligation, hooks) in mappings {
        let any = hooks.iter().any(|h| combined.contains(h));
        assert!(
            any,
            "spec 13 obligation '{obligation}' has no hook in spec 03 or 14 (looked for: {hooks:?})"
        );
    }
}

#[test]
fn every_primitive_in_section_22_is_defined() {
    // Spec 13 §22 names primitives that should exist either in the canonical
    // schema (spec 02) or the extended primitives (spec 14).
    let schema = read_repo("specs/02-data-model.sql");
    let ext = read_repo("specs/14-extended-primitives.md");
    let combined = format!("{schema}\n{ext}");
    let primitives = [
        "actor",
        "session",
        "agent_event",
        "command_record",
        "intent",
        "observation",
        "capsule",
        "delegation",
        "budget",
        "trust_profile",
        "drift_signal",
        "compensation_plan",
    ];
    for p in primitives {
        assert!(
            combined.contains(p),
            "primitive {p} from spec 13 §22 not defined"
        );
    }
}
