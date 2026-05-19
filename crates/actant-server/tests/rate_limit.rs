//! Verify the per-workspace rate limiter on /v1/command.

use std::net::SocketAddr;

use actant_core::*;
use actant_server::{router, AppState};
use actant_storage::{Storage, StorageConfig};

async fn start_with_limit(limit: u32) -> String {
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
    let state = AppState::new(storage).with_rate_limit(actant_reliability::throttle::Policy {
        limit,
        // Very slow refill so the burst is the only resource.
        refill_per_second: 0.001,
    });
    let router = router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let bound = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{bound}")
}

#[tokio::test]
async fn rate_limit_returns_429_when_burst_exhausted() {
    let base = start_with_limit(3).await;
    let c = reqwest::Client::new();
    // Drain the bucket.
    for _ in 0..3 {
        let r = c
            .post(format!("{base}/v1/command"))
            .json(&serde_json::json!({
                "workspace_id": "ws_default",
                "actor_id": "act_system",
                "command_type": "create_session",
                "input": {}
            }))
            .send()
            .await
            .unwrap();
        assert!(r.status().is_success(), "got {}", r.status());
    }
    // 4th must be rate-limited.
    let r = c
        .post(format!("{base}/v1/command"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": "create_session",
            "input": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 429);
    assert!(r.headers().get("retry-after").is_some());
}
