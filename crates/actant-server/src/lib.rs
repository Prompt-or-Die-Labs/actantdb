//! actant-server — HTTP + WebSocket server for ActantDB.
//!
//! Endpoints:
//!   GET  /v1/healthz                 200 OK
//!   GET  /v1/metadata/commands       list of registered command types
//!   POST /v1/command                 dispatch a command
//!   GET  /v1/events?session=...      list events in a session
//!   GET  /v1/approvals?ws=...        pending approvals for a workspace
//!   GET  /v1/ws                      WebSocket subscription
//!
//! See `/specs/08-api-spec.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::sync::Arc;

use actant_command::Engine;
use actant_core::{
    now_rfc3339, ActantError, Actor, ActorId, ActorKind, EventId, SessionId, WorkspaceId,
};
use actant_storage::Storage;
use actant_subscribe::{SubscribeHub, Topic};
use axum::{
    extract::{ws::WebSocket, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Command engine.
    pub engine: Engine,
    /// Storage handle (also accessible via `engine.storage()`).
    pub storage: Storage,
    /// Live-subscription hub.
    pub hub: SubscribeHub,
    /// Optional shared secret. When set, /v1/command requires `Authorization:
    /// Bearer <HS256-JWT>` signed with this secret.
    pub auth_secret: Option<Vec<u8>>,
    /// Per-workspace rate-limiter bucket. None = no rate limiting.
    pub rate_limiter: Option<
        std::sync::Arc<
            tokio::sync::Mutex<std::collections::HashMap<String, actant_throttle::Bucket>>,
        >,
    >,
    /// Token-bucket policy (applied when rate_limiter is set).
    pub rate_policy: Option<actant_throttle::Policy>,
}

impl AppState {
    /// Build a new app state from a storage handle. Wraps the storage in a
    /// fresh command engine + subscribe hub. Auth is off by default.
    pub fn new(storage: Storage) -> Self {
        let engine = Engine::new(storage.clone());
        Self {
            engine,
            storage,
            hub: SubscribeHub::new(),
            auth_secret: None,
            rate_limiter: None,
            rate_policy: None,
        }
    }

    /// Builder: enable HS256 bearer-token auth using the given shared secret.
    pub fn with_auth(mut self, secret: impl Into<Vec<u8>>) -> Self {
        self.auth_secret = Some(secret.into());
        self
    }

    /// Builder: enable per-workspace rate limiting via actant-throttle.
    pub fn with_rate_limit(mut self, policy: actant_throttle::Policy) -> Self {
        self.rate_limiter = Some(std::sync::Arc::new(tokio::sync::Mutex::new(
            std::collections::HashMap::new(),
        )));
        self.rate_policy = Some(policy);
        self
    }
}

/// Construct the axum router with every endpoint registered.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/healthz", get(healthz))
        .route("/v1/healthz/startup", get(healthz_startup))
        .route("/v1/healthz/live", get(healthz_live))
        .route("/v1/healthz/ready", get(healthz_ready))
        .route("/v1/metadata/commands", get(metadata_commands))
        .route("/v1/openapi.yaml", get(openapi_yaml))
        .route("/v1/command", post(dispatch_command))
        .route("/v1/events", get(list_events))
        .route("/v1/approvals", get(list_approvals))
        .route("/v1/ws", get(ws_handler))
        .route("/v1/metrics", get(metrics))
        .route("/v1/replay/checkpoint", post(replay_checkpoint))
        .route("/v1/replay/run", post(replay_run))
        .route("/v1/sync/since", post(sync_since))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(Arc::new(state))
}

/// Startup probe — always 200 once the server has bound.
async fn healthz_startup() -> impl IntoResponse {
    Json(serde_json::json!({"phase":"startup","ok":true}))
}

/// Liveness probe — process is alive (always 200 once bound).
async fn healthz_live() -> impl IntoResponse {
    Json(serde_json::json!({"phase":"live","ok":true}))
}

