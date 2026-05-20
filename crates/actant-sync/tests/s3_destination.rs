//! S3 destination smoke test. Compiled only with `--features s3`. Runs
//! against the endpoint named in `ACTANTDB_TEST_S3_BUCKET` /
//! `ACTANTDB_TEST_S3_ENDPOINT`, skipping if either env var is missing (same
//! skip pattern as the Ollama provider test and the existing
//! `actant-objectstore::s3_provider` test).

#![cfg(feature = "s3")]

use std::sync::Arc;

use actant_core::*;
use actant_objectstore::S3Config;
use actant_storage::{Storage, StorageConfig};
use actant_sync::{Destination, S3Destination, SyncRunner, SyncRunnerConfig};

fn ev(workspace: &WorkspaceId, actor: &ActorId, session: &SessionId, idx: usize) -> AgentEvent {
    let parent_hash = "0".repeat(64);
    let payload = serde_json::json!({"idx": idx});
    let pc = canonical_json(&payload);
    let ph = sha256_hex(pc.as_bytes());
    AgentEvent {
        id: EventId::from_string(format!("evt_{}_{idx:04}", ulid::Ulid::new())),
        workspace_id: workspace.clone(),
        actor_id: actor.clone(),
        session_id: Some(session.clone()),
        parent_event_id: None,
        event_type: "demo".into(),
        causality_kind: CausalityKind::Audit,
        sensitivity: Sensitivity::Low,
        authority_scope_id: None,
        payload_ref: None,
        payload_inline: Some(pc),
        payload_hash: ph.clone(),
        event_hash: chain_hash(&parent_hash, &ph),
        created_at: format!("2026-05-19T00:00:{:02}Z", idx),
        model_call_id: None,
        tool_call_id: None,
        workflow_run_id: None,
        memory_id: None,
        artifact_id: None,
        command_id: None,
        effect_id: None,
    }
}

#[tokio::test]
async fn s3_destination_roundtrip_when_endpoint_is_configured() {
    let Ok(bucket) = std::env::var("ACTANTDB_TEST_S3_BUCKET") else {
        eprintln!("skipping s3_destination_roundtrip: ACTANTDB_TEST_S3_BUCKET unset");
        return;
    };
    let mut cfg = S3Config::new(&bucket);
    cfg.endpoint = std::env::var("ACTANTDB_TEST_S3_ENDPOINT").ok();
    cfg.region = std::env::var("ACTANTDB_TEST_S3_REGION").ok();
    cfg.access_key_id = std::env::var("ACTANTDB_TEST_S3_ACCESS_KEY").ok();
    cfg.secret_access_key = std::env::var("ACTANTDB_TEST_S3_SECRET_KEY").ok();
    cfg.allow_http = std::env::var("ACTANTDB_TEST_S3_ALLOW_HTTP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let dest = Arc::new(S3Destination::from_config(cfg).expect("S3Destination::from_config"));
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
    for i in 0..5 {
        storage
            .append_event(&ev(&ws.id, &actor.id, &session.id, i))
            .await
            .unwrap();
    }

    let runner = SyncRunner::new(storage.clone(), ws.id.clone(), dest.clone()).with_config(
        SyncRunnerConfig {
            batch_size: 3,
            ..Default::default()
        },
    );
    let stats = runner.run_once().await.expect("run_once");
    assert_eq!(stats.events_pushed, 5);
    let cursor = dest.cursor(&ws.id).await.expect("cursor");
    assert!(cursor.is_some(), "cursor advanced");
}
