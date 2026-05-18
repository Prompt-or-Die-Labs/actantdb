//! Parser fixtures for `actant-schema-dsl`.
//!
//! The crate currently exposes `parse_json` against a `Project` shape (tools +
//! models). The richer `.actant` grammar described in
//! `/agents/actant-schema-dsl.md` (tables/commands/workflows) is not yet
//! implemented in this crate — when it lands, add `.actant` fixtures here and
//! extend these assertions.

use actant_schema_dsl::parse_json;

const MINIMAL: &str = include_str!("fixtures/minimal.json");
const CODING_AGENT: &str = include_str!("fixtures/coding_agent.json");

#[test]
fn parses_minimal_fixture() {
    let p = parse_json(MINIMAL).expect("minimal fixture parses");
    assert_eq!(p.name, "minimal");
    assert_eq!(p.tools.len(), 1);
    assert_eq!(p.models.len(), 1);
    assert_eq!(p.tools[0].name, "shell.run");
    assert_eq!(p.tools[0].kind, "shell");
    // No `risk` field on the fixture — the default-risk path applies.
    assert_eq!(p.tools[0].risk, "medium");
}

#[test]
fn parses_coding_agent_fixture() {
    let p = parse_json(CODING_AGENT).expect("coding-agent fixture parses");
    assert_eq!(p.name, "coding-agent");
    assert_eq!(p.tools.len(), 4);
    assert_eq!(p.models.len(), 3);

    let shell = p.tools.iter().find(|t| t.name == "shell.run").unwrap();
    assert_eq!(shell.risk, "critical", "explicit risk preserved");
    let browser = p.tools.iter().find(|t| t.name == "browser.open").unwrap();
    assert_eq!(browser.kind, "browser");

    let planner = p.models.iter().find(|m| m.name == "planner").unwrap();
    assert_eq!(planner.provider, "anthropic");
    assert_eq!(planner.model, "sonnet");
}

#[test]
fn empty_tools_and_models_default_to_empty_lists() {
    let p = parse_json(r#"{ "name": "bare" }"#).expect("bare project parses");
    assert_eq!(p.name, "bare");
    assert!(p.tools.is_empty());
    assert!(p.models.is_empty());
}

#[test]
fn malformed_json_returns_err_not_panic() {
    let r = parse_json(r#"{ "name": "x" "#); // unterminated
    assert!(r.is_err(), "expected parse error for malformed JSON");
}

#[test]
fn missing_required_field_returns_err() {
    // `name` is mandatory in the current Project shape.
    let r = parse_json(r#"{ "tools": [], "models": [] }"#);
    assert!(r.is_err(), "missing `name` should fail to parse");
}