/// Readiness probe — actually touches the database. 503 if the DB is
/// unreachable.
async fn healthz_ready(State(s): State<Arc<AppState>>) -> Response {
    // SELECT 1 against the pool.
    let r: Result<(i64,), _> = sqlx::query_as("SELECT 1").fetch_one(s.storage.pool()).await;
    match r {
        Ok(_) => Json(serde_json::json!({"phase":"ready","ok":true})).into_response(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"phase":"ready","ok":false,"error":e.to_string()})),
        )
            .into_response(),
    }
}

/// Attach an `x-request-id` header to every response. Generated if the
/// client didn't supply one. Required for production tracing.
async fn request_id_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let rid = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("req_{}", ulid::Ulid::new()));
    tracing::info!(request_id = %rid, method = %req.method(), uri = %req.uri(), "request");
    let mut resp = next.run(req).await;
    if let Ok(val) = axum::http::HeaderValue::from_str(&rid) {
        resp.headers_mut().insert("x-request-id", val);
    }
    resp
}

/// Spawn a task that listens for SIGTERM/SIGINT and flips the shutdown
/// signal. Use with `axum::serve(...).with_graceful_shutdown(shutdown_signal())`.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}

/// Bind and serve a router. When both `tls_cert` and `tls_key` are provided,
/// the listener is wrapped in rustls and the protocol is HTTPS; otherwise
/// plain HTTP. In both cases the server listens for shutdown via
/// [`shutdown_signal`].
pub async fn serve(
    router: axum::Router,
    bind: &str,
    tls_cert: Option<std::path::PathBuf>,
    tls_key: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    let addr: std::net::SocketAddr = bind
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid bind address {bind:?}: {e}"))?;
    match (tls_cert, tls_key) {
        (Some(cert), Some(key)) => {
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
            eprintln!("actantdb listening on https://{bind}");
            let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert, &key)
                .await
                .map_err(|e| anyhow::anyhow!("load TLS config: {e}"))?;
            let handle = axum_server::Handle::new();
            let sh = handle.clone();
            tokio::spawn(async move {
                shutdown_signal().await;
                sh.graceful_shutdown(Some(std::time::Duration::from_secs(10)));
            });
            axum_server::bind_rustls(addr, config)
                .handle(handle)
                .serve(router.into_make_service())
                .await?;
        }
        _ => {
            eprintln!("actantdb listening on http://{bind}");
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown_signal())
                .await?;
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ReplayCheckpointRequest {
    workspace_id: String,
    event_id: String,
}

async fn replay_checkpoint(
    State(s): State<Arc<AppState>>,
    Json(req): Json<ReplayCheckpointRequest>,
) -> Response {
    let ws = WorkspaceId::from_string(req.workspace_id);
    let eid = actant_core::EventId::from_string(req.event_id);
    match actant_replay::checkpoint(&s.storage, &ws, &eid).await {
        Ok(cp_id) => Json(serde_json::json!({"checkpoint_id": cp_id.as_str()})).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Debug, Deserialize)]
struct ReplayRunRequest {
    actor_id: String,
    checkpoint_id: String,
    /// `recorded` | `model` | `policy` | `memory`.
    mode: String,
}

async fn replay_run(State(s): State<Arc<AppState>>, Json(req): Json<ReplayRunRequest>) -> Response {
    let actor = ActorId::from_string(req.actor_id);
    let cp = actant_core::ReplayCheckpointId::from_string(req.checkpoint_id);
    let mode = match req.mode.as_str() {
        "recorded" => actant_replay::ReplayMode::Recorded,
        "model" => actant_replay::ReplayMode::Model,
        "policy" => actant_replay::ReplayMode::Policy,
        "memory" => actant_replay::ReplayMode::Memory,
        other => {
            return err_response(ActantError::InvalidInput(format!(
                "unknown replay mode: {other}"
            )))
        }
    };
    match actant_replay::run(&s.storage, &actor, &cp, mode).await {
        Ok(diff) => Json(diff).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Debug, Deserialize)]
struct SyncSinceRequest {
    workspace_id: String,
    /// Only return events with id strictly greater than this. ULIDs are
    /// lexicographically sortable, so "since the last seen event id" is
    /// a string compare. Empty string means "from the beginning."
    #[serde(default)]
    since_event_id: String,
    /// Max events to return (1..=10_000).
    #[serde(default = "default_sync_limit")]
    limit: u32,
}

fn default_sync_limit() -> u32 {
    1000
}

/// Cluster-sync pull endpoint. A peer hands the server its last-seen event
/// id; the server returns events strictly after it, capped at `limit`.
/// Idempotent: re-pulling with the same `since_event_id` returns the same
/// set.
async fn sync_since(State(s): State<Arc<AppState>>, Json(req): Json<SyncSinceRequest>) -> Response {
    let limit = req.limit.clamp(1, 10_000) as i64;
    let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, String)>(
        "SELECT id, event_type, actor_id, payload_hash, payload_inline, created_at
         FROM agent_event
         WHERE workspace_id = ? AND id > ?
         ORDER BY id ASC LIMIT ?",
    )
    .bind(&req.workspace_id)
    .bind(&req.since_event_id)
    .bind(limit)
    .fetch_all(s.storage.pool())
    .await;
    match rows {
        Ok(rows) => {
            let events: Vec<_> = rows
                .into_iter()
                .map(|(id, et, actor, ph, pi, created)| {
                    serde_json::json!({
                        "id": id,
                        "event_type": et,
                        "actor_id": actor,
                        "payload_hash": ph,
                        "payload_inline": pi,
                        "created_at": created,
                    })
                })
                .collect();
            let next = events
                .last()
                .and_then(|e| e["id"].as_str())
                .map(String::from);
            Json(serde_json::json!({
                "events": events,
                "next_since": next,
            }))
            .into_response()
        }
        Err(e) => err_response(ActantError::Storage(e.to_string())),
    }
}

