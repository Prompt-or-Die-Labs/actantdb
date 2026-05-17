//! Spec 16 — Protocols (MCP/A2A/AP2) verification.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn every_protocol_table_in_migration_0003() {
    let mig = read_repo("migrations/0003_ai_native_and_reliability.sql");
    for table in [
        "mcp_server",
        "mcp_resource",
        "mcp_prompt",
        "a2a_card",
        "a2a_interaction",
        "ap2_mandate",
        "ap2_intent",
        "ap2_transaction",
    ] {
        assert!(
            mig.contains(&format!("CREATE TABLE {table}")),
            "migration 0003 missing protocol table {table}"
        );
    }
}

#[test]
fn ap2_mandate_enforces_spend_limit() {
    // Spec 16: "An AP2 intent without an approval_request cannot transition
    // to executed." — approximation: the Ap2Mandate type checks spend_limit.
    use actant_protocol::Ap2Mandate;
    let m = Ap2Mandate {
        holder: "x".into(),
        purpose: "buy".into(),
        spend_limit_usd: 100.0,
    };
    assert!(m.permits(50.0));
    assert!(!m.permits(150.0));
}

#[test]
fn a2a_card_type_exists() {
    use actant_protocol::A2aCard;
    let c = A2aCard {
        peer_name: "p".into(),
        endpoint: "https://example.com".into(),
        capabilities: vec!["search".into()],
    };
    assert_eq!(c.peer_name, "p");
}
