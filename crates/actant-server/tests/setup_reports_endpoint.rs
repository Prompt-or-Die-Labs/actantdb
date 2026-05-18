//! /v1/setup-reports POST then GET round-trip.

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

#[tokio::test]
async fn post_then_get_latest_round_trips_content() {
    let base = start().await;
    let c = reqwest::Client::new();

    let body = "macOS 14.5\nXcode 16.0\nClaude Code 1.0";
    let r: serde_json::Value = c
        .post(format!("{base}/v1/setup-reports"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "content": body,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let art = r["artifact_id"].as_str().unwrap().to_string();
    let evt = r["event_id"].as_str().unwrap().to_string();
    assert!(art.starts_with("art_"));
    assert!(evt.starts_with("evt_"));

    let r: serde_json::Value = c
        .get(format!(
            "{base}/v1/setup-reports?workspace_id=ws_default&latest=true"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let report = &r["report"];
    assert_eq!(report["artifact_id"].as_str().unwrap(), art);
    assert_eq!(report["event_id"].as_str().unwrap(), evt);
    assert_eq!(report["content"].as_str().unwrap(), body);
    assert_eq!(report["bytes"].as_i64().unwrap(), body.len() as i64);
    assert_eq!(
        report["content_hash"].as_str().unwrap().len(),
        64,
        "sha256 hex"
    );
}

#[tokio::test]
async fn get_latest_with_no_reports_returns_null() {
    let base = start().await;
    let c = reqwest::Client::new();
    let r: serde_json::Value = c
        .get(format!(
            "{base}/v1/setup-reports?workspace_id=ws_default&latest=true"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(r["report"].is_null());
}

#[tokio::test]
async fn get_list_orders_newest_first() {
    let base = start().await;
    let c = reqwest::Client::new();
    for i in 0..3 {
        let _ = c
            .post(format!("{base}/v1/setup-reports"))
            .json(&serde_json::json!({
                "workspace_id": "ws_default",
                "actor_id": "act_system",
                "content": format!("report {i}"),
            }))
            .send()
            .await
            .unwrap();
        // Tiny pause ensures the RFC3339 timestamp advances (subsecond
        // precision is fine, but back-to-back inserts in the same ms would
        // collide on `created_at` ordering).
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let r: serde_json::Value = c
        .get(format!("{base}/v1/setup-reports?workspace_id=ws_default"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let reports = r["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0]["content"].as_str().unwrap(), "report 2");
    assert_eq!(reports[2]["content"].as_str().unwrap(), "report 0");
}