async fn openapi_yaml() -> Response {
    let body = include_str!("../openapi.yaml");
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/yaml")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap()
}

/// Prometheus text-format metrics.
async fn metrics(State(s): State<Arc<AppState>>) -> Response {
    let mut out = String::with_capacity(2048);
    // Events by kind.
    let by_kind: Vec<(String, i64)> =
        sqlx::query_as("SELECT event_type, COUNT(*) as n FROM agent_event GROUP BY event_type")
            .fetch_all(s.storage.pool())
            .await
            .unwrap_or_default();
    out.push_str("# HELP actantdb_events_total Number of Chronicle events by kind.\n");
    out.push_str("# TYPE actantdb_events_total counter\n");
    for (kind, n) in by_kind {
        out.push_str(&format!(
            "actantdb_events_total{{event_type=\"{}\"}} {}\n",
            escape_label(&kind),
            n
        ));
    }

    // Effects by status.
    let by_status: Vec<(String, i64)> =
        sqlx::query_as("SELECT status, COUNT(*) as n FROM effect GROUP BY status")
            .fetch_all(s.storage.pool())
            .await
            .unwrap_or_default();
    out.push_str("# HELP actantdb_effects_total Number of effects by status.\n");
    out.push_str("# TYPE actantdb_effects_total gauge\n");
    for (status, n) in by_status {
        out.push_str(&format!(
            "actantdb_effects_total{{status=\"{}\"}} {}\n",
            escape_label(&status),
            n
        ));
    }

    // Pending approvals.
    let pending: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM approval_request WHERE status='pending'")
            .fetch_one(s.storage.pool())
            .await
            .unwrap_or((0,));
    out.push_str("# HELP actantdb_approvals_pending Number of pending approval requests.\n");
    out.push_str("# TYPE actantdb_approvals_pending gauge\n");
    out.push_str(&format!("actantdb_approvals_pending {}\n", pending.0));

    // Workspaces.
    let workspaces: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM workspace")
        .fetch_one(s.storage.pool())
        .await
        .unwrap_or((0,));
    out.push_str("# HELP actantdb_workspaces_total Number of workspaces.\n");
    out.push_str("# TYPE actantdb_workspaces_total gauge\n");
    out.push_str(&format!("actantdb_workspaces_total {}\n", workspaces.0));

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain; version=0.0.4")
        .body(axum::body::Body::from(out))
        .unwrap()
}

