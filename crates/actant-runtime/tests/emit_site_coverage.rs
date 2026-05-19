//! Spec 17 §1 — every span name must have at least one emit site in `crates/`.
//!
//! Covers AC: "Every span name in `/specs/17-observability.md` §1 has an
//! emit site."
//!
//! This test parses the span catalog table from spec 17 (rows beginning
//! with `| \`<name>\``), invokes `grep -r` over the crates directory for
//! each name, and fails LOUDLY with the list of missing emit sites IF the
//! crate-level emitter functions (per the work package's Scope section)
//! eventually land. For Phase 1 the only emitters that exist are id
//! minters (`new_trace_id` / `new_span_id`); per task instructions we do
//! NOT fail on missing emitters — instead we report them and add an
//! `#[ignore = "TODO: ..."]` follow-up subtest naming the gap.
//!
//! The "redaction chokepoint" acceptance criterion is already covered by
//! `tests/spec_17_verification.rs::redaction_is_a_single_chokepoint`; this
//! file does not duplicate that assertion.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.parent().unwrap().parent().unwrap().to_path_buf()
}

/// Parse span names from `specs/17-observability.md` §1.
///
/// Looks for lines that begin (after a leading `|`) with a backtick-quoted
/// identifier — that pattern matches the `| \`workflow.run\`` table rows
/// in §1 but skips headers and prose.
fn parse_span_names() -> Vec<String> {
    let spec =
        fs::read_to_string(repo_root().join("specs/17-observability.md")).expect("read spec 17");
    let mut names = Vec::new();
    for raw in spec.lines() {
        let line = raw.trim();
        if !line.starts_with('|') {
            continue;
        }
        // Find the first backtick-quoted token after the leading `|`.
        let after_pipe = line.trim_start_matches('|').trim_start();
        if let Some(rest) = after_pipe.strip_prefix('`') {
            if let Some(end) = rest.find('`') {
                let candidate = &rest[..end];
                // Span names are dotted identifiers like `model.call`.
                // Header rows have `Span name` (with space) — filter those.
                if (candidate.contains('.')
                    || candidate
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '_'))
                    && !candidate.contains(' ')
                    && !candidate.is_empty()
                {
                    names.push(candidate.to_string());
                }
            }
        }
    }
    names
}

#[test]
fn parses_at_least_one_span_name() {
    let names = parse_span_names();
    assert!(
        !names.is_empty(),
        "spec parser produced 0 span names — table format may have changed"
    );
    // Sanity: spec 17 §1 lists ~22 spans; assert a reasonable lower bound.
    assert!(
        names.len() >= 15,
        "parsed only {} span names — expected ~22 per spec 17 §1: {names:?}",
        names.len()
    );
}

#[test]
fn report_emit_site_coverage_for_each_span() {
    let crates_dir = repo_root().join("crates");
    let names = parse_span_names();
    assert!(!names.is_empty(), "no span names parsed");

    let mut missing: Vec<String> = Vec::new();
    let mut present: Vec<String> = Vec::new();
    for name in &names {
        // Search for the literal span name as a quoted string anywhere in
        // the crates tree — emit sites typically use the literal
        // `"workflow.run"` to start the span.
        let needle = format!("\"{name}\"");
        let output = Command::new("grep")
            .args(["-r", "-l", "--include=*.rs", &needle])
            .arg(&crates_dir)
            .output()
            .expect("invoke grep");
        if output.stdout.is_empty() {
            missing.push(name.clone());
        } else {
            present.push(name.clone());
        }
    }

    // Always print the report so CI logs surface the gap.
    eprintln!(
        "spec 17 §1 emit-site coverage: {} / {} span names have at least one emit site",
        present.len(),
        names.len()
    );
    eprintln!("present: {present:?}");
    eprintln!("missing: {missing:?}");

    // Per task instructions: do NOT fail; the `#[ignore]` follow-up below
    // names the gap explicitly. This makes the absence loud without
    // breaking the workspace test gate.
}

#[test]
#[ignore = "TODO: spec 17 §1 lists 22 span emit sites; the actant-runtime trace module currently only mints trace/span ids. Full span emitters (model.call, tool.call, workflow.run, ...) are not yet implemented."]
fn every_spec_17_span_has_emit_site_in_named_crate() {
    // When span emitters land, remove the #[ignore] above and make this
    // assert non-empty grep hits for every name in parse_span_names().
}
