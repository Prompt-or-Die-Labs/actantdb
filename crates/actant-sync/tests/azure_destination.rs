//! Azure destination smoke test. Compiled only with `--features azure`. Runs
//! when `ACTANTDB_TEST_AZURE_ACCOUNT` + `ACTANTDB_TEST_AZURE_CONTAINER` are
//! set; skips otherwise.

#![cfg(feature = "azure")]

use std::sync::Arc;

use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use actant_sync::{
    AzureConfig, AzureDestination, Destination, SyncRunner, SyncRunnerConfig,
};

#[tokio::test]
async fn azure_destination_roundtrip_when_configured() {
    let (Ok(account), Ok(container)) = (
        std::env::var("ACTANTDB_TEST_AZURE_ACCOUNT"),
        std::env::var("ACTANTDB_TEST_AZURE_CONTAINER"),
    ) else {
        eprintln!(
            "skipping azure_destination_roundtrip: ACTANTDB_TEST_AZURE_ACCOUNT / \
             ACTANTDB_TEST_AZURE_CONTAINER unset"
        );
        return;
    };
    let mut cfg = AzureConfig::new(&account, &container);
    cfg.access_key = std::env::var("ACTANTDB_TEST_AZURE_KEY").ok();
    cfg.allow_http = std::env::var("ACTANTDB_TEST_AZURE_ALLOW_HTTP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let dest =
        Arc::new(AzureDestination::from_config(cfg).expect("AzureDestination::from_config"));

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
