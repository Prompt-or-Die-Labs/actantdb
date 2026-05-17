//! Verify the three health-probe endpoints.

use std::net::SocketAddr;

use actant_core::*;
use actant_server::{router, AppState};
use actant_storage::{Storage, StorageConfig};

async fn start() -> String {
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::from_string("ws_default".to_string()),
        name: "d".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    let state = AppState::new(storage);
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
async fn all_three_probes_return_200() {
    let base = start().await;
    let c = reqwest::Client::new();
    for path in [
        "/v1/healthz/startup",
        "/v1/healthz/live",
        "/v1/healthz/ready",
    ] {
        let r = c.get(format!("{base}{path}")).send().await.unwrap();
        assert!(r.status().is_success(), "{path} returned {}", r.status());
    }
}

#[tokio::test]
async fn request_id_header_is_echoed() {
    let base = start().await;
    let c = reqwest::Client::new();
    let r = c
        .get(format!("{base}/v1/healthz"))
        .header("x-request-id", "test-rid-42")
        .send()
        .await
        .unwrap();
    let echoed = r.headers().get("x-request-id").unwrap().to_str().unwrap();
    assert_eq!(echoed, "test-rid-42");
}

#[tokio::test]
async fn request_id_generated_when_missing() {
    let base = start().await;
    let c = reqwest::Client::new();
    let r = c.get(format!("{base}/v1/healthz")).send().await.unwrap();
    let rid = r.headers().get("x-request-id").unwrap().to_str().unwrap();
    assert!(
        rid.starts_with("req_"),
        "expected generated request id, got: {rid}"
    );
}