fn escape_label(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

async fn healthz() -> impl IntoResponse {
    Json(serde_json::json!({"status":"ok","time": now_rfc3339()}))
}

async fn metadata_commands() -> impl IntoResponse {
    Json(serde_json::json!({
        "commands": [
            "create_session",
            "append_user_message",
            "append_agent_message",
            "request_tool_call",
            "approve_tool_call",
            "deny_tool_call",
            "record_tool_result",
            "propose_memory",
            "approve_memory",
            "reject_memory",
        ]
    }))
}

#[derive(Debug, Deserialize)]
struct CommandRequest {
    workspace_id: String,
    actor_id: String,
    command_type: String,
    input: serde_json::Value,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[derive(Debug, Serialize)]
struct CommandResponse {
    command_id: String,
    event_id: Option<EventId>,
    result: serde_json::Value,
}

async fn dispatch_command(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CommandRequest>,
) -> Response {
    // Rate limiting: consult per-workspace token bucket.
    if let (Some(limiter), Some(policy)) = (&s.rate_limiter, &s.rate_policy) {
        let mut g = limiter.lock().await;
        let bucket = g
            .entry(req.workspace_id.clone())
            .or_insert_with(|| actant_throttle::Bucket::new(policy.clone()));
        if let Err(retry_after) = bucket.try_consume(1) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", retry_after.as_secs().max(1).to_string())],
                Json(serde_json::json!({
                    "error":"rate_limited",
                    "retry_after_seconds": retry_after.as_secs()
                })),
            )
                .into_response();
        }
    }
    // Auth: when a secret is configured, require a valid HS256 JWT.
    if let Some(secret) = &s.auth_secret {
        let token = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        let Some(token) = token else {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error":"missing_authorization"})),
            )
                .into_response();
        };
        match actant_auth::verify(token, secret) {
            Ok(claims) => {
                // Optional: pin the principal to the request actor/workspace.
                if claims.iss != req.workspace_id {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(serde_json::json!({
                            "error": "workspace_mismatch",
                            "iss": claims.iss,
                            "workspace_id": req.workspace_id
                        })),
                    )
                        .into_response();
                }
            }
            Err(_) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error":"invalid_token"})),
                )
                    .into_response();
            }
        }
    }
    let ws = WorkspaceId::from_string(req.workspace_id);
    let actor = ActorId::from_string(req.actor_id);
    let outcome = s
        .engine
        .dispatch(
            &ws,
            &actor,
            &req.command_type,
            req.input,
            req.idempotency_key.as_deref(),
        )
        .await;
    match outcome {
        Ok(out) => {
            // Best-effort: publish to the events topic.
            let topic = Topic {
                workspace_id: ws.clone(),
                session_id: None,
                kind: "events".into(),
            };
            s.hub
                .publish(
                    topic,
                    serde_json::json!({
                        "command_id": out.command_id.as_str(),
                        "event_id": out.event_id.as_ref().map(|e| e.as_str()),
                        "result": out.result,
                    }),
                )
                .await;
            (
                StatusCode::OK,
                Json(CommandResponse {
                    command_id: out.command_id.as_str().into(),
                    event_id: out.event_id,
                    result: out.result,
                }),
            )
                .into_response()
        }
        Err(e) => err_response(e),
    }
}

#[derive(Debug, Deserialize)]
struct EventsQuery {
    session_id: String,
}

async fn list_events(State(s): State<Arc<AppState>>, Query(q): Query<EventsQuery>) -> Response {
    let session_id = SessionId::from_string(q.session_id);
    match s.storage.events_in_session(&session_id).await {
        Ok(events) => Json(serde_json::json!({"events": events})).into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Debug, Deserialize)]
struct ApprovalsQuery {
    workspace_id: String,
}

