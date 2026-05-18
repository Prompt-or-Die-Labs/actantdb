//! /v1/scout-records — POST then GET all, and GET filtered by source.

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
    format!("http://{bound}")
}

async fn post_record(c: &reqwest::Client, base: &str, source: &str, content: &str) {
    let r = c
        .post(format!("{base}/v1/scout-records"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "source_id": source,
            "kind": "screenshot",
            "sensitivity": "low",
            "content": content,
            "metadata": {"app": source},
        }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "got {}", r.status());
}

#[tokio::test]
async fn post_then_get_all_returns_record() {
    let base = start().await;
    let c = reqwest::Client::new();
    post_record(&c, &base, "browser", "scout content alpha").await;

    let r: serde_json::Value = c
        .get(format!("{base}/v1/scout-records?workspace_id=ws_default"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let records = r["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["source_id"].as_str().unwrap(), "browser");
    assert_eq!(records[0]["kind"].as_str().unwrap(), "screenshot");
    assert_eq!(records[0]["sensitivity"].as_str().unwrap(), "low");
    assert_eq!(
        records[0]["content"].as_str().unwrap(),
        "scout content alpha"
    );
    assert_eq!(records[0]["metadata"]["app"].as_str().unwrap(), "browser");
    assert!(records[0]["event_id"].as_str().unwrap().starts_with("evt_"));
}

#[tokio::test]
async fn source_filter_narrows_results() {
    let base = start().await;
    let c = reqwest::Client::new();
    post_record(&c, &base, "browser", "b1").await;
    post_record(&c, &base, "slack", "s1").await;
    post_record(&c, &base, "browser", "b2").await;

    let r: serde_json::Value = c
        .get(format!(
            "{base}/v1/scout-records?workspace_id=ws_default&source=slack"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let records = r["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["source_id"].as_str().unwrap(), "slack");
    assert_eq!(records[0]["content"].as_str().unwrap(), "s1");
}

#[tokio::test]
async fn invalid_sensitivity_returns_400() {
    let base = start().await;
    let c = reqwest::Client::new();
    let r = c
        .post(format!("{base}/v1/scout-records"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "source_id": "browser",
            "kind": "screenshot",
            "sensitivity": "ultraspicy",
            "content": "x",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}
