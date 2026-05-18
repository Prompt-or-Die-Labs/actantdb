//! /v1/permissions create / list / revoke.

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
async fn create_then_list_returns_grant() {
    let base = start().await;
    let c = reqwest::Client::new();

    let r = c
        .post(format!("{base}/v1/permissions"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "permission": "file.read",
            "level": "medium",
            "scope": "~/Projects/**",
            "allowed_actions": ["read", "list"],
        }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "got {}", r.status());
    let body: serde_json::Value = r.json().await.unwrap();
    let id = body["id"].as_str().unwrap();
    assert!(id.starts_with("auth_"));

    let r = c
        .get(format!("{base}/v1/permissions?workspace_id=ws_default"))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let body: serde_json::Value = r.json().await.unwrap();
    let perms = body["permissions"].as_array().unwrap();
    assert_eq!(perms.len(), 1);
    assert_eq!(perms[0]["id"].as_str().unwrap(), id);
    assert_eq!(perms[0]["permission"].as_str().unwrap(), "file.read");
    assert_eq!(perms[0]["sensitivity_ceiling"].as_str().unwrap(), "medium");
    assert_eq!(
        perms[0]["resource_pattern"].as_str().unwrap(),
        "~/Projects/**"
    );
    let actions = perms[0]["allowed_actions"].as_array().unwrap();
    assert_eq!(actions.len(), 2);
}

#[tokio::test]
async fn delete_soft_revokes_so_list_skips_it() {
    let base = start().await;
    let c = reqwest::Client::new();

    let body: serde_json::Value = c
        .post(format!("{base}/v1/permissions"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "permission": "shell.exec",
            "level": "high",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = body["id"].as_str().unwrap().to_string();

    let r = c
        .delete(format!("{base}/v1/permissions"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "authority_scope_id": id,
        }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "got {}", r.status());
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["ok"], true);

    let r = c
        .get(format!("{base}/v1/permissions?workspace_id=ws_default"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    let perms = body["permissions"].as_array().unwrap();
    assert!(perms.is_empty(), "revoked grant should not appear");
}

#[tokio::test]
async fn invalid_level_returns_400() {
    let base = start().await;
    let c = reqwest::Client::new();
    let r = c
        .post(format!("{base}/v1/permissions"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "permission": "shell.exec",
            "level": "spicy",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}

#[tokio::test]
async fn delete_unknown_id_returns_404() {
    let base = start().await;
    let c = reqwest::Client::new();
    let r = c
        .delete(format!("{base}/v1/permissions"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "authority_scope_id": "auth_nope",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}
