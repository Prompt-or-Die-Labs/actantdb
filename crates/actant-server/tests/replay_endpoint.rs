//! HTTP /v1/replay/{checkpoint,run} round-trip.

use std::net::SocketAddr;

use actant_core::*;
use actant_server::{router, AppState};
use actant_storage::{Storage, StorageConfig};

async fn start_with_seed() -> (String, Storage, EventId) {
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::from_string("ws_default".to_string()),
        name: "d".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::from_string("act_system".to_string()),
        workspace_id: ws.id.clone(),
        kind: ActorKind::System,
        display_name: "system".into(),
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
    // One model_call event we can checkpoint to.
    let payload = serde_json::json!({"summary":"plan"});
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
    storage.append_event(&e).await.unwrap();
    let event_id = e.id.clone();

    let state = AppState::new(storage.clone());
    let router = router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let bound = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (format!("http://{bound}"), storage, event_id)
}

#[tokio::test]
async fn checkpoint_then_run_model_replay() {
    let (base, _s, eid) = start_with_seed().await;
    let c = reqwest::Client::new();

    // 1. checkpoint
    let r = c
        .post(format!("{base}/v1/replay/checkpoint"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "event_id": eid.as_str()
        }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "checkpoint failed: {}", r.status());
    let cp = r.json::<serde_json::Value>().await.unwrap();
    let cp_id = cp["checkpoint_id"].as_str().unwrap().to_string();

    // 2. run
    let r = c
        .post(format!("{base}/v1/replay/run"))
        .json(&serde_json::json!({
            "actor_id": "act_system",
            "checkpoint_id": cp_id,
            "mode": "model"
        }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "run failed: {}", r.status());
    let diff = r.json::<serde_json::Value>().await.unwrap();
    let entries = diff["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert!(entries.iter().any(|e| e["kind"] == "changed"));
}

#[tokio::test]
async fn openapi_endpoint_serves_yaml() {
    let (base, _s, _e) = start_with_seed().await;
    let c = reqwest::Client::new();
    let r = c
        .get(format!("{base}/v1/openapi.yaml"))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let body = r.text().await.unwrap();
    assert!(body.contains("openapi:"));
    assert!(body.contains("/v1/command"));
    assert!(body.contains("/v1/replay/run"));
}
