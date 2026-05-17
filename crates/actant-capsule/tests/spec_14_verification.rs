//! Spec 14 — Extended Primitives verification.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn every_extended_table_in_migration_0002() {
    // Spec 14 verification: "Every table introduced here has a
    // column-by-column counterpart in migrations/0002_extended_primitives.sql"
    let mig = read_repo("migrations/0002_extended_primitives.sql");
    for table in [
        "intent",
        "observation",
        "capsule",
        "capsule_membership",
        "delegation",
        "budget",
        "regret_event",
        "eval_case",
        "eval_run",
        "memory_conflict",
        "intervention",
        "trust_profile",
        "compensation_plan",
        "model_route_decision",
        "context_debt",
        "drift_signal",
    ] {
        let pattern = format!("CREATE TABLE {table}");
        assert!(
            mig.contains(&pattern),
            "migration 0002 missing extended primitive table {table}"
        );
    }
}

#[test]
fn cross_cutting_alters_in_spec_02() {
    // Spec 14: "Every cross-cutting ALTER TABLE is reflected in spec 02."
    let spec_02 = read_repo("specs/02-data-model.sql");
    for col in [
        "causal_parent_ids",
        "undo_capability",
        "drift_threshold",
        "capsule_id",
    ] {
        assert!(
            spec_02.contains(col),
            "spec 02 missing cross-cutting column {col}"
        );
    }
}

#[test]
fn every_primitive_referenced_in_spec_13() {
    // Spec 14: "Every primitive is referenced from 13-actant-contract.md."
    let spec_13 = read_repo("specs/13-actant-contract.md").to_lowercase();
    for primitive in [
        "intent",
        "observation",
        "capsule",
        "delegation",
        "budget",
        "trust",
        "drift",
    ] {
        assert!(
            spec_13.contains(primitive),
            "spec 14 primitive '{primitive}' not referenced from spec 13"
        );
    }
}
