//! Replay isolation property test — closes the "replay isolation property
//! test (no main-projection rows written during replay)" half of the
//! audit row on `agents/phase-5-extensions.md`. `spec_07_verification.rs`
//! covers the "synthetic events live in the replay scope" wording at the
//! source-grep level; this test goes further and asserts the invariant at
//! runtime against actual row counts.

use actant_core::*;
use actant_replay::{checkpoint, run as replay_run, ReplayMode};
use actant_storage::{Storage, StorageConfig};
use sqlx::Row;

async fn fixture() -> (Storage, WorkspaceId, ActorId, EventId) {
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "iso".into(),
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

    let payload = serde_json::json!({"prompt": "p"});
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

async fn count(s: &Storage, table: &str) -> i64 {
    let row = sqlx::query(&format!("SELECT COUNT(*) AS c FROM {table}"))
        .fetch_one(s.pool())
        .await
        .unwrap();
    row.get::<i64, _>("c")
}

#[tokio::test]
async fn replay_does_not_touch_main_projection_rows() {
    let (storage, ws, actor, eid) = fixture().await;
    let cp = checkpoint(&storage, &ws, &eid).await.unwrap();

    // Snapshot the main-projection tables BEFORE the replay. These are the
    // tables Phase 1 ships that a replay-mode worker could plausibly try to
    // write to. Per `/specs/07-workflows-and-replay.md` and the
    // "Do NOT write to `agent_event` from inside replay" rule, none of
    // their counts must move while a replay runs.
    let main_tables = [
        "agent_event",
        "command_record",
        "tool_call",
        "approval_request",
        "effect",
        "effect_result",
        "memory",
        "workflow_run",
        "workflow_step_run",
    ];
    let mut before = Vec::with_capacity(main_tables.len());
    for t in main_tables {
        before.push((t, count(&storage, t).await));
    }

    // Run replay across all seven declared modes (Experimental is expected
    // to error, but the error path also must not have written to the main
    // projection). This is the property-test posture from the audit row.
    for mode in [
        ReplayMode::Recorded,
        ReplayMode::Model,
        ReplayMode::Policy,
        ReplayMode::Memory,
        ReplayMode::Tool,
        ReplayMode::LocalOnly,
        ReplayMode::Experimental,
    ] {
        let _ = replay_run(&storage, &actor, &cp, mode).await;
    }

    for (t, expected) in before {
        let now = count(&storage, t).await;
        assert_eq!(
            now, expected,
            "main-projection table {t} mutated during replay (was {expected}, now {now})"
        );
    }

    // Sanity: replay-scoped tables DID move (the runs were persisted).
    let runs = count(&storage, "replay_run").await;
    assert!(runs > 0, "replay_run rows should accumulate from replays");
}
