//! Worker conformance harness.
//!
//! AC: "Worker conformance harness (separate; lives in tests) accepts at
//! least one reference worker that does nothing but use this library."
//!
//! We implement `ReferenceNoOpWorker`, a `Handler` that returns an empty
//! JSON object for every call, and then drive a `WorkerRunner` through one
//! full lifecycle:
//!
//!   register → claim → start → complete
//!
//! using the real `EffectQueue` (in-memory SQLite). If you write a worker
//! that only uses this library and implements `Handler`, you pass this
//! smoke test. Anything more elaborate (sandboxing, cost math, file IO)
//! belongs in the per-worker crate.

use actant_core::{
    now_rfc3339, Actor, ActorId, ActorKind, CommandId, CommandRecord, CommandStatus, RiskLevel,
    Workspace, WorkspaceId,
};
use actant_effects::EffectQueue;
use actant_storage::{Storage, StorageConfig};
use actant_worker_protocol::{Handler, HandlerResult, WorkerDescriptor, WorkerRunner};
use async_trait::async_trait;
use std::time::Duration;

/// Reference worker. Trivially implements `Handler`: every effect returns
/// `{}`. This is the "implement the trait correctly and you pass" baseline.
#[derive(Debug, Default)]
struct ReferenceNoOpWorker;

#[async_trait]
impl Handler for ReferenceNoOpWorker {
    fn effect_type(&self) -> &'static str {
        "noop"
    }

    async fn handle(&self, _input: serde_json::Value) -> HandlerResult {
        Ok(serde_json::json!({}))
    }
}

async fn fresh_workspace() -> (Storage, EffectQueue, WorkspaceId, ActorId, CommandId) {
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "harness".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Worker,
        display_name: "noop-worker".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    s.insert_actor(&actor).await.unwrap();
    let cmd = CommandRecord {
        id: CommandId::new(),
        workspace_id: ws.id.clone(),
        actor_id: actor.id.clone(),
        session_id: None,
        command_type: "test".into(),
        input_inline: None,
        input_hash: "h".into(),
        policy_id: None,
        status: CommandStatus::Committed,
        error: None,
        created_at: now_rfc3339(),
        committed_at: None,
    };
    s.insert_command(&cmd).await.unwrap();
    let q = EffectQueue::new(s.clone());
    (s, q, ws.id, actor.id, cmd.id)
}

#[tokio::test]
async fn noop_handler_round_trips_through_queue_directly() {
    // Direct (non-runner) drive: claim → start → handle → complete.
    let (storage, queue, ws, actor, cmd_id) = fresh_workspace().await;
    let handler = ReferenceNoOpWorker;

    // Enqueue an effect for the noop handler.
    let effect_id = queue
        .enqueue(
            &ws,
            &cmd_id,
            &actor,
            "noop",
            serde_json::json!({"payload": "anything"}),
            RiskLevel::Low,
        )
        .await
        .unwrap();

    // Register the worker.
    let worker_id = actant_core::WorkerId::new();
    let worker_row = actant_core::Worker {
        id: worker_id.clone(),
        workspace_id: ws.clone(),
        actor_id: actor.clone(),
        name: "noop-1".into(),
        host: None,
        version: None,
        status: "online".into(),
        last_heartbeat_at: Some(now_rfc3339()),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    queue.register_worker(&worker_row, &["noop"]).await.unwrap();

    // Claim → start → handle → complete.
    let lease = queue
        .claim_one(&worker_id, &ws, &["noop"])
        .await
        .unwrap()
        .expect("noop effect should be claimable");
    queue.start(&lease.effect_id).await.unwrap();

    let input: serde_json::Value = lease
        .input_inline
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::json!({}));
    let output = handler.handle(input).await.expect("noop must succeed");
    assert_eq!(output, serde_json::json!({}));
    queue.complete(&lease.effect_id, &output).await.unwrap();

    // Heartbeat is also part of the protocol surface — exercise it once.
    queue.heartbeat(&worker_id, 0).await.unwrap();

    // Verify final row state.
    let (status, result_ref): (String, Option<String>) =
        sqlx::query_as("SELECT status, result_ref FROM effect WHERE id = ?")
            .bind(effect_id.as_str())
            .fetch_one(storage.pool())
            .await
            .unwrap();
    assert_eq!(status, "succeeded");
    assert_eq!(result_ref.as_deref(), Some("{}"));
}

#[tokio::test]
async fn noop_handler_through_worker_runner() {
    // End-to-end through `WorkerRunner::run` — confirms that the harness
    // works for a worker that does literally nothing but plug a noop
    // handler into the runner. We send shutdown right after enqueueing so
    // the loop processes the effect and exits.
    let (storage, queue, ws, actor, cmd_id) = fresh_workspace().await;

    let effect_id = queue
        .enqueue(
            &ws,
            &cmd_id,
            &actor,
            "noop",
            serde_json::json!({}),
            RiskLevel::Low,
        )
        .await
        .unwrap();

    let desc = WorkerDescriptor {
        workspace_id: ws.clone(),
        actor_id: actor.clone(),
        name: "noop-runner".into(),
        capabilities: vec!["noop".into()],
    };
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mut runner = WorkerRunner::new(
        queue.clone(),
        desc,
        vec![Box::new(ReferenceNoOpWorker)],
        shutdown_rx,
    );

    // Race the runner against a timed shutdown.
    let run_handle = tokio::spawn(async move { runner.run().await });
    tokio::time::sleep(Duration::from_millis(500)).await;
    shutdown_tx.send(true).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), run_handle)
        .await
        .expect("runner must shut down within 5s");
    result
        .expect("join")
        .expect("runner returned an error rather than shutting down cleanly");

    // The effect should have been processed during the half-second window.
    let (status,): (String,) = sqlx::query_as("SELECT status FROM effect WHERE id = ?")
        .bind(effect_id.as_str())
        .fetch_one(storage.pool())
        .await
        .unwrap();
    assert_eq!(
        status, "succeeded",
        "WorkerRunner should have completed the noop effect before shutdown"
    );
}
