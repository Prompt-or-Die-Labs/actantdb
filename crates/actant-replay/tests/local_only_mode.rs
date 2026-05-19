//! `mode=local_only` — strictly in-process replay. Closes the local_only
//! third of GAPS.md row #7.
//!
//! Scenarios:
//!   * a session with no remote routes — diff is byte-for-byte
//!     identical to `mode=recorded`.
//!   * a session containing a model_call that crossed a cloud route — the
//!     local_only diff marks that row `changed` (with a summary explaining
//!     why) while a side-by-side `recorded` diff leaves it `identical`.
//!   * isolation: local_only does NOT read from the effect queue, hit the
//!     network, or write to any main-projection table. We assert the
//!     no-mutation half here; the no-network half is structural (there is
//!     no code path in `run_local_only` that issues HTTP or queries
//!     `effect` / `effect_result`).

use actant_core::*;
use actant_replay::{checkpoint, run, ReplayMode};
use actant_storage::{Storage, StorageConfig};
use sqlx::Row;

async fn fixture(
    payloads: &[(&str, serde_json::Value)],
) -> (Storage, WorkspaceId, ActorId, EventId) {
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "lo".into(),
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

    let mut prev = "0".repeat(64);
    let mut anchor: Option<EventId> = None;
    for (i, (etype, payload)) in payloads.iter().enumerate() {
        let pc = canonical_json(payload);
        let ph = sha256_hex(pc.as_bytes());
        let event_hash = chain_hash(&prev, &ph);
        let e = AgentEvent {
            id: EventId::new(),
            workspace_id: ws.id.clone(),
            actor_id: actor.id.clone(),
            session_id: Some(session.id.clone()),
            parent_event_id: None,
            event_type: (*etype).into(),
            causality_kind: CausalityKind::Intent,
            sensitivity: Sensitivity::Low,
            authority_scope_id: None,
            payload_ref: None,
            payload_inline: Some(pc),
            payload_hash: ph,
            event_hash: event_hash.clone(),
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
        prev = event_hash;
        if i == 0 {
            anchor = Some(e.id.clone());
        }
    }
    (s, ws.id, actor.id, anchor.unwrap())
}

#[tokio::test]
async fn local_only_matches_recorded_when_no_remote_routes() {
    // Session that only references a local route and a local tool.
    let (s, ws, actor, eid) = fixture(&[
        (
            "model_call",
            serde_json::json!({"route": "local:ollama:llama3", "prompt":"hi"}),
        ),
        (
            "tool_call_completed",
            serde_json::json!({"tool_call_id":"tc1","status":"ok","result":{"k":1}}),
        ),
        ("agent_run_finished", serde_json::json!({})),
    ])
    .await;

    let cp = checkpoint(&s, &ws, &eid).await.unwrap();
    let recorded = run(&s, &actor, &cp, ReplayMode::Recorded).await.unwrap();
    let local = run(&s, &actor, &cp, ReplayMode::LocalOnly).await.unwrap();

    assert_eq!(recorded.entries.len(), local.entries.len());
    for (a, b) in recorded.entries.iter().zip(local.entries.iter()) {
        assert_eq!(
            a.event_type, b.event_type,
            "stream shape must be identical between recorded and local_only"
        );
        assert_eq!(
            a.kind, b.kind,
            "for runs with no remote side effects, local_only and recorded agree on every slot ({})",
            a.event_type
        );
        assert_eq!(b.kind, "identical");
    }
}

#[tokio::test]
async fn local_only_flags_cloud_routes_as_changed() {
    let (s, ws, actor, eid) = fixture(&[
        (
            "model_call",
            serde_json::json!({"route": "anthropic:claude-opus-4-7", "prompt": "expensive"}),
        ),
        (
            "model_call",
            serde_json::json!({"route": "local:ollama:llama3", "prompt": "cheap"}),
        ),
        ("agent_run_finished", serde_json::json!({})),
    ])
    .await;

    let cp = checkpoint(&s, &ws, &eid).await.unwrap();
    let diff = run(&s, &actor, &cp, ReplayMode::LocalOnly).await.unwrap();

    assert_eq!(diff.entries[0].kind, "changed", "cloud model_call flagged");
    assert!(
        diff.entries[0].summary.is_some(),
        "local_only changed rows must carry a summary explaining the swap"
    );
    assert_eq!(
        diff.entries[1].kind, "identical",
        "local model_call left alone"
    );
    assert_eq!(diff.entries[2].kind, "identical", "finish unchanged");
}

#[tokio::test]
async fn local_only_does_not_mutate_main_projection() {
    let (s, ws, actor, eid) = fixture(&[
        (
            "model_call",
            serde_json::json!({"route": "anthropic:claude", "prompt": "x"}),
        ),
        ("agent_run_finished", serde_json::json!({})),
    ])
    .await;
    let cp = checkpoint(&s, &ws, &eid).await.unwrap();

    async fn count(s: &Storage, table: &str) -> i64 {
        let r = sqlx::query(&format!("SELECT COUNT(*) AS c FROM {table}"))
            .fetch_one(s.pool())
            .await
            .unwrap();
        r.get::<i64, _>("c")
    }
    let projection_tables = [
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
    let mut before = Vec::with_capacity(projection_tables.len());
    for t in projection_tables {
        before.push((t, count(&s, t).await));
    }

    let _ = run(&s, &actor, &cp, ReplayMode::LocalOnly).await.unwrap();

    for (t, expected) in before {
        assert_eq!(
            count(&s, t).await,
            expected,
            "local_only must not write to main-projection table {t}"
        );
    }
}

#[tokio::test]
async fn local_only_is_deterministic() {
    let payloads = [
        (
            "model_call",
            serde_json::json!({"route": "anthropic:claude", "prompt": "x"}),
        ),
        (
            "model_call",
            serde_json::json!({"route": "local:ollama", "prompt": "y"}),
        ),
    ];
    let (s, ws, actor, eid) = fixture(&payloads).await;
    let cp = checkpoint(&s, &ws, &eid).await.unwrap();
    let a = run(&s, &actor, &cp, ReplayMode::LocalOnly).await.unwrap();
    let b = run(&s, &actor, &cp, ReplayMode::LocalOnly).await.unwrap();

    // Run ids differ (each run gets its own ulid) but the diff stream
    // shape must be byte-for-byte identical.
    assert_ne!(a.run_id, b.run_id);
    assert_eq!(a.entries.len(), b.entries.len());
    for (x, y) in a.entries.iter().zip(b.entries.iter()) {
        assert_eq!(x.event_type, y.event_type);
        assert_eq!(x.kind, y.kind);
        assert_eq!(x.summary, y.summary);
    }
}
