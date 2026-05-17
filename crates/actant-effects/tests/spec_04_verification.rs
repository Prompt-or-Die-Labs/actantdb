//! Spec 04 — effect protocol verification.
//!
//! The spec's `## Verification` makes 4 claims:
//!   1. Every effect type listed here has at least one worker capability in
//!      a Phase 1 reference worker.
//!   2. Every status in the lifecycle has at least one transition that
//!      produces a Chronicle event.
//!   3. The idempotency invariant holds against the schema.
//!   4. Replay can reproduce effect outcomes by reading the `effect_result`
//!      table without re-executing the worker.

use std::fs;
use std::path::Path;

use actant_core::*;
use actant_effects::EffectQueue;
use actant_storage::{Storage, StorageConfig};

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn every_documented_effect_type_has_a_reference_worker() {
    // Spec 04 names these effect types:
    let documented = [
        ("shell.run", "crates/actant-worker-shell"),
        ("file.read", "crates/actant-worker-file"),
        ("file.write", "crates/actant-worker-file"),
        ("model.call", "crates/actant-worker-model"),
        ("mcp.call", "crates/actant-worker-mcp"),
        ("browser.navigate", "crates/actant-worker-browser"),
        ("email.send", "crates/actant-worker-email"),
        ("slack.post", "crates/actant-worker-slack"),
    ];
    for (effect_type, worker_crate) in documented {
        let lib = read_repo(&format!("{worker_crate}/src/lib.rs"));
        assert!(
            lib.contains(&format!("\"{effect_type}\"")),
            "{worker_crate} doesn't declare effect_type {effect_type}"
        );
    }
}

#[tokio::test]
async fn idempotency_invariant_holds_at_the_schema_level() {
    // Spec 04 + spec 02: `UNIQUE (workspace_id, idempotency_key)` on the
    // `effect` table. A second enqueue with the same key returns the same
    // effect (or errors with a conflict).
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "x".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::System,
        display_name: "x".into(),
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
    let q = EffectQueue::new(s.clone());
    let id1 = q
        .enqueue(
            &ws.id,
            &cmd.id,
            &actor.id,
            "shell.run",
            serde_json::json!({}),
            RiskLevel::Low,
        )
        .await
        .unwrap();
    // Insert an effect with the same idempotency_key — must fail.
    let dup = sqlx::query(
        "INSERT INTO effect (id, workspace_id, command_id, requested_by_actor_id,
                             effect_type, status, risk_level, idempotency_key,
                             input_hash, attempt_count, max_attempts, created_at)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(EffectId::new().as_str())
    .bind(ws.id.as_str())
    .bind(cmd.id.as_str())
    .bind(actor.id.as_str())
    .bind("shell.run")
    .bind("pending")
    .bind("low")
    .bind("k1")
    .bind("h")
    .bind(0i64)
    .bind(3i64)
    .bind(now_rfc3339())
    .execute(s.pool())
    .await;
    assert!(dup.is_ok());
    let dup2 = sqlx::query(
        "INSERT INTO effect (id, workspace_id, command_id, requested_by_actor_id,
                             effect_type, status, risk_level, idempotency_key,
                             input_hash, attempt_count, max_attempts, created_at)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(EffectId::new().as_str())
    .bind(ws.id.as_str())
    .bind(cmd.id.as_str())
    .bind(actor.id.as_str())
    .bind("shell.run")
    .bind("pending")
    .bind("low")
    .bind("k1") // same idempotency_key
    .bind("h")
    .bind(0i64)
    .bind(3i64)
    .bind(now_rfc3339())
    .execute(s.pool())
    .await;
    assert!(
        dup2.is_err(),
        "second enqueue with same idempotency_key must conflict"
    );
    let _ = id1;
}

#[tokio::test]
async fn effect_result_table_is_writeable_for_replay_reuse() {
    // Spec 04: "Replay can reproduce effect outcomes by reading the
    // effect_result table without re-executing the worker."
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "x".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::System,
        display_name: "x".into(),
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
    let q = EffectQueue::new(s.clone());
    let eid = q
        .enqueue(
            &ws.id,
            &cmd.id,
            &actor.id,
            "shell.run",
            serde_json::json!({}),
            RiskLevel::Low,
        )
        .await
        .unwrap();
    q.complete(&eid, &serde_json::json!({"ok": true}))
        .await
        .unwrap();
    // effect_result row exists.
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM effect_result WHERE effect_id = ?")
        .bind(eid.as_str())
        .fetch_one(s.pool())
        .await
        .unwrap();
    assert_eq!(n, 1);
}
