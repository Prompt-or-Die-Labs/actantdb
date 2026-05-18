//! /v1/memories/conflicts — seed two opposing memories, run detection
//! directly via `MemoryStore`, then hit the endpoint.

use std::net::SocketAddr;

use actant_core::*;
use actant_memory::MemoryStore;
use actant_server::{router, AppState};
use actant_storage::{Storage, StorageConfig};

async fn start_with_storage() -> (String, Storage, WorkspaceId) {
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::from_string("ws_conflict".to_string()),
        name: "c".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    storage
        .insert_actor(&Actor {
            id: ActorId::from_string("act_system".to_string()),
            workspace_id: ws.id.clone(),
            kind: ActorKind::System,
            display_name: "system".into(),
            created_at: now_rfc3339(),
            disabled_at: None,
        })
        .await
        .unwrap();
    let state = AppState::new(storage.clone());
    let router = router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let bound = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (format!("http://{bound}"), storage, ws.id)
}

async fn insert_mem(s: &Storage, ws: &WorkspaceId, text: &str) {
    let id = MemoryId::new();
    sqlx::query(
        "INSERT INTO memory
            (id, workspace_id, text, category, sensitivity, scope,
             source_event_ids, usage_count, created_at)
         VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(id.as_str())
    .bind(ws.as_str())
    .bind(text)
    .bind("fact")
    .bind("low")
    .bind("global")
    .bind("[]")
    .bind(0i64)
    .bind(now_rfc3339())
    .execute(s.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn detected_conflicts_are_returned() {
    let (base, storage, ws) = start_with_storage().await;
    insert_mem(&storage, &ws, "the project always uses pytest").await;
    insert_mem(&storage, &ws, "the project never uses pytest").await;

    let store = MemoryStore::new(storage.clone());
    let inserted = store.detect_conflicts(&ws, 0.3).await.unwrap();
    assert_eq!(inserted, 1);

    let c = reqwest::Client::new();
    let r: serde_json::Value = c
        .get(format!(
            "{base}/v1/memories/conflicts?workspace_id=ws_conflict"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let conflicts = r["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0]["conflict_type"].as_str().unwrap(), "polarity");
    assert_eq!(
        conflicts[0]["workspace_id"].as_str().unwrap(),
        "ws_conflict"
    );
}

#[tokio::test]
async fn no_conflicts_returns_empty_list() {
    let (base, _storage, _ws) = start_with_storage().await;
    let c = reqwest::Client::new();
    let r: serde_json::Value = c
        .get(format!(
            "{base}/v1/memories/conflicts?workspace_id=ws_conflict"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(r["conflicts"].as_array().unwrap().is_empty());
}
