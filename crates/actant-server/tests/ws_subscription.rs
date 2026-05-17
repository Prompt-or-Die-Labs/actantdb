//! End-to-end: open /v1/ws, dispatch a command via HTTP, assert the
//! WebSocket subscriber receives a broadcast.

use std::net::SocketAddr;

use actant_core::*;
use actant_server::{router, AppState};
use actant_storage::{Storage, StorageConfig};
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

async fn start() -> String {
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::from_string("ws_default".to_string()),
        name: "d".into(),
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
    let state = AppState::new(storage);
    let router = router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let bound = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("{bound}")
}

#[tokio::test]
async fn subscriber_receives_command_broadcast() {
    let addr = start().await;
    let ws_url = format!("ws://{addr}/v1/ws?workspace_id=ws_default&kind=events");

    let (mut socket, _resp) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
    // Send a ping to make sure the upgrade completed before we trigger the
    // broadcast. The server doesn't have to reply.
    let _ = socket.send(Message::Ping(Vec::new())).await;
    // Give the subscriber a moment to register with the hub.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let http_url = format!("http://{addr}/v1/command");
    let c = reqwest::Client::new();
    let r = c
        .post(&http_url)
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": "create_session",
            "input": {}
        }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());

    // Wait for the first non-control message on the socket. Allow 2s for it.
    let timed = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while let Some(Ok(msg)) = socket.next().await {
            if let Message::Text(t) = msg {
                return Some::<String>(t.to_string());
            }
        }
        None
    })
    .await
    .ok()
    .flatten();
    let text = timed.expect("expected a broadcast on the WS within 2s");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["topic"]["workspace_id"], "ws_default");
    assert_eq!(v["topic"]["kind"], "events");
    assert!(v["payload"]["command_id"].is_string());
}
