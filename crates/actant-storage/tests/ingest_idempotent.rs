//! Tests for `Storage::ingest_events` — the replication-friendly idempotent
//! batch insert path that backs the FFI sync layer.
//!
//! Verifies (GAPS.md row #43):
//! - ingesting the same batch twice ends with every row skipped on the
//!   second pass
//! - tampering with a row's payload makes that row reject without
//!   corrupting the rest of the batch
//! - the supplied HLC clock advances to at least the highest HLC observed
//!   in the batch

use actant_core::*;
use actant_storage::{IngestEvent, Storage, StorageConfig};

fn make_event(
    ws: &WorkspaceId,
    actor: &ActorId,
    hlc: Hlc,
    body: &serde_json::Value,
) -> IngestEvent {
    let canonical = canonical_json(body);
    let canonical_bytes = canonical.clone().into_bytes();
    let payload_hash = sha256_hex(&canonical_bytes);
    let id = EventId::content_derived(&canonical_bytes, hlc, actor);
    let event = AgentEvent {
        id,
        workspace_id: ws.clone(),
        actor_id: actor.clone(),
        session_id: None,
        parent_event_id: None,
        event_type: "user_message".into(),
        causality_kind: CausalityKind::Observation,
        sensitivity: Sensitivity::Low,
        authority_scope_id: None,
        payload_ref: None,
        payload_inline: Some(canonical),
        payload_hash: payload_hash.clone(),
        event_hash: chain_hash(&"0".repeat(64), &payload_hash),
        created_at: now_rfc3339(),
        model_call_id: None,
        tool_call_id: None,
        workflow_run_id: None,
        memory_id: None,
        artifact_id: None,
        command_id: None,
        effect_id: None,
    };
    IngestEvent {
        event,
        hlc,
        device_id: "dev_alpha".into(),
        canonical_payload: canonical_bytes,
    }
}

async fn seeded_storage() -> (Storage, WorkspaceId, ActorId) {
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "ingest".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Human,
        display_name: "alice".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    s.insert_actor(&actor).await.unwrap();
    (s, ws.id, actor.id)
}

#[tokio::test]
async fn re_ingesting_same_batch_skips_every_row() {
    let (s, ws, actor) = seeded_storage().await;
    let batch = vec![
        make_event(
            &ws,
            &actor,
            Hlc::new(1_000, 0),
            &serde_json::json!({"a": 1}),
        ),
        make_event(
            &ws,
            &actor,
            Hlc::new(1_000, 1),
            &serde_json::json!({"a": 2}),
        ),
        make_event(
            &ws,
            &actor,
            Hlc::new(1_001, 0),
            &serde_json::json!({"a": 3}),
        ),
    ];

    let first = s.ingest_events(&batch, None).await.unwrap();
    assert_eq!(first.accepted, 3);
    assert_eq!(first.skipped, 0);
    assert!(first.rejected.is_empty());

    let second = s.ingest_events(&batch, None).await.unwrap();
    assert_eq!(second.accepted, 0);
    assert_eq!(second.skipped, 3);
    assert!(second.rejected.is_empty());
}

#[tokio::test]
async fn tampered_row_rejected_others_accepted() {
    let (s, ws, actor) = seeded_storage().await;
    let mut good_a = make_event(
        &ws,
        &actor,
        Hlc::new(2_000, 0),
        &serde_json::json!({"a": 1}),
    );
    let mut tampered = make_event(
        &ws,
        &actor,
        Hlc::new(2_000, 1),
        &serde_json::json!({"a": 2}),
    );
    let good_b = make_event(
        &ws,
        &actor,
        Hlc::new(2_000, 2),
        &serde_json::json!({"a": 3}),
    );

    // Sanity: untampered ids round-trip.
    let _ = &mut good_a;
    let _ = &mut tampered;

    // Tamper with the second row's canonical payload after the id was derived.
    tampered.canonical_payload = serde_json::to_vec(&serde_json::json!({"a": 999})).unwrap();

    let report = s
        .ingest_events(&[good_a, tampered, good_b], None)
        .await
        .unwrap();
    assert_eq!(report.accepted, 2);
    assert_eq!(report.skipped, 0);
    assert_eq!(report.rejected.len(), 1);
    assert_eq!(report.rejected[0].index, 1);
    assert_eq!(report.rejected[0].reason, "hash_mismatch");
}

#[tokio::test]
async fn empty_device_id_rejected() {
    let (s, ws, actor) = seeded_storage().await;
    let mut ev = make_event(
        &ws,
        &actor,
        Hlc::new(3_000, 0),
        &serde_json::json!({"k": 1}),
    );
    ev.device_id.clear();
    let report = s.ingest_events(&[ev], None).await.unwrap();
    assert_eq!(report.accepted, 0);
    assert_eq!(report.rejected.len(), 1);
    assert_eq!(report.rejected[0].reason, "missing_fields");
}

#[tokio::test]
async fn unknown_workspace_rejected() {
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws_unknown = WorkspaceId::new();
    let actor = ActorId::new();
    let ev = make_event(
        &ws_unknown,
        &actor,
        Hlc::new(4_000, 0),
        &serde_json::json!({"k": 1}),
    );
    let report = s.ingest_events(&[ev], None).await.unwrap();
    assert_eq!(report.accepted, 0);
    assert_eq!(report.rejected.len(), 1);
    assert_eq!(report.rejected[0].reason, "workspace_unknown");
}

#[tokio::test]
async fn ingest_advances_local_hlc() {
    let (s, ws, actor) = seeded_storage().await;
    fn frozen() -> u64 {
        100
    }
    let clock = HlcClock::with_clock_source(Hlc::new(100, 0), frozen);

    let batch = vec![
        make_event(
            &ws,
            &actor,
            Hlc::new(5_000, 0),
            &serde_json::json!({"i": 1}),
        ),
        make_event(
            &ws,
            &actor,
            Hlc::new(5_000, 1),
            &serde_json::json!({"i": 2}),
        ),
    ];
    s.ingest_events(&batch, Some(&clock)).await.unwrap();

    let after = clock.peek();
    assert!(
        after.physical_ms >= 5_000,
        "clock should have moved forward to absorb remote HLC, got {after:?}"
    );
    // local_tick now produces a strictly-greater HLC.
    let next = clock.local_tick();
    assert!(next > Hlc::new(5_000, 1));
}
