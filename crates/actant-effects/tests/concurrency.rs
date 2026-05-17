//! Concurrency: under contention, exactly one worker claims an effect.

use std::sync::Arc;

use actant_core::*;
use actant_effects::EffectQueue;
use actant_storage::{Storage, StorageConfig};

async fn setup(pool_size: u32) -> (Arc<EffectQueue>, WorkspaceId, ActorId, CommandId) {
    let mut cfg = StorageConfig::in_memory();
    cfg.max_connections = pool_size;
    let s = Storage::open(cfg).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "race".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Worker,
        display_name: "w".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    s.insert_actor(&actor).await.unwrap();
    let cmd = CommandRecord {
        id: CommandId::new(),
        workspace_id: ws.id.clone(),
        actor_id: actor.id.clone(),
        session_id: None,
        command_type: "t".into(),
        input_inline: None,
        input_hash: "h".into(),
        policy_id: None,
        status: CommandStatus::Committed,
        error: None,
        created_at: now_rfc3339(),
        committed_at: None,
    };
    s.insert_command(&cmd).await.unwrap();
    (
        Arc::new(EffectQueue::new(s.clone())),
        ws.id,
        actor.id,
        cmd.id,
    )
}

#[tokio::test]
async fn two_workers_racing_for_one_effect_have_one_winner() {
    // SQLite in-memory uses a single shared connection, so we get
    // serialization for free. Verify the WHERE-status='pending' gate
    // produces exactly one winner.
    let (q, ws, actor, cmd) = setup(1).await;
    let eid = q
        .enqueue(
            &ws,
            &cmd,
            &actor,
            "shell.run",
            serde_json::json!({}),
            RiskLevel::Low,
        )
        .await
        .unwrap();

    let worker_a = Worker {
        id: WorkerId::new(),
        workspace_id: ws.clone(),
        actor_id: actor.clone(),
        name: "A".into(),
        host: None,
        version: None,
        status: "online".into(),
        last_heartbeat_at: None,
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    let worker_b = Worker {
        id: WorkerId::new(),
        workspace_id: ws.clone(),
        actor_id: actor.clone(),
        name: "B".into(),
        host: None,
        version: None,
        status: "online".into(),
        last_heartbeat_at: None,
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    q.register_worker(&worker_a, &["shell.run"]).await.unwrap();
    q.register_worker(&worker_b, &["shell.run"]).await.unwrap();

    let qa = q.clone();
    let qb = q.clone();
    let ws_a = ws.clone();
    let ws_b = ws.clone();
    let wa = worker_a.id.clone();
    let wb = worker_b.id.clone();
    let (ra, rb) = tokio::join!(
        async move { qa.claim_one(&wa, &ws_a, &["shell.run"]).await },
        async move { qb.claim_one(&wb, &ws_b, &["shell.run"]).await }
    );
    let winners: Vec<_> = [ra.unwrap(), rb.unwrap()].into_iter().flatten().collect();
    assert_eq!(
        winners.len(),
        1,
        "exactly one worker should claim the effect"
    );
    assert_eq!(winners[0].effect_id.as_str(), eid.as_str());
}

#[tokio::test]
async fn double_complete_is_idempotent_at_the_status_level() {
    // Completing an effect twice should not crash. The second call is a
    // no-op (effect already 'succeeded').
    let (q, ws, actor, cmd) = setup(1).await;
    let eid = q
        .enqueue(
            &ws,
            &cmd,
            &actor,
            "shell.run",
            serde_json::json!({}),
            RiskLevel::Low,
        )
        .await
        .unwrap();
    q.complete(&eid, &serde_json::json!({"ok": true}))
        .await
        .unwrap();
    // Second complete writes another effect_result row but doesn't break.
    q.complete(&eid, &serde_json::json!({"ok": true}))
        .await
        .unwrap();
    use sqlx::Row;
    let row = sqlx::query("SELECT status FROM effect WHERE id = ?")
        .bind(eid.as_str())
        .fetch_one(q.storage().pool())
        .await
        .unwrap();
    let status: String = row.get("status");
    assert_eq!(status, "succeeded");
}
