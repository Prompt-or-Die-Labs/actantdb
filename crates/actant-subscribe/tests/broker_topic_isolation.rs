//! Broker: publishing to topic A does not reach subscribers of topic B.

use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use actant_subscribe::Broker;

#[tokio::test]
async fn topic_isolation() {
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::from_string("ws_default".to_string()),
        name: "d".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    let broker = Broker::new(storage);

    let mut rx_b = broker.subscribe(&ws.id, "B", None).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // Publish to A only.
    broker
        .publish(&ws.id, "A", serde_json::json!({"x": 1}))
        .await
        .unwrap();

    // B receiver must not see it within a reasonable window.
    let r = tokio::time::timeout(std::time::Duration::from_millis(200), rx_b.recv()).await;
    assert!(r.is_err(), "topic B saw a message destined for topic A");

    // Sanity: a publish to B does arrive.
    broker
        .publish(&ws.id, "B", serde_json::json!({"x": 2}))
        .await
        .unwrap();
    let got = tokio::time::timeout(std::time::Duration::from_secs(1), rx_b.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.payload["x"], 2);
}
