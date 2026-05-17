//! /v1/sync/since cluster-sync wire protocol.

use std::net::SocketAddr;

use actant_command::Engine;
use actant_core::*;
use actant_server::{router, AppState};
use actant_storage::{Storage, StorageConfig};

async fn start_with_events() -> (String, Vec<String>) {
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

    let engine = Engine::new(storage.clone());
    // Generate three Chronicle events through the command engine.
    let mut event_ids = Vec::new();
    for _ in 0..3 {
        let r = engine
            .dispatch(
                &ws.id,
                &actor.id,
                "create_session",
                serde_json::json!({}),
                None,
            )
            .await
            .unwrap();
        if let Some(eid) = r.event_id {
            event_ids.push(eid.as_str().to_string());
        }
    }

    let state = AppState::new(storage);
    let router = router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let bound = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (format!("http://{bound}"), event_ids)
}

#[tokio::test]
async fn sync_since_returns_all_events_from_start() {
    let (base, ids) = start_with_events().await;
    let c = reqwest::Client::new();
    let r = c
        .post(format!("{base}/v1/sync/since"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "since_event_id": "",
            "limit": 10
        }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let body: serde_json::Value = r.json().await.unwrap();
    let events = body["events"].as_array().unwrap();
    assert!(events.len() >= ids.len(), "should return all events");
    assert!(body["next_since"].is_string());
}

#[tokio::test]
async fn sync_since_skips_already_seen() {
    let (base, ids) = start_with_events().await;
    let c = reqwest::Client::new();
    // First event id as "since" — should return events 2+, not event 1.
    let since = &ids[0];
    let r = c
        .post(format!("{base}/v1/sync/since"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "since_event_id": since,
            "limit": 10
        }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    let events = body["events"].as_array().unwrap();
    for e in events {
        assert!(
            e["id"].as_str().unwrap() > since.as_str(),
            "{e} not > {since}"
        );
    }
}

#[tokio::test]
async fn sync_since_idempotent_repeat_returns_same_set() {
    let (base, _) = start_with_events().await;
    let c = reqwest::Client::new();
    let r1: serde_json::Value = c
        .post(format!("{base}/v1/sync/since"))
        .json(&serde_json::json!({"workspace_id":"ws_default","since_event_id":"","limit":5}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let r2: serde_json::Value = c
        .post(format!("{base}/v1/sync/since"))
        .json(&serde_json::json!({"workspace_id":"ws_default","since_event_id":"","limit":5}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ids_1: Vec<&str> = r1["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    let ids_2: Vec<&str> = r2["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids_1, ids_2);
}
