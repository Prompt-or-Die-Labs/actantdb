//! Generator round-trip for `actant-schema-dsl`.
//!
//! The crate's Phase 1 output is just the parsed `Project` struct, which
//! serializes back to JSON via `serde`. Once `Compiler::to_sql/to_rust/...`
//! land (see `/agents/actant-schema-dsl.md`), extend this file with
//! generator-specific assertions (e.g. parse generated SQL with sqlparser,
//! compile generated Rust with `rustc --emit=metadata`, etc.).

use actant_schema_dsl::parse_json;

const MINIMAL: &str = include_str!("fixtures/minimal.json");

#[test]
fn json_roundtrip_preserves_project() {
    let original = parse_json(MINIMAL).unwrap();
    let re_encoded = serde_json::to_string(&original).unwrap();
    let re_parsed = parse_json(&re_encoded).unwrap();
    assert_eq!(original.name, re_parsed.name);
    assert_eq!(original.tools.len(), re_parsed.tools.len());
    assert_eq!(original.models.len(), re_parsed.models.len());
}

#[test]
fn project_serializes_with_explicit_default_risk() {
    // Verify the round-trip carries the default-risk substitution back out — a
    // downstream SQL generator would see "medium", not None.
    let project = parse_json(MINIMAL).unwrap();
    let out = serde_json::to_value(&project).unwrap();
    let risk = out["tools"][0]["risk"].as_str().unwrap_or("");
    assert_eq!(risk, "medium");
}