async fn list_approvals(
    State(s): State<Arc<AppState>>,
    Query(q): Query<ApprovalsQuery>,
) -> Response {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT id, tool_call_id, requested_by_actor_id, risk_level, summary, status
         FROM approval_request
         WHERE workspace_id = ? AND status = 'pending'
         ORDER BY created_at ASC",
    )
    .bind(q.workspace_id)
    .fetch_all(s.storage.pool())
    .await;
    match rows {
        Ok(rows) => {
            let approvals: Vec<_> = rows
                .into_iter()
                .map(
                    |(id, tool_call_id, requested_by, risk_level, summary, status)| {
                        serde_json::json!({
                            "id": id,
                            "tool_call_id": tool_call_id,
                            "requested_by": requested_by,
                            "risk_level": risk_level,
                            "summary": summary,
                            "status": status,
                        })
                    },
                )
                .collect();
            Json(serde_json::json!({"approvals": approvals})).into_response()
        }
        Err(e) => err_response(ActantError::Storage(e.to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct WsQuery {
    workspace_id: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default = "default_kind")]
    kind: String,
}
fn default_kind() -> String {
    "events".into()
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(s): State<Arc<AppState>>,
    Query(q): Query<WsQuery>,
) -> Response {
    let topic = Topic {
        workspace_id: WorkspaceId::from_string(q.workspace_id),
        session_id: q.session_id.map(SessionId::from_string),
        kind: q.kind,
    };
    ws.on_upgrade(move |sock| run_subscription(s, sock, topic))
}

async fn run_subscription(state: Arc<AppState>, mut sock: WebSocket, topic: Topic) {
    let mut rx = state.hub.subscribe(topic).await;
    while let Ok(msg) = rx.recv().await {
        let text = match serde_json::to_string(&msg) {
            Ok(t) => t,
            Err(_) => break,
        };
        if sock
            .send(axum::extract::ws::Message::Text(text))
            .await
            .is_err()
        {
            break;
        }
    }
}

fn err_response(e: ActantError) -> Response {
    let (status, kind) = match &e {
        ActantError::InvalidInput(_) => (StatusCode::BAD_REQUEST, "invalid_input"),
        ActantError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
        ActantError::PermissionDenied(_) => (StatusCode::FORBIDDEN, "permission_denied"),
        ActantError::ApprovalRequired(_) => (StatusCode::ACCEPTED, "approval_required"),
        ActantError::ApprovalDenied(_) => (StatusCode::FORBIDDEN, "approval_denied"),
        ActantError::IdempotentReplay(_) => (StatusCode::OK, "idempotent_replay"),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    };
    (
        status,
        Json(serde_json::json!({"error": kind, "message": e.to_string()})),
    )
        .into_response()
}

/// Bootstrap helper: open storage, build state, return router + pool. Used
/// by the binary and by tests.
pub async fn bootstrap(
    db_path: Option<std::path::PathBuf>,
) -> Result<(Router, AppState), ActantError> {
    let cfg = match db_path {
        Some(p) => actant_storage::StorageConfig::file(p),
        None => actant_storage::StorageConfig::in_memory(),
    };
    let storage = Storage::open(cfg).await?;
    let state = AppState::new(storage);
    // Seed a default workspace + system actor when the DB is empty so the
    // first user can hit the server without a separate setup step.
    seed_if_empty(&state).await?;
    let router = router(state.clone());
    Ok((router, state))
}

async fn seed_if_empty(state: &AppState) -> Result<(), ActantError> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM workspace")
        .fetch_one(state.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
    if count.0 > 0 {
        return Ok(());
    }
    let ws = actant_core::Workspace {
        id: WorkspaceId::from_string("ws_default".to_string()),
        name: "default".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    state.storage.insert_workspace(&ws).await?;
    let actor = Actor {
        id: ActorId::from_string("act_system".to_string()),
        workspace_id: ws.id.clone(),
        kind: ActorKind::System,
        display_name: "system".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    state.storage.insert_actor(&actor).await?;
    Ok(())
}
