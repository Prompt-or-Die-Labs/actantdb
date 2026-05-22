//! WebSocket + HTTP surface for the persistent pub/sub broker
//! (DEVX_GAPS X93).
//!
//! Two routes share the same path:
//!
//! * `GET  /v1/pubsub/<workspace>/<topic>`  — WebSocket. Optional `?since=<id>`
//!   replays every persisted envelope with id > cursor before the live tail.
//! * `POST /v1/pubsub/<workspace>/<topic>`  — publish a JSON body as the
//!   envelope payload. Returns `{ id, ts }`.
//!
//! Workspace isolation is enforced through the server authentication gate.

use std::sync::Arc;

use actant_core::WorkspaceId;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;

use crate::{enforce_auth, enforce_rate_limit, AppState};

/// Path arguments for the `/v1/pubsub/<workspace>/<topic>` route.
#[derive(Debug, Deserialize)]
pub struct PubsubPath {
    /// Workspace identifier.
    pub workspace: String,
    /// Topic name.
    pub topic: String,
}

/// Query arguments for the WebSocket variant.
#[derive(Debug, Deserialize)]
pub struct PubsubQuery {
    /// Optional cursor — backfill rows whose id is strictly greater.
    #[serde(default)]
    pub since: Option<String>,
}

/// `GET /v1/pubsub/<workspace>/<topic>` — WebSocket subscription.
pub async fn ws_pubsub(
    ws: WebSocketUpgrade,
    State(s): State<Arc<AppState>>,
    Path(path): Path<PubsubPath>,
    Query(q): Query<PubsubQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(resp) = enforce_auth(&s, &headers, &Method::GET, &path.workspace).await {
        return resp;
    }
    let workspace_id = WorkspaceId::from_string(path.workspace);
    let topic = path.topic;
    ws.on_upgrade(move |sock| run_broker_socket(s, sock, workspace_id, topic, q.since))
}

async fn run_broker_socket(
    state: Arc<AppState>,
    mut sock: WebSocket,
    workspace_id: WorkspaceId,
    topic: String,
    since: Option<String>,
) {
    let mut rx = match state.broker.subscribe(&workspace_id, &topic, since).await {
        Ok(rx) => rx,
        Err(e) => {
            let _ = sock
                .send(Message::Text(
                    serde_json::json!({"error":"subscribe_failed","message":e.to_string()})
                        .to_string(),
                ))
                .await;
            return;
        }
    };
    loop {
        match rx.recv().await {
            Ok(env) => {
                let text = match serde_json::to_string(&env) {
                    Ok(t) => t,
                    Err(_) => break,
                };
                if sock.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// `POST /v1/pubsub/<workspace>/<topic>` — publish an envelope.
pub async fn http_publish(
    State(s): State<Arc<AppState>>,
    Path(path): Path<PubsubPath>,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    if let Err(resp) = enforce_rate_limit(&s, &path.workspace).await {
        return resp;
    }
    if let Err(resp) = enforce_auth(&s, &headers, &Method::POST, &path.workspace).await {
        return resp;
    }
    let workspace_id = WorkspaceId::from_string(path.workspace);
    match s.broker.publish(&workspace_id, &path.topic, payload).await {
        Ok(env) => (
            StatusCode::OK,
            Json(serde_json::json!({ "id": env.id, "ts": env.ts })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"publish_failed","message":e.to_string()})),
        )
            .into_response(),
    }
}
