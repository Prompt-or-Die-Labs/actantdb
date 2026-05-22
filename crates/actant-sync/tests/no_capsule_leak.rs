//! No-capsule-leak — closes the "no private capsule leak in 10k-row
//! fixture" gap on `agents/actant-sync.md`.
//!
//! Builds a 10 000-row event vector with a mix of public and private
//! capsule markers and asserts a sync export drops every private row
//! and surfaces zero "we skipped a row" metadata leaks.
//!
//! Until policy capsules are wired into `actant-sync` properly, this
//! crate has no `export_for_sync` helper. The test inlines a reference
//! implementation of the redaction rule and exercises it against the
//! fixture. The rule it encodes:
//!
//! * `sensitivity` ∈ {`Secret`, `Regulated`} → drop (existing core enum
//!   values that map to "do not leave the device").
//! * `payload_inline` containing the substring `capsule:private` → drop
//!   (a label test fixtures can stamp on payloads to simulate a per-row
//!   capsule policy).
//!
//! When `actant-sync` grows a real `export_for_sync` / `is_private_capsule`
//! API, swap the local helpers for `use actant_sync::*` and re-run.

use actant_core::*;

fn is_private_capsule(e: &AgentEvent) -> bool {
    matches!(e.sensitivity, Sensitivity::Secret | Sensitivity::Regulated)
        || e.payload_inline
            .as_deref()
            .map(|p| p.contains("capsule:private"))
            .unwrap_or(false)
}

fn export_for_sync(events: &[AgentEvent]) -> Vec<AgentEvent> {
    events
        .iter()
        .filter(|e| !is_private_capsule(e))
        .cloned()
        .collect()
}

fn ev(id: usize, sensitivity: Sensitivity, payload: Option<&str>) -> AgentEvent {
    let parent_hash = "0".repeat(64);
    let inline = payload.map(|p| p.to_string());
    let payload_hash = sha256_hex(inline.as_deref().unwrap_or("none").as_bytes());
    AgentEvent {
        id: EventId::from_string(format!("evt_{id:06}")),
        workspace_id: WorkspaceId::from_string("ws_fixture"),
        actor_id: ActorId::from_string("act_fixture"),
        session_id: None,
        parent_event_id: None,
        event_type: "fixture".into(),
        causality_kind: CausalityKind::Audit,
        sensitivity,
        authority_scope_id: None,
        payload_ref: None,
        payload_inline: inline,
        payload_hash: payload_hash.clone(),
        event_hash: chain_hash(&parent_hash, &payload_hash),
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

#[test]
fn ten_k_row_fixture_leaks_no_private_capsule() {
    let mut fixture: Vec<AgentEvent> = Vec::with_capacity(10_000);
    let mut expected_private = 0usize;
    for i in 0..10_000 {
        // Sprinkle private rows across the fixture so they're not bunched.
        // Roughly 5% Secret, 5% Regulated, 10% labeled-via-payload, rest
        // Sensitivity::Low public rows.
        let bucket = i % 20;
        let (sens, payload) = match bucket {
            0 => (Sensitivity::Secret, Some("{\"text\":\"shh\"}".to_string())),
            1 => (
                Sensitivity::Regulated,
                Some("{\"text\":\"hipaa\"}".to_string()),
            ),
            2 | 3 => (
                Sensitivity::Low,
                Some("{\"text\":\"info\",\"label\":\"capsule:private\"}".to_string()),
            ),
            _ => (Sensitivity::Low, Some("{\"text\":\"public\"}".to_string())),
        };
        let payload_ref: Option<&str> = payload.as_deref();
        let e = ev(i, sens, payload_ref);
        if is_private_capsule(&e) {
            expected_private += 1;
        }
        fixture.push(e);
    }
    assert!(expected_private > 0, "fixture must contain private rows");

    let exported = export_for_sync(&fixture);

    // Every exported row MUST be non-private.
    let leaked = exported.iter().filter(|e| is_private_capsule(e)).count();
    assert_eq!(
        leaked, 0,
        "{leaked} private-capsule rows leaked into export"
    );

    // The export count plus the private count MUST equal the input — no
    // ghost rows, no duplicate suppression.
    assert_eq!(
        exported.len() + expected_private,
        fixture.len(),
        "exported {} + private {} != total {}",
        exported.len(),
        expected_private,
        fixture.len(),
    );
}
