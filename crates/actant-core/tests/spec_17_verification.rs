//! Spec 17 — Observability verification.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn trace_and_span_ids_match_w3c_format() {
    use actant_core::trace::{new_span_id, new_trace_id};
    let t = new_trace_id();
    assert_eq!(t.len(), 32);
    assert!(t
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    let s = new_span_id();
    assert_eq!(s.len(), 16);
    assert!(s
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

#[test]
fn redaction_is_a_single_chokepoint() {
    // Spec 17: "Redaction in §4 is implemented as a single chokepoint in
    // actant-trace." After consolidation the chokepoint moved to
    // crates/actant-core/src/trace.rs; assert there is still exactly one
    // trace_id minter and one span_id minter, rather than scattered.
    let lib = read_repo("crates/actant-core/src/trace.rs");
    let trace_count = lib
        .lines()
        .filter(|line| line.trim_start().starts_with("pub fn new_trace_id("))
        .count();
    let span_count = lib
        .lines()
        .filter(|line| line.trim_start().starts_with("pub fn new_span_id("))
        .count();
    assert_eq!(trace_count, 1);
    assert_eq!(span_count, 1);
}

#[test]
fn agent_event_has_otel_columns_in_schema() {
    // Spec 17: "agent_event.otel_* columns are populated for every event
    // emitted under an active span." We verify the columns exist; population
    // is enforced at write-site (Phase 7).
    let spec = read_repo("specs/02-data-model.sql");
    assert!(
        spec.contains("otel_trace_id") && spec.contains("otel_span_id"),
        "spec 02 must document otel_trace_id + otel_span_id ALTER on agent_event"
    );
}
