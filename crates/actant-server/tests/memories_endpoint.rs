//! /v1/memories list with status filtering. Uses /v1/command to seed
//! candidates and approved memories through the canonical lifecycle.

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

async fn dispatch(
    c: &reqwest::Client,
    base: &str,
    cmd: &str,
    input: serde_json::Value,
) -> serde_json::Value {
    let r = c
        .post(format!("{base}/v1/command"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": cmd,
            "input": input,
        }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{cmd} failed: {}", r.status());
    r.json().await.unwrap()
}

#[tokio::test]
async fn pending_then_approved_round_trip() {
    let base = start().await;
    let c = reqwest::Client::new();

    let prop = dispatch(
        &c,
        &base,
        "propose_memory",
        serde_json::json!({"text": "user prefers tabs", "category": "preference"}),
    )
    .await;
    let candidate_id = prop["result"]["memory_candidate_id"].as_str().unwrap();

    // Status=pending should show the candidate.
    let r: serde_json::Value = c
        .get(format!(
            "{base}/v1/memories?workspace_id=ws_default&status=pending"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mems = r["memories"].as_array().unwrap();
    assert_eq!(mems.len(), 1);
    assert_eq!(mems[0]["id"].as_str().unwrap(), candidate_id);
    assert_eq!(mems[0]["status"].as_str().unwrap(), "pending");

    // Approve it.
    let appr = dispatch(
        &c,
        &base,
        "approve_memory",
        serde_json::json!({"memory_candidate_id": candidate_id}),
    )
    .await;
    let memory_id = appr["result"]["memory_id"].as_str().unwrap();

    // Status=approved should return the new memory row.
    let r: serde_json::Value = c
        .get(format!(
            "{base}/v1/memories?workspace_id=ws_default&status=approved"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mems = r["memories"].as_array().unwrap();
    assert_eq!(mems.len(), 1);
    assert_eq!(mems[0]["id"].as_str().unwrap(), memory_id);
    assert_eq!(mems[0]["status"].as_str().unwrap(), "approved");
    assert_eq!(mems[0]["text"].as_str().unwrap(), "user prefers tabs");
}

#[tokio::test]
async fn status_all_unions_both_tables() {
    let base = start().await;
    let c = reqwest::Client::new();

    // 1 candidate that we'll approve.
    let prop = dispatch(
        &c,
        &base,
        "propose_memory",
        serde_json::json!({"text": "fact one"}),
    )
    .await;
    let cid = prop["result"]["memory_candidate_id"]
        .as_str()
        .unwrap()
        .to_string();
    dispatch(
        &c,
        &base,
        "approve_memory",
        serde_json::json!({"memory_candidate_id": cid}),
    )
    .await;

    // 1 candidate left pending.
    dispatch(
        &c,
        &base,
        "propose_memory",
        serde_json::json!({"text": "fact two"}),
    )
    .await;

    let r: serde_json::Value = c
        .get(format!(
            "{base}/v1/memories?workspace_id=ws_default&status=all"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mems = r["memories"].as_array().unwrap();
    // Approved-then-promoted candidates must NOT also surface as pending.
    assert_eq!(mems.len(), 2, "got {} memories", mems.len());
    let statuses: Vec<&str> = mems.iter().map(|m| m["status"].as_str().unwrap()).collect();
    assert!(statuses.contains(&"approved"));
    assert!(statuses.contains(&"pending"));
    let pending_text = mems
        .iter()
        .find(|m| m["status"].as_str() == Some("pending"))
        .and_then(|m| m["text"].as_str())
        .unwrap();
    assert_eq!(pending_text, "fact two");
}

#[tokio::test]
async fn unknown_status_returns_400() {
    let base = start().await;
    let c = reqwest::Client::new();
    let r = c
        .get(format!(
            "{base}/v1/memories?workspace_id=ws_default&status=bogus"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}
