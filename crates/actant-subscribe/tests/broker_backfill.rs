//! Broker: a subscriber that connects with a `since` cursor receives every
//! persisted envelope with id > cursor before live tail.

use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use actant_subscribe::Broker;

#[tokio::test]
async fn subscriber_with_cursor_backfills_then_tails() {
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::from_string("ws_default".to_string()),
        name: "d".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    let broker = Broker::new(storage);

    // Publish 3 envelopes; remember the first id as the cursor.
    let _e1 = broker
        .publish(&ws.id, "log", serde_json::json!({"i": 1}))
        .await
        .unwrap();
    let e2 = broker
        .publish(&ws.id, "log", serde_json::json!({"i": 2}))
        .await
        .unwrap();
    let e3 = broker
        .publish(&ws.id, "log", serde_json::json!({"i": 3}))
        .await
        .unwrap();

    // Subscribe with the first envelope's id as cursor — should receive e2, e3.
    let mut rx = broker
        .subscribe(&ws.id, "log", Some(_e1.id.clone()))
        .await
        .unwrap();

    let got1 = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got1.id, e2.id);
    let got2 = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got2.id, e3.id);

    // Now publish a new envelope; it should arrive on the same receiver
    // (live-tail after backfill).
    let e4 = broker
        .publish(&ws.id, "log", serde_json::json!({"i": 4}))
        .await
        .unwrap();
    let got3 = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got3.id, e4.id);
}
