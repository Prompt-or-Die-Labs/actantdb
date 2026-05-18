//! JWT middleware tests for /v1/command.

use std::net::SocketAddr;

use actant_auth::{sign, Claims};
use actant_core::*;
use actant_server::{router, AppState};
use actant_storage::{Storage, StorageConfig};

async fn start_with_secret(secret: &[u8]) -> String {
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
    let state = AppState::new(storage).with_auth(secret.to_vec());
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

fn fresh_token(secret: &[u8], workspace_id: &str) -> String {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let claims = Claims {
        sub: "act_system".into(),
        iss: workspace_id.into(),
        roles: vec!["admin".into()],
        iat: now,
        exp: now + 60,
    };
    sign(&claims, secret).unwrap()
}

#[tokio::test]
async fn missing_token_returns_401() {
    let base = start_with_secret(b"shared").await;
    let c = reqwest::Client::new();
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
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn valid_token_passes() {
    let secret = b"shared-secret";
    let base = start_with_secret(secret).await;
    let token = fresh_token(secret, "ws_default");
    let c = reqwest::Client::new();
    let r = c
        .post(format!("{base}/v1/command"))
        .bearer_auth(&token)
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

#[tokio::test]
async fn token_with_wrong_workspace_is_rejected() {
    let secret = b"shared-secret";
    let base = start_with_secret(secret).await;
    let token = fresh_token(secret, "ws_some_other_tenant");
    let c = reqwest::Client::new();
    let r = c
        .post(format!("{base}/v1/command"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": "create_session",
            "input": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn bad_signature_is_rejected() {
    let base = start_with_secret(b"right-secret").await;
    let token = fresh_token(b"wrong-secret", "ws_default");
    let c = reqwest::Client::new();
    let r = c
        .post(format!("{base}/v1/command"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": "create_session",
            "input": {}
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn permissions_get_requires_auth_when_secret_set() {
    // The new endpoints share enforce_auth() with /v1/command, so cover one
    // GET, one POST, and one DELETE here. If the helper is dropped from any
    // handler, this fails.
    let secret = b"shared-secret";
    let base = start_with_secret(secret).await;
    let c = reqwest::Client::new();

    // Unauthenticated GET => 401.
    let r = c
        .get(format!("{base}/v1/permissions?workspace_id=ws_default"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401, "GET /v1/permissions should require auth");

    // Unauthenticated POST => 401.
    let r = c
        .post(format!("{base}/v1/setup-reports"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "content": "x",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        401,
        "POST /v1/setup-reports should require auth"
    );

    // Unauthenticated DELETE => 401.
    let r = c
        .delete(format!("{base}/v1/permissions"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "authority_scope_id": "auth_nope",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        401,
        "DELETE /v1/permissions should require auth"
    );

    // With a valid token, GET succeeds.
    let token = fresh_token(secret, "ws_default");
    let r = c
        .get(format!("{base}/v1/permissions?workspace_id=ws_default"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "got {}", r.status());
}
