//! Broker: a single publish must reach a single live subscriber.

use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use actant_subscribe::Broker;

async fn boot() -> (Storage, WorkspaceId) {
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::from_string("ws_default".to_string()),
        name: "d".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    (storage, ws.id)
}

#[tokio::test]
async fn single_publish_delivered_to_single_subscriber() {
    let (storage, ws) = boot().await;
    let broker = Broker::new(storage);

    let mut rx = broker.subscribe(&ws, "alerts", None).await.unwrap();
    // Subscribers must register before publish to receive live messages.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let env_pub = broker
        .publish(&ws, "alerts", serde_json::json!({"kind":"sev1"}))
        .await
        .unwrap();
    let env_rx = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(env_rx.id, env_pub.id);
    assert_eq!(env_rx.topic, "alerts");
    assert_eq!(env_rx.payload["kind"], "sev1");
}
