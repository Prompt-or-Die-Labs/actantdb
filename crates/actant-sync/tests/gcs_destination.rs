//! GCS destination smoke test. Compiled only with `--features gcs`. Runs
//! when `ACTANTDB_TEST_GCS_BUCKET` is set; skips otherwise.

#![cfg(feature = "gcs")]

use std::sync::Arc;

use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use actant_sync::{Destination, GcsConfig, GcsDestination, SyncRunner, SyncRunnerConfig};

#[tokio::test]
async fn gcs_destination_roundtrip_when_bucket_is_configured() {
    let Ok(bucket) = std::env::var("ACTANTDB_TEST_GCS_BUCKET") else {
        eprintln!("skipping gcs_destination_roundtrip: ACTANTDB_TEST_GCS_BUCKET unset");
        return;
    };
    let mut cfg = GcsConfig::new(&bucket);
    cfg.service_account_path = std::env::var("ACTANTDB_TEST_GCS_SERVICE_ACCOUNT_PATH").ok();
    cfg.service_account_key = std::env::var("ACTANTDB_TEST_GCS_SERVICE_ACCOUNT_KEY").ok();
    let dest = Arc::new(GcsDestination::from_config(cfg).expect("GcsDestination::from_config"));

    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::from_string(format!("ws_test_{}", ulid::Ulid::new())),
        name: "n".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Human,
        display_name: "a".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    storage.insert_actor(&actor).await.unwrap();
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
    storage.insert_session(&session).await.unwrap();
    let parent_hash = "0".repeat(64);
    for i in 0..3 {
        let payload = serde_json::json!({"idx": i});
        let pc = canonical_json(&payload);
        let ph = sha256_hex(pc.as_bytes());
        storage
            .append_event(&AgentEvent {
                id: EventId::from_string(format!("evt_{}_{i:04}", ulid::Ulid::new())),
                workspace_id: ws.id.clone(),
                actor_id: actor.id.clone(),
                session_id: Some(session.id.clone()),
                parent_event_id: None,
                event_type: "demo".into(),
                causality_kind: CausalityKind::Audit,
                sensitivity: Sensitivity::Low,
                authority_scope_id: None,
                payload_ref: None,
                payload_inline: Some(pc),
                payload_hash: ph.clone(),
                event_hash: chain_hash(&parent_hash, &ph),
                created_at: format!("2026-05-19T00:00:{i:02}Z"),
                model_call_id: None,
                tool_call_id: None,
                workflow_run_id: None,
                memory_id: None,
                artifact_id: None,
                command_id: None,
                effect_id: None,
            })
            .await
            .unwrap();
    }

    let runner = SyncRunner::new(storage.clone(), ws.id.clone(), dest.clone())
        .with_config(SyncRunnerConfig::default());
    let stats = runner.run_once().await.expect("run_once");
    assert_eq!(stats.events_pushed, 3);
    assert!(dest.cursor(&ws.id).await.expect("cursor").is_some());
}
