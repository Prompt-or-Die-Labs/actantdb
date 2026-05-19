//! IPFS destination smoke test. Compiled only with `--features ipfs`. Runs
//! against the Kubo HTTP API at `ACTANTDB_TEST_IPFS_API` (default
//! `http://localhost:5001`) when `ACTANTDB_TEST_IPFS=1` is set; skips
//! otherwise.

#![cfg(feature = "ipfs")]

use std::sync::Arc;

use actant_core::*;
use actant_objectstore::IpfsConfig;
use actant_storage::{Storage, StorageConfig};
use actant_sync::{Destination, IpfsDestination, SyncRunner, SyncRunnerConfig};

#[tokio::test]
async fn ipfs_destination_roundtrip_when_daemon_is_running() {
    if std::env::var("ACTANTDB_TEST_IPFS").ok().as_deref() != Some("1") {
        eprintln!("skipping ipfs_destination_roundtrip: ACTANTDB_TEST_IPFS != 1");
        return;
    }
    let cfg = IpfsConfig {
        base_url: std::env::var("ACTANTDB_TEST_IPFS_API")
            .unwrap_or_else(|_| "http://localhost:5001".into()),
        gateway: std::env::var("ACTANTDB_TEST_IPFS_GATEWAY").ok(),
    };
    let dest = Arc::new(IpfsDestination::from_config(cfg).expect("IpfsDestination::from_config"));

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
    for i in 0..2 {
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
    assert_eq!(stats.events_pushed, 2);
    assert!(dest.cursor(&ws.id).await.expect("cursor").is_some());
}
