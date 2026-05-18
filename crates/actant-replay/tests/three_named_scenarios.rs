//! Three named replay scenarios — closes the "3 named replay scenarios
//! rendered by Studio" half of the audit row on
//! `agents/phase-5-extensions.md`.
//!
//! Each scenario boots an in-memory storage with a minimal session, builds
//! a checkpoint at the recorded `model_call` event, and runs `actant_replay::run`
//! in a different mode. The headline assertion is that every scenario
//! produces a non-empty diff and writes a `replay_run` row marked
//! `completed` — i.e. the mode is wired end-to-end rather than stubbed.

use actant_core::*;
use actant_replay::{checkpoint, run, ReplayMode};
use actant_storage::{Storage, StorageConfig};
use sqlx::Row;

async fn fixture() -> (Storage, WorkspaceId, ActorId, EventId) {
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "scenarios".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Human,
        display_name: "u".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    s.insert_actor(&actor).await.unwrap();
    let session = Session {
        id: SessionId::new(),
        workspace_id: ws.id.clone(),
        title: None,
        initiator_actor_id: actor.id.clone(),
        agent_actor_id: None,
        status: SessionStatus::Active,
        created_at: now_rfc3339(),
        closed_at: None,
    };
    s.insert_session(&session).await.unwrap();

    let payload = serde_json::json!({"prompt": "hi"});
    let pc = canonical_json(&payload);
    let ph = sha256_hex(pc.as_bytes());
    let e = AgentEvent {
        id: EventId::new(),
        workspace_id: ws.id.clone(),
        actor_id: actor.id.clone(),
        session_id: Some(session.id.clone()),
        parent_event_id: None,
        event_type: "model_call".into(),
        causality_kind: CausalityKind::Intent,
        sensitivity: Sensitivity::Low,
        authority_scope_id: None,
        payload_ref: None,
        payload_inline: Some(pc),
        payload_hash: ph.clone(),
        event_hash: chain_hash(&"0".repeat(64), &ph),
        created_at: now_rfc3339(),
        model_call_id: None,
        tool_call_id: None,
        workflow_run_id: None,
        memory_id: None,
        artifact_id: None,
        command_id: None,
        effect_id: None,
    };
    s.append_event(&e).await.unwrap();
    (s, ws.id, actor.id, e.id)
}

async fn count_replay_runs(s: &Storage) -> i64 {
    let row = sqlx::query("SELECT COUNT(*) AS c FROM replay_run WHERE status = 'completed'")
        .fetch_one(s.pool())
        .await
        .unwrap();
    row.get::<i64, _>("c")
}

#[tokio::test]
async fn three_named_scenarios_complete_end_to_end() {
    let (storage, _ws_id, actor, eid) = fixture().await;
    let cp = checkpoint(&storage, &_ws_id, &eid).await.unwrap();

    // Scenario 1 — "recorded" replay (CHANGELOG: Phase 5 ships this mode).
    let recorded = run(&storage, &actor, &cp, ReplayMode::Recorded)
        .await
        .expect("recorded replay runs");
    assert!(
        !recorded.entries.is_empty(),
        "recorded scenario produced a non-empty diff"
    );

    // Scenario 2 — "model" replay (CHANGELOG: re-invokes the model worker).
    let model = run(&storage, &actor, &cp, ReplayMode::Model)
        .await
        .expect("model replay runs");
    assert!(
        !model.entries.is_empty(),
        "model scenario produced a non-empty diff"
    );

    // Scenario 3 — "policy" replay (CHANGELOG: re-evaluates Guard slots).
    let policy = run(&storage, &actor, &cp, ReplayMode::Policy)
        .await
        .expect("policy replay runs");
    assert!(
        !policy.entries.is_empty(),
        "policy scenario produced a non-empty diff"
    );

    // All three runs MUST be persisted in `replay_run` with status=completed
    // so Studio can render them from the catalog.
    let completed = count_replay_runs(&storage).await;
    assert_eq!(completed, 3, "expected 3 completed replay_run rows");

    // Each run has a distinct id.
    assert_ne!(recorded.run_id, model.run_id);
    assert_ne!(model.run_id, policy.run_id);
    assert_ne!(recorded.run_id, policy.run_id);
}
