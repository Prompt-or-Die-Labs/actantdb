//! Broker: workspace isolation — a subscriber on ws_a does not receive
//! envelopes published to the same topic name in ws_b, even on backfill.

use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use actant_subscribe::Broker;

#[tokio::test]
async fn workspace_isolation() {
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws_a = Workspace {
        id: WorkspaceId::from_string("ws_a".to_string()),
        name: "a".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    let ws_b = Workspace {
        id: WorkspaceId::from_string("ws_b".to_string()),
        name: "b".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws_a).await.unwrap();
    storage.insert_workspace(&ws_b).await.unwrap();
    let broker = Broker::new(storage);

    let mut rx_a = broker.subscribe(&ws_a.id, "notify", None).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // Publish to the same topic name in workspace B.
    broker
        .publish(&ws_b.id, "notify", serde_json::json!({"from": "b"}))
        .await
        .unwrap();

    // ws_a must not see it.
    let r = tokio::time::timeout(std::time::Duration::from_millis(200), rx_a.recv()).await;
    assert!(r.is_err(), "ws_a saw a message published in ws_b");

    // Backfill from cursor 0 in ws_b returns only the ws_b row.
    let backfill_b = broker.replay_since(&ws_b.id, "notify", "").await.unwrap();
    assert_eq!(backfill_b.len(), 1);
    assert_eq!(backfill_b[0].payload["from"], "b");

    // Backfill from cursor 0 in ws_a returns nothing.
    let backfill_a = broker.replay_since(&ws_a.id, "notify", "").await.unwrap();
    assert!(backfill_a.is_empty(), "ws_a backfill leaked ws_b rows");
}
