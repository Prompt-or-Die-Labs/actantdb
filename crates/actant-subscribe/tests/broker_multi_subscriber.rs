//! Broker: multiple live subscribers each receive a copy of every publish.

use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use actant_subscribe::Broker;

#[tokio::test]
async fn multiple_subscribers_receive_same_message() {
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::from_string("ws_default".to_string()),
        name: "d".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    let broker = Broker::new(storage);

    let mut a = broker.subscribe(&ws.id, "broadcast", None).await.unwrap();
    let mut b = broker.subscribe(&ws.id, "broadcast", None).await.unwrap();
    let mut c = broker.subscribe(&ws.id, "broadcast", None).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let env = broker
        .publish(&ws.id, "broadcast", serde_json::json!({"n": 42}))
        .await
        .unwrap();

    for rx in [&mut a, &mut b, &mut c] {
        let got = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.id, env.id);
        assert_eq!(got.payload["n"], 42);
    }
}
