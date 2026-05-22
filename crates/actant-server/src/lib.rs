//! actant-server — HTTP + WebSocket server for ActantDB.
//!
//! Endpoints:
//!   GET    /v1/healthz                 200 OK
//!   GET    /v1/metadata/commands       list of registered command types
//!   POST   /v1/command                 dispatch a command
//!   GET    /v1/events?session=...      list events in a session
//!   GET    /v1/approvals?ws=...        pending approvals for a workspace
//!   GET    /v1/ws                      WebSocket subscription
//!   GET    /v1/memories                list approved | pending | rejected | all
//!   GET    /v1/memories/conflicts      pairs detected by MemoryStore
//!   GET    /v1/permissions             list active authority_scope rows
//!   POST   /v1/permissions             grant an authority_scope
//!   DELETE /v1/permissions             soft-revoke an authority_scope
//!   POST   /v1/setup-reports           append a setup_report agent_event + artifact
//!   GET    /v1/setup-reports           latest or recent setup-report artifacts
//!   POST   /v1/scout-records           append a scout_record agent_event + artifact
//!   GET    /v1/scout-records           recent scout-record artifacts (optional source)
//!
//! See `/specs/08-api-spec.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod auth_routes;
pub mod prom;
pub mod pubsub_routes;

#[cfg(feature = "auto-rest")]
pub mod auto_rest;
#[cfg(feature = "graphql")]
pub mod graphql_api;
#[cfg(any(feature = "auto-rest", feature = "graphql"))]
pub mod schema_introspect;

use std::{collections::HashMap, sync::Arc};

use actant_command::Engine;
use actant_core::{
    canonical_json, chain_hash, now_rfc3339, sha256_hex, ActantError, Actor, ActorId, ActorKind,
    AgentEvent, CausalityKind, EventId, Sensitivity, SessionId, Workspace, WorkspaceId,
};
use actant_storage::{PgStorage, Storage};
use actant_subscribe::{Broker, SubscribeHub, Topic};
use axum::{
    extract::{ws::WebSocket, Path, Query, State, WebSocketUpgrade},
    http::{header, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
type HmacSha256 = Hmac<sha2::Sha256>;

type WorkspaceRateLimiter =
    Arc<tokio::sync::Mutex<HashMap<String, actant_reliability::throttle::Bucket>>>;

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
    pub rate_limiter: Option<WorkspaceRateLimiter>,
    /// Token-bucket policy (applied when rate_limiter is set).
    pub rate_policy: Option<actant_reliability::throttle::Policy>,
    /// Per-IP / per-session limiters for the UI auth surface (link redeem,
    /// password login). Separate from the per-workspace command bucket
    /// because the failure modes don't overlap.
    pub auth_limiters: Arc<auth_routes::AuthRateLimiters>,
    /// Loopback bind detected at boot (or set explicitly via the env
    /// override). When `true`, the new auth routes accept unauthenticated
    /// access AND the session cookie is minted without `Secure`.
    pub local_mode: bool,
    /// Whether the server is configured to terminate TLS. Drives the
    /// `Secure` flag on session cookies and the boot-time refusal in the
    /// binary when bound non-loopback without TLS.
    pub tls_enabled: bool,
    /// When `true`, the server will honor reverse-proxy headers
    /// (`X-Forwarded-For`, `Forwarded`) in `local_mode`. By default a
    /// forwarded request arriving at a loopback bind fails closed
    /// (`reverse_proxy_detected`) so a misconfigured reverse proxy can't
    /// trivially bypass the "loopback = trusted" assumption.
    pub trust_proxy: bool,
    /// Persistent named-topic pub/sub broker (DEVX_GAPS X93). Backs the
    /// `/v1/pubsub/<workspace>/<topic>` WebSocket route.
    pub broker: Broker,
    /// Schema introspection cache used by the optional `auto-rest` and
    /// `graphql` surfaces. `None` when those features aren't enabled or
    /// when the cache hasn't been initialized yet.
    #[cfg(any(feature = "auto-rest", feature = "graphql"))]
    pub schema_cache: Option<Arc<schema_introspect::SchemaCache>>,
}

impl AppState {
    /// Build a new app state from a storage handle. Wraps the storage in a
    /// fresh command engine + subscribe hub. Auth is off by default.
    pub fn new(storage: Storage) -> Self {
        let engine = Engine::new(storage.clone());
        let broker = Broker::new(storage.clone());
        Self {
            engine,
            storage,
            hub: SubscribeHub::new(),
            auth_secret: None,
            rate_limiter: None,
            rate_policy: None,
            auth_limiters: Arc::new(auth_routes::AuthRateLimiters::new()),
            // Loopback-trusted by default — the binary flips this off when
            // the bind address is non-loopback.
            local_mode: true,
            tls_enabled: false,
            trust_proxy: false,
            broker,
            #[cfg(any(feature = "auto-rest", feature = "graphql"))]
            schema_cache: None,
        }
    }

    /// Builder: install a schema cache so the auto-rest / graphql routes
    /// can introspect the database. Use [`schema_introspect::SchemaCache::introspect`]
    /// at boot to build it.
    #[cfg(any(feature = "auto-rest", feature = "graphql"))]
    pub fn with_schema_cache(mut self, cache: schema_introspect::SchemaCache) -> Self {
        self.schema_cache = Some(Arc::new(cache));
        self
    }

    /// Builder: explicitly mark this state as remote (`/link` flow + cookies
    /// required for the UI surface).
    pub fn with_local_mode(mut self, local: bool) -> Self {
        self.local_mode = local;
        self
    }

    /// Builder: announce that the server is serving HTTPS (so the session
    /// cookie gets the `Secure` flag).
    pub fn with_tls_enabled(mut self, enabled: bool) -> Self {
        self.tls_enabled = enabled;
        self
    }

    /// Builder: trust `X-Forwarded-For` / `Forwarded` headers even in
    /// `local_mode`. Set this only when the loopback bind sits behind a
    /// trusted reverse proxy you control.
    pub fn with_trust_proxy(mut self, trust: bool) -> Self {
        self.trust_proxy = trust;
        self
    }

    /// Builder: enable HS256 bearer-token auth using the given shared secret.
    pub fn with_auth(mut self, secret: impl Into<Vec<u8>>) -> Self {
        self.auth_secret = Some(secret.into());
        self
    }

    /// Builder: enable per-workspace rate limiting via actant-reliability::throttle.
    pub fn with_rate_limit(mut self, policy: actant_reliability::throttle::Policy) -> Self {
        self.rate_limiter = Some(std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())));
        self.rate_policy = Some(policy);
        self
    }
}

/// Shared state for the Postgres-backed core HTTP surface.
#[derive(Clone)]
pub struct PostgresAppState {
    /// Command engine.
    pub engine: Engine,
    /// Postgres storage handle.
    pub storage: PgStorage,
    /// Optional HS256 bearer-token secret.
    pub auth_secret: Option<Vec<u8>>,
    /// Per-workspace rate-limiter bucket. None = no rate limiting.
    pub rate_limiter: Option<WorkspaceRateLimiter>,
    /// Token-bucket policy.
    pub rate_policy: Option<actant_reliability::throttle::Policy>,
    /// Loopback bind mode.
    pub local_mode: bool,
    /// Whether forwarded proxy headers are trusted in local mode.
    pub trust_proxy: bool,
}

impl PostgresAppState {
    /// Build a Postgres state from a storage handle.
    pub fn new(storage: PgStorage) -> Self {
        let engine = Engine::postgres(storage.clone());
        Self {
            engine,
            storage,
            auth_secret: None,
            rate_limiter: None,
            rate_policy: None,
            local_mode: true,
            trust_proxy: false,
        }
    }

    /// Builder: explicitly mark this state as remote.
    pub fn with_local_mode(mut self, local: bool) -> Self {
        self.local_mode = local;
        self
    }

    /// Builder: trust `X-Forwarded-For` / `Forwarded` headers in local mode.
    pub fn with_trust_proxy(mut self, trust: bool) -> Self {
        self.trust_proxy = trust;
        self
    }

    /// Builder: enable HS256 bearer-token auth using the given shared secret.
    pub fn with_auth(mut self, secret: impl Into<Vec<u8>>) -> Self {
        self.auth_secret = Some(secret.into());
        self
    }

    /// Builder: enable per-workspace rate limiting.
    pub fn with_rate_limit(mut self, policy: actant_reliability::throttle::Policy) -> Self {
        self.rate_limiter = Some(Arc::new(tokio::sync::Mutex::new(HashMap::new())));
        self.rate_policy = Some(policy);
        self
    }
}

/// Construct the axum router with every endpoint registered.
pub fn router(state: AppState) -> Router {
    let r: Router<Arc<AppState>> = Router::new()
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
        .route("/v1/memories", get(list_memories))
        .route("/v1/memories/conflicts", get(list_memory_conflicts))
        .route(
            "/v1/permissions",
            get(list_permissions)
                .post(create_permission)
                .delete(revoke_permission),
        )
        .route(
            "/v1/setup-reports",
            get(list_setup_reports).post(create_setup_report),
        )
        .route(
            "/v1/scout-records",
            get(list_scout_records).post(create_scout_record),
        )
        .route("/v1/entities", get(list_entities).post(create_entity))
        .route(
            "/v1/entity-relations",
            get(list_entity_relations).post(create_entity_relation),
        )
        // --- UI auth (per UI_AUTH_DESIGN.md §5) ---------------------------
        .route("/link", get(auth_routes::link_page))
        .route("/link/{code}", get(auth_routes::link_page))
        .route("/login", get(auth_routes::login_page))
        .route("/v1/auth/link", post(auth_routes::link_redeem))
        .route("/v1/auth/password", post(auth_routes::set_password))
        .route("/v1/auth/login", post(auth_routes::login))
        .route("/v1/auth/logout", post(auth_routes::logout))
        .route("/v1/auth/me", get(auth_routes::whoami))
        .route("/v1/auth/providers", get(auth_providers))
        .route("/v1/auth/oauth/{provider}/start", get(auth_oauth_start))
        .route(
            "/v1/auth/oauth/{provider}/callback",
            get(auth_oauth_callback),
        )
        // Prometheus exposition (in addition to the older /v1/metrics view).
        .route("/metrics", get(prom_metrics))
        // Pub/sub broker WS surface (DEVX_GAPS X93).
        .route(
            "/v1/pubsub/{workspace}/{topic}",
            get(pubsub_routes::ws_pubsub).post(pubsub_routes::http_publish),
        );

    #[cfg(feature = "auto-rest")]
    let r = auto_rest::mount(r);
    #[cfg(feature = "graphql")]
    let r = graphql_api::mount(r);

    r.layer(axum::middleware::from_fn(prom::record_http_middleware))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(Arc::new(state))
}

/// Construct the Postgres-backed core HTTP router.
///
/// This surface intentionally registers only routes whose implementation is
/// backend-neutral or backed by `PgStorage`: health, metadata, typed command
/// dispatch, event reads, and sync pulls. SQLite-specific Studio/admin routes
/// fail closed with 501 instead of silently downgrading to SQLite.
pub fn postgres_router(state: PostgresAppState) -> Router {
    Router::new()
        .route("/v1/healthz", get(healthz))
        .route("/v1/healthz/startup", get(healthz_startup))
        .route("/v1/healthz/live", get(healthz_live))
        .route("/v1/healthz/ready", get(pg_healthz_ready))
        .route("/v1/metadata/commands", get(metadata_commands))
        .route("/v1/openapi.yaml", get(openapi_yaml))
        .route("/v1/command", post(pg_dispatch_command))
        .route("/v1/events", get(pg_list_events))
        .route("/v1/sync/since", post(pg_sync_since))
        .route("/v1/auth/providers", get(auth_providers))
        .route("/v1/auth/oauth/{provider}/start", get(auth_oauth_start))
        .route(
            "/v1/auth/oauth/{provider}/callback",
            get(auth_oauth_callback),
        )
        .fallback(pg_not_implemented)
        .layer(axum::middleware::from_fn(prom::record_http_middleware))
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

async fn pg_healthz_ready(State(s): State<Arc<PostgresAppState>>) -> Response {
    match sqlx::query("SELECT 1").execute(s.storage.pool()).await {
        Ok(_) => Json(serde_json::json!({
            "phase": "ready",
            "ok": true,
            "backend": "postgres"
        }))
        .into_response(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "phase": "ready",
                "ok": false,
                "backend": "postgres",
                "error": e.to_string()
            })),
        )
            .into_response(),
    }
}

async fn pg_dispatch_command(
    State(s): State<Arc<PostgresAppState>>,
    headers: HeaderMap,
    Json(req): Json<CommandRequest>,
) -> Response {
    if let Err(resp) = enforce_pg_auth(&s, &headers, &Method::POST, &req.workspace_id).await {
        return resp;
    }
    if let Err(resp) = enforce_pg_rate_limit(&s, &req.workspace_id).await {
        return resp;
    }
    prom::record_command(&req.workspace_id, &req.command_type);
    let ws = WorkspaceId::from_string(req.workspace_id);
    let actor = ActorId::from_string(req.actor_id);
    match s
        .engine
        .dispatch(
            &ws,
            &actor,
            &req.command_type,
            req.input,
            req.idempotency_key.as_deref(),
        )
        .await
    {
        Ok(out) => (
            StatusCode::OK,
            Json(CommandResponse {
                command_id: out.command_id.as_str().into(),
                event_id: out.event_id,
                result: out.result,
            }),
        )
            .into_response(),
        Err(e) => err_response(e),
    }
}

async fn pg_list_events(
    State(s): State<Arc<PostgresAppState>>,
    headers: HeaderMap,
    Query(q): Query<EventsQuery>,
) -> Response {
    let session_id = SessionId::from_string(q.session_id);
    let workspace_id =
        match sqlx::query_scalar::<_, String>("SELECT workspace_id FROM session WHERE id = $1")
            .bind(session_id.as_str())
            .fetch_optional(s.storage.pool())
            .await
        {
            Ok(Some(workspace_id)) => workspace_id,
            Ok(None) => return pg_events_auth_error(),
            Err(e) => return err_response(ActantError::Storage(e.to_string())),
        };
    if enforce_pg_auth(&s, &headers, &Method::GET, &workspace_id)
        .await
        .is_err()
    {
        return pg_events_auth_error();
    }
    match s.storage.events_in_session(&session_id).await {
        Ok(events) => Json(serde_json::json!({"events": events})).into_response(),
        Err(e) => err_response(e),
    }
}

async fn pg_sync_since(
    State(s): State<Arc<PostgresAppState>>,
    headers: HeaderMap,
    Json(req): Json<SyncSinceRequest>,
) -> Response {
    if let Err(resp) = enforce_pg_auth(&s, &headers, &Method::POST, &req.workspace_id).await {
        return resp;
    }
    if let Err(resp) = enforce_pg_rate_limit(&s, &req.workspace_id).await {
        return resp;
    }
    let limit = req.limit.clamp(1, 10_000) as i64;
    let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, String)>(
        "SELECT id, event_type, actor_id, payload_hash, payload_inline, created_at
         FROM agent_event
         WHERE workspace_id = $1 AND id > $2
         ORDER BY id ASC LIMIT $3",
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

async fn pg_not_implemented(uri: axum::http::Uri) -> Response {
    err_response(ActantError::NotImplemented(format!(
        "route {} is SQLite-only; Postgres server mode exposes health, metadata, command, events, and sync routes",
        uri.path()
    )))
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
                .serve(router.into_make_service_with_connect_info::<std::net::SocketAddr>())
                .await?;
        }
        _ => {
            eprintln!("actantdb listening on http://{bind}");
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
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

/// Prometheus exposition for the in-process counter registry. Lives at
/// `/metrics` (the conventional path) alongside the older snapshot view
/// at `/v1/metrics`. See `prom.rs` for the registered collectors.
async fn prom_metrics(State(s): State<Arc<AppState>>) -> Response {
    // Best-effort ledger size sample on every scrape. SQLite reports its
    // own file footprint via `page_count * page_size`; the in-memory
    // backend simply returns 0. A future revision can attribute size
    // per workspace once the per-tenant storage layout lands; today
    // every workspace shares one database file, so the size is reported
    // against the `_global` label.
    let bytes: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT (SELECT page_count FROM pragma_page_count()) \
              * (SELECT page_size  FROM pragma_page_size())",
    )
    .fetch_one(s.storage.pool())
    .await
    .unwrap_or(0);
    prom::record_ledger_bytes("_global", bytes.max(0) as u64);
    prom::render()
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

#[derive(Debug, Clone, Serialize)]
struct AuthProviderInfo {
    id: String,
    label: String,
}

#[derive(Debug, Clone)]
struct AuthProviderConfig {
    id: String,
    label: String,
    auth_url: String,
    client_id: String,
    scope: String,
}

#[derive(Debug, Deserialize)]
struct OAuthCallbackQuery {
    state: Option<String>,
}

const OAUTH_STATE_TTL_SECS: i64 = 10 * 60;

fn unix_now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn oauth_enabled() -> bool {
    let Ok(value) = std::env::var("ACTANTDB_OAUTH_ENABLED") else {
        return false;
    };
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn oauth_state_secret() -> Result<Vec<u8>, Response> {
    match std::env::var("ACTANTDB_OAUTH_STATE_SECRET") {
        Ok(value) if !value.trim().is_empty() => Ok(value.into_bytes()),
        _ => Err((
            StatusCode::NOT_IMPLEMENTED,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            "OAuth sign-in is not enabled: set ACTANTDB_OAUTH_ENABLED=true and ACTANTDB_OAUTH_STATE_SECRET.",
        )
            .into_response()),
    }
}

fn oauth_state_signature(secret: &[u8], provider: &str, expires_at: i64, nonce: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(format!("{provider}.{expires_at}.{nonce}").as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn new_oauth_state(provider: &str) -> Result<String, Response> {
    let secret = oauth_state_secret()?;
    let expires_at = unix_now() + OAUTH_STATE_TTL_SECS;
    let nonce = ulid::Ulid::new().to_string();
    let signature = oauth_state_signature(&secret, provider, expires_at, &nonce);
    Ok(format!("oauth.{provider}.{expires_at}.{nonce}.{signature}"))
}

fn verify_oauth_state(state: &str, provider: &str) -> Result<(), Response> {
    let parts: Vec<_> = state.split('.').collect();
    if parts.len() != 5 || parts[0] != "oauth" {
        return Err(err_response(ActantError::PermissionDenied(
            "invalid OAuth state".into(),
        )));
    }
    let state_provider = parts[1];
    let expires_at = parts[2]
        .parse::<i64>()
        .map_err(|_| err_response(ActantError::PermissionDenied("invalid OAuth state".into())))?;
    let nonce = parts[3];
    let signature = parts[4];
    if state_provider != provider {
        return Err(err_response(ActantError::PermissionDenied(
            "OAuth state provider mismatch".into(),
        )));
    }
    if expires_at <= unix_now() {
        return Err(err_response(ActantError::PermissionDenied(
            "expired OAuth state".into(),
        )));
    }
    let secret = oauth_state_secret()?;
    let sig = hex::decode(signature)
        .map_err(|_| err_response(ActantError::PermissionDenied("invalid OAuth state".into())))?;
    let mut mac = HmacSha256::new_from_slice(&secret).expect("HMAC accepts any key length");
    mac.update(format!("{state_provider}.{expires_at}.{nonce}").as_bytes());
    mac.verify_slice(&sig)
        .map_err(|_| err_response(ActantError::PermissionDenied("invalid OAuth state".into())))
}

async fn auth_providers() -> Response {
    if !oauth_enabled() {
        return Json(serde_json::json!({ "providers": [] })).into_response();
    }
    let providers: Vec<_> = configured_auth_providers()
        .into_iter()
        .map(|p| AuthProviderInfo {
            id: p.id,
            label: p.label,
        })
        .collect();
    Json(serde_json::json!({ "providers": providers })).into_response()
}

async fn auth_oauth_start(Path(provider): Path<String>) -> Response {
    if !oauth_enabled() {
        return (
            StatusCode::NOT_IMPLEMENTED,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            "OAuth sign-in is not enabled for this server.",
        )
            .into_response();
    }
    let Some(config) = configured_auth_providers()
        .into_iter()
        .find(|p| p.id == provider)
    else {
        return err_response(ActantError::NotFound(format!(
            "unknown auth provider: {provider}"
        )));
    };
    let Ok(base_url) = std::env::var("ACTANTDB_PUBLIC_BASE_URL") else {
        return err_response(ActantError::InvalidInput(
            "ACTANTDB_PUBLIC_BASE_URL is required for OAuth redirects".into(),
        ));
    };
    let redirect_uri = format!(
        "{}/v1/auth/oauth/{}/callback",
        base_url.trim_end_matches('/'),
        config.id
    );
    let state = match new_oauth_state(&config.id) {
        Ok(state) => state,
        Err(response) => return response,
    };
    let location = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}",
        config.auth_url,
        percent_encode(&config.client_id),
        percent_encode(&redirect_uri),
        percent_encode(&config.scope),
        percent_encode(&state)
    );
    match axum::http::HeaderValue::from_str(&location) {
        Ok(value) => {
            let mut response = StatusCode::FOUND.into_response();
            response.headers_mut().insert(header::LOCATION, value);
            response
        }
        Err(e) => err_response(ActantError::InvalidInput(format!(
            "invalid OAuth redirect URL: {e}"
        ))),
    }
}

async fn auth_oauth_callback(
    Path(provider): Path<String>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Response {
    let Some(state) = query.state else {
        return err_response(ActantError::PermissionDenied("missing OAuth state".into()));
    };
    if let Err(response) = verify_oauth_state(&state, &provider) {
        return response;
    }
    err_response(ActantError::NotImplemented(format!(
        "OAuth callback for provider {provider} requires a token-exchange service with provider client secret storage"
    )))
}

fn configured_auth_providers() -> Vec<AuthProviderConfig> {
    let Ok(raw) = std::env::var("ACTANTDB_OAUTH_PROVIDERS") else {
        return Vec::new();
    };
    raw.split(',')
        .filter_map(|provider| {
            let id = provider.trim().to_lowercase();
            if id.is_empty() {
                return None;
            }
            let key = id
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_uppercase()
                    } else {
                        '_'
                    }
                })
                .collect::<String>();
            let auth_url = std::env::var(format!("ACTANTDB_OAUTH_{key}_AUTH_URL")).ok()?;
            let client_id = std::env::var(format!("ACTANTDB_OAUTH_{key}_CLIENT_ID")).ok()?;
            let label = std::env::var(format!("ACTANTDB_OAUTH_{key}_LABEL"))
                .unwrap_or_else(|_| title_case_provider(&id));
            let scope = std::env::var(format!("ACTANTDB_OAUTH_{key}_SCOPE"))
                .unwrap_or_else(|_| "openid profile email".into());
            Some(AuthProviderConfig {
                id,
                label,
                auth_url,
                client_id,
                scope,
            })
        })
        .collect()
}

fn title_case_provider(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = String::new();
                    out.extend(first.to_uppercase());
                    out.push_str(chars.as_str());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn percent_encode(raw: &str) -> String {
    raw.bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => {
                let encoded = format!("%{byte:02X}");
                encoded.chars().collect()
            }
        })
        .collect()
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

/// Apply per-workspace token-bucket rate-limiting. Returns `Err(response)`
/// with a 429 + retry-after when the bucket is exhausted; `Ok(())` when
/// rate-limiting is disabled or the request is permitted.
#[allow(clippy::result_large_err)]
async fn enforce_rate_limit(state: &AppState, workspace_id: &str) -> Result<(), Response> {
    if let (Some(limiter), Some(policy)) = (&state.rate_limiter, &state.rate_policy) {
        let mut g = limiter.lock().await;
        let bucket = g
            .entry(workspace_id.to_string())
            .or_insert_with(|| actant_reliability::throttle::Bucket::new(policy.clone()));
        if let Err(retry_after) = bucket.try_consume(1) {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", retry_after.as_secs().max(1).to_string())],
                Json(serde_json::json!({
                    "error": "rate_limited",
                    "retry_after_seconds": retry_after.as_secs()
                })),
            )
                .into_response());
        }
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
async fn enforce_pg_rate_limit(
    state: &PostgresAppState,
    workspace_id: &str,
) -> Result<(), Response> {
    if let (Some(limiter), Some(policy)) = (&state.rate_limiter, &state.rate_policy) {
        let mut g = limiter.lock().await;
        let bucket = g
            .entry(workspace_id.to_string())
            .or_insert_with(|| actant_reliability::throttle::Bucket::new(policy.clone()));
        if let Err(retry_after) = bucket.try_consume(1) {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", retry_after.as_secs().max(1).to_string())],
                Json(serde_json::json!({
                    "error": "rate_limited",
                    "retry_after_seconds": retry_after.as_secs()
                })),
            )
                .into_response());
        }
    }
    Ok(())
}

#[derive(Debug)]
struct SessionAuthRow {
    workspace_id: String,
    csrf_secret: String,
    expires_at: String,
    revoked_at: Option<String>,
}

impl From<(String, String, String, Option<String>)> for SessionAuthRow {
    fn from(row: (String, String, String, Option<String>)) -> Self {
        Self {
            workspace_id: row.0,
            csrf_secret: row.1,
            expires_at: row.2,
            revoked_at: row.3,
        }
    }
}

fn auth_error(status: StatusCode, body: serde_json::Value) -> Response {
    (status, Json(body)).into_response()
}

fn pg_events_auth_error() -> Response {
    auth_error(
        StatusCode::UNAUTHORIZED,
        serde_json::json!({"error": "session_not_authorized"}),
    )
}

#[allow(clippy::result_large_err)]
fn enforce_local_auth_bypass(
    local_mode: bool,
    trust_proxy: bool,
    auth_secret: Option<&[u8]>,
    headers: &HeaderMap,
) -> Result<bool, Response> {
    if !local_mode || auth_secret.is_some() {
        return Ok(false);
    }
    let proxied = headers.contains_key("x-forwarded-for")
        || headers.contains_key("forwarded")
        || headers.contains_key("x-real-ip");
    if proxied && !trust_proxy {
        return Err(auth_error(
            StatusCode::FORBIDDEN,
            serde_json::json!({
                "error": "reverse_proxy_detected",
                "message":
                    "the server is in local-mode but received a forwarded \
                     proxy header; pass --trust-proxy to honor it or bind \
                     non-loopback to enable full auth"
            }),
        ));
    }
    Ok(true)
}

#[allow(clippy::result_large_err)]
fn enforce_bearer_auth(
    auth_secret: Option<&[u8]>,
    headers: &HeaderMap,
    workspace_id: &str,
) -> Option<Result<(), Response>> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))?;
    let Some(secret) = auth_secret else {
        return Some(Err(auth_error(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({"error": "bearer_not_configured"}),
        )));
    };
    Some(match actant_auth::verify(token, secret) {
        Ok(claims) if claims.iss == workspace_id => Ok(()),
        Ok(claims) => Err(auth_error(
            StatusCode::FORBIDDEN,
            serde_json::json!({
                "error": "workspace_mismatch",
                "iss": claims.iss,
                "workspace_id": workspace_id
            }),
        )),
        Err(_) => Err(auth_error(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({"error": "invalid_token"}),
        )),
    })
}

fn csrf_header(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-csrf-token")
        .or_else(|| headers.get("X-CSRF-Token"))
        .and_then(|v| v.to_str().ok())
}

#[allow(clippy::result_large_err)]
fn validate_session_auth(
    row: SessionAuthRow,
    headers: &HeaderMap,
    method: &Method,
    workspace_id: &str,
) -> Result<(), Response> {
    if row.revoked_at.is_some() {
        return Err(auth_error(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({"error": "revoked"}),
        ));
    }
    let exp = time::OffsetDateTime::parse(
        &row.expires_at,
        &time::format_description::well_known::Rfc3339,
    )
    .map(|t| t.unix_timestamp())
    .map_err(|_| {
        auth_error(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({"error": "invalid_session_expiry"}),
        )
    })?;
    if time::OffsetDateTime::now_utc().unix_timestamp() > exp {
        return Err(auth_error(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({"error": "expired"}),
        ));
    }
    if row.workspace_id != workspace_id {
        return Err(auth_error(
            StatusCode::FORBIDDEN,
            serde_json::json!({
                "error": "workspace_mismatch",
                "session_workspace": row.workspace_id,
                "workspace_id": workspace_id
            }),
        ));
    }
    if !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        let Some(presented) = csrf_header(headers) else {
            return Err(auth_error(
                StatusCode::FORBIDDEN,
                serde_json::json!({
                    "error": "csrf_required",
                    "message": "X-CSRF-Token header is required for mutating routes"
                }),
            ));
        };
        if !actant_auth::verify_csrf(&row.csrf_secret, presented) {
            return Err(auth_error(
                StatusCode::FORBIDDEN,
                serde_json::json!({
                    "error": "csrf_mismatch",
                    "message": "X-CSRF-Token does not match session"
                }),
            ));
        }
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
fn enforce_missing_auth(auth_secret: Option<&[u8]>) -> Result<(), Response> {
    if auth_secret.is_some() {
        Err(auth_error(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({"error": "missing_authorization"}),
        ))
    } else {
        Ok(())
    }
}

async fn fetch_sqlite_session_auth(
    storage: &Storage,
    token_hash: &str,
) -> Result<Option<SessionAuthRow>, Response> {
    sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT workspace_id, csrf_secret, expires_at, revoked_at
         FROM session_token WHERE token_hash = ?",
    )
    .bind(token_hash)
    .fetch_optional(storage.pool())
    .await
    .map(|row| row.map(SessionAuthRow::from))
    .map_err(|e| err_response(ActantError::Storage(e.to_string())))
}

async fn fetch_postgres_session_auth(
    storage: &PgStorage,
    token_hash: &str,
) -> Result<Option<SessionAuthRow>, Response> {
    sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT workspace_id, csrf_secret, expires_at, revoked_at
         FROM session_token WHERE token_hash = $1",
    )
    .bind(token_hash)
    .fetch_optional(storage.pool())
    .await
    .map(|row| row.map(SessionAuthRow::from))
    .map_err(|e| err_response(ActantError::Storage(e.to_string())))
}

#[allow(clippy::result_large_err)]
async fn enforce_pg_auth(
    state: &PostgresAppState,
    headers: &HeaderMap,
    method: &Method,
    workspace_id: &str,
) -> Result<(), Response> {
    let auth_secret = state.auth_secret.as_deref();
    if enforce_local_auth_bypass(state.local_mode, state.trust_proxy, auth_secret, headers)? {
        return Ok(());
    }
    if let Some(result) = enforce_bearer_auth(auth_secret, headers, workspace_id) {
        return result;
    }
    if let Some(plain) = auth_routes::extract_session_cookie(headers) {
        let hash = actant_auth::hash_token(&plain);
        let Some(row) = fetch_postgres_session_auth(&state.storage, &hash).await? else {
            return Err(auth_error(
                StatusCode::UNAUTHORIZED,
                serde_json::json!({"error": "invalid_session"}),
            ));
        };
        return validate_session_auth(row, headers, method, workspace_id);
    }
    enforce_missing_auth(auth_secret)
}

#[allow(clippy::result_large_err)]
async fn enforce_auth(
    state: &AppState,
    headers: &HeaderMap,
    method: &Method,
    workspace_id: &str,
) -> Result<(), Response> {
    let auth_secret = state.auth_secret.as_deref();
    if enforce_local_auth_bypass(state.local_mode, state.trust_proxy, auth_secret, headers)? {
        return Ok(());
    }
    if let Some(result) = enforce_bearer_auth(auth_secret, headers, workspace_id) {
        return result;
    }
    if let Some(plain) = auth_routes::extract_session_cookie(headers) {
        let hash = actant_auth::hash_token(&plain);
        let Some(row) = fetch_sqlite_session_auth(&state.storage, &hash).await? else {
            return Err(auth_error(
                StatusCode::UNAUTHORIZED,
                serde_json::json!({"error": "invalid_session"}),
            ));
        };
        return validate_session_auth(row, headers, method, workspace_id);
    }
    enforce_missing_auth(auth_secret)
}

async fn dispatch_command(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CommandRequest>,
) -> Response {
    if let Err(resp) = enforce_rate_limit(&s, &req.workspace_id).await {
        return resp;
    }
    if let Err(resp) =
        enforce_auth(&s, &headers, &axum::http::Method::POST, &req.workspace_id).await
    {
        return resp;
    }
    // Record the dispatch attempt for Prometheus. Done after auth and
    // rate-limit have passed so that 401/429 responses don't pollute
    // the per-workspace counter.
    prom::record_command(&req.workspace_id, &req.command_type);
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
    let status = match &e {
        ActantError::InvalidInput(_) => StatusCode::BAD_REQUEST,
        ActantError::NotFound(_) => StatusCode::NOT_FOUND,
        ActantError::PermissionDenied(_) => StatusCode::FORBIDDEN,
        ActantError::ApprovalRequired(_) => StatusCode::ACCEPTED,
        ActantError::ApprovalDenied(_) => StatusCode::FORBIDDEN,
        ActantError::IdempotentReplay(_) => StatusCode::OK,
        ActantError::Conflict(_) => StatusCode::CONFLICT,
        ActantError::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
        ActantError::PolicyHalt(_) => StatusCode::FORBIDDEN,
        ActantError::Storage(_) | ActantError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let code = e.code();
    (
        status,
        Json(serde_json::json!({
            "error": code,
            "code": code,
            "message": e.to_string(),
            "hint": e.hint(),
            "fix": e.fix(),
        })),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// New endpoints for the Swift / Swoosh consumer.
// ---------------------------------------------------------------------------

#[allow(clippy::result_large_err)]
fn workspace_query_required(s: &str) -> Result<(), Response> {
    if s.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_input",
                "message": "workspace_id is required"
            })),
        )
            .into_response());
    }
    Ok(())
}

fn parse_sensitivity(s: &str) -> Result<Sensitivity, ActantError> {
    serde_json::from_value::<Sensitivity>(serde_json::Value::String(s.to_string())).map_err(|_| {
        ActantError::InvalidInput(format!(
            "invalid sensitivity '{s}'; expected one of public|low|medium|high|secret|regulated"
        ))
    })
}

/// Compute the chain hash for a fresh event and append it. Returns the new
/// event id. Mirrors the private `Engine::append_chronicle` used by the
/// command layer (`actant-command::Engine`), so the Chronicle stays a single
/// uniform chain regardless of who writes it.
#[allow(clippy::too_many_arguments)]
async fn append_chronicle_event(
    storage: &Storage,
    workspace_id: &WorkspaceId,
    actor_id: &ActorId,
    event_type: &str,
    causality_kind: CausalityKind,
    sensitivity: Sensitivity,
    payload: &serde_json::Value,
) -> Result<(EventId, String), ActantError> {
    let payload_canon = canonical_json(payload);
    let payload_hash = sha256_hex(payload_canon.as_bytes());
    let prev = storage
        .last_event_hash(workspace_id, None)
        .await?
        .unwrap_or_else(|| "0".repeat(64));
    let event_hash = chain_hash(&prev, &payload_hash);
    let event = AgentEvent {
        id: EventId::new(),
        workspace_id: workspace_id.clone(),
        actor_id: actor_id.clone(),
        session_id: None,
        parent_event_id: None,
        event_type: event_type.into(),
        causality_kind,
        sensitivity,
        authority_scope_id: None,
        payload_ref: None,
        payload_inline: Some(payload_canon),
        payload_hash,
        event_hash,
        created_at: now_rfc3339(),
        model_call_id: None,
        tool_call_id: None,
        workflow_run_id: None,
        memory_id: None,
        artifact_id: None,
        command_id: None,
        effect_id: None,
    };
    let id = event.id.clone();
    let created_at = event.created_at.clone();
    storage.append_event(&event).await?;
    Ok((id, created_at))
}

// --- memories ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct MemoriesQuery {
    workspace_id: String,
    #[serde(default = "default_memory_status")]
    status: String,
}

fn default_memory_status() -> String {
    "approved".into()
}

async fn list_memories(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<MemoriesQuery>,
) -> Response {
    if let Err(resp) = workspace_query_required(&q.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_auth(&s, &headers, &axum::http::Method::GET, &q.workspace_id).await {
        return resp;
    }
    let mut out: Vec<serde_json::Value> = Vec::new();
    let include_approved = matches!(q.status.as_str(), "approved" | "all");
    let include_pending = matches!(q.status.as_str(), "pending" | "all");
    let include_rejected = matches!(q.status.as_str(), "rejected" | "all");
    if !include_approved && !include_pending && !include_rejected {
        return err_response(ActantError::InvalidInput(format!(
            "unknown status '{}'; expected approved|pending|rejected|all",
            q.status
        )));
    }

    if include_approved {
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                Option<f64>,
                String,
                Option<String>,
                i64,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                String,
            ),
        >(
            "SELECT id, workspace_id, text, category, sensitivity, confidence,
                    scope, source_candidate_id, usage_count, last_used_at,
                    expires_at, revoked_at, deleted_at, created_at
             FROM memory
             WHERE workspace_id = ?
             ORDER BY created_at DESC",
        )
        .bind(&q.workspace_id)
        .fetch_all(s.storage.pool())
        .await;
        match rows {
            Ok(rows) => {
                for (
                    id,
                    workspace_id,
                    text,
                    category,
                    sensitivity,
                    confidence,
                    scope,
                    source_candidate_id,
                    usage_count,
                    last_used_at,
                    expires_at,
                    revoked_at,
                    deleted_at,
                    created_at,
                ) in rows
                {
                    out.push(serde_json::json!({
                        "id": id,
                        "workspace_id": workspace_id,
                        "text": text,
                        "category": category,
                        "sensitivity": sensitivity,
                        "confidence": confidence,
                        "scope": scope,
                        "source_candidate_id": source_candidate_id,
                        "usage_count": usage_count,
                        "last_used_at": last_used_at,
                        "expires_at": expires_at,
                        "revoked_at": revoked_at,
                        "deleted_at": deleted_at,
                        "created_at": created_at,
                        "status": "approved",
                    }));
                }
            }
            Err(e) => return err_response(ActantError::Storage(e.to_string())),
        }
    }

    if include_pending || include_rejected {
        // status=pending covers candidate states still under review;
        // status=rejected is exact; status=all unions both — but excludes
        // approved candidates, since the promoted row is already represented
        // via the `memory` table above.
        let sql: &str = match (include_pending, include_rejected) {
            (true, true) => {
                "SELECT id, workspace_id, proposed_by_actor_id, text, category,
                        confidence, sensitivity, status, review_reason, created_at
                 FROM memory_candidate
                 WHERE workspace_id = ? AND status != 'approved'
                 ORDER BY created_at DESC"
            }
            (true, false) => {
                "SELECT id, workspace_id, proposed_by_actor_id, text, category,
                        confidence, sensitivity, status, review_reason, created_at
                 FROM memory_candidate
                 WHERE workspace_id = ? AND status IN ('pending_review','proposed','edited')
                 ORDER BY created_at DESC"
            }
            (false, true) => {
                "SELECT id, workspace_id, proposed_by_actor_id, text, category,
                        confidence, sensitivity, status, review_reason, created_at
                 FROM memory_candidate
                 WHERE workspace_id = ? AND status = 'rejected'
                 ORDER BY created_at DESC"
            }
            (false, false) => unreachable!(),
        };
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                f64,
                String,
                String,
                Option<String>,
                String,
            ),
        >(sql)
        .bind(&q.workspace_id)
        .fetch_all(s.storage.pool())
        .await;
        match rows {
            Ok(rows) => {
                for (
                    id,
                    workspace_id,
                    _proposed_by,
                    text,
                    category,
                    confidence,
                    sensitivity,
                    candidate_status,
                    _review_reason,
                    created_at,
                ) in rows
                {
                    let bucket = match candidate_status.as_str() {
                        "rejected" => "rejected",
                        _ => "pending",
                    };
                    out.push(serde_json::json!({
                        "id": id,
                        "workspace_id": workspace_id,
                        "text": text,
                        "category": category,
                        "sensitivity": sensitivity,
                        "confidence": confidence,
                        "scope": serde_json::Value::Null,
                        "source_candidate_id": serde_json::Value::Null,
                        "usage_count": serde_json::Value::Null,
                        "last_used_at": serde_json::Value::Null,
                        "expires_at": serde_json::Value::Null,
                        "revoked_at": serde_json::Value::Null,
                        "deleted_at": serde_json::Value::Null,
                        "created_at": created_at,
                        "status": bucket,
                    }));
                }
            }
            Err(e) => return err_response(ActantError::Storage(e.to_string())),
        }
    }

    Json(serde_json::json!({ "memories": out })).into_response()
}

// --- memory conflicts -------------------------------------------------------

#[derive(Debug, Deserialize)]
struct MemoryConflictsQuery {
    workspace_id: String,
}

async fn list_memory_conflicts(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<MemoryConflictsQuery>,
) -> Response {
    if let Err(resp) = workspace_query_required(&q.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_auth(&s, &headers, &axum::http::Method::GET, &q.workspace_id).await {
        return resp;
    }
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            String,
        ),
    >(
        "SELECT id, workspace_id, memory_a_id, memory_b_id, conflict_type,
                resolution_policy, last_resolved_at, detected_at
         FROM memory_conflict
         WHERE workspace_id = ?
         ORDER BY detected_at DESC",
    )
    .bind(&q.workspace_id)
    .fetch_all(s.storage.pool())
    .await;
    match rows {
        Ok(rows) => {
            let conflicts: Vec<_> = rows
                .into_iter()
                .map(
                    |(
                        id,
                        workspace_id,
                        a,
                        b,
                        conflict_type,
                        resolution_policy,
                        last_resolved_at,
                        detected_at,
                    )| {
                        serde_json::json!({
                            "id": id,
                            "workspace_id": workspace_id,
                            "memory_a_id": a,
                            "memory_b_id": b,
                            "conflict_type": conflict_type,
                            "resolution_policy": resolution_policy,
                            "last_resolved_at": last_resolved_at,
                            "detected_at": detected_at,
                        })
                    },
                )
                .collect();
            Json(serde_json::json!({ "conflicts": conflicts })).into_response()
        }
        Err(e) => err_response(ActantError::Storage(e.to_string())),
    }
}

// --- permissions ------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct PermissionsQuery {
    workspace_id: String,
}

async fn list_permissions(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<PermissionsQuery>,
) -> Response {
    if let Err(resp) = workspace_query_required(&q.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_auth(&s, &headers, &axum::http::Method::GET, &q.workspace_id).await {
        return resp;
    }
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
        ),
    >(
        "SELECT id, workspace_id, actor_id, permission, resource_pattern,
                sensitivity_ceiling, allowed_actions, granted_by_actor_id,
                expires_at, revoked_at, created_at
         FROM authority_scope
         WHERE workspace_id = ? AND revoked_at IS NULL
         ORDER BY created_at DESC",
    )
    .bind(&q.workspace_id)
    .fetch_all(s.storage.pool())
    .await;
    match rows {
        Ok(rows) => {
            let perms: Vec<_> = rows
                .into_iter()
                .map(
                    |(
                        id,
                        workspace_id,
                        actor_id,
                        permission,
                        resource_pattern,
                        sensitivity_ceiling,
                        allowed_actions,
                        granted_by_actor_id,
                        expires_at,
                        revoked_at,
                        created_at,
                    )| {
                        let actions: serde_json::Value = serde_json::from_str(&allowed_actions)
                            .unwrap_or_else(|_| {
                                serde_json::Value::Array(vec![serde_json::Value::String(
                                    allowed_actions.clone(),
                                )])
                            });
                        serde_json::json!({
                            "id": id,
                            "workspace_id": workspace_id,
                            "actor_id": actor_id,
                            "permission": permission,
                            "resource_pattern": resource_pattern,
                            "sensitivity_ceiling": sensitivity_ceiling,
                            "allowed_actions": actions,
                            "granted_by_actor_id": granted_by_actor_id,
                            "expires_at": expires_at,
                            "revoked_at": revoked_at,
                            "created_at": created_at,
                        })
                    },
                )
                .collect();
            Json(serde_json::json!({ "permissions": perms })).into_response()
        }
        Err(e) => err_response(ActantError::Storage(e.to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct CreatePermissionRequest {
    workspace_id: String,
    actor_id: String,
    permission: String,
    level: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    allowed_actions: Option<serde_json::Value>,
}

async fn create_permission(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreatePermissionRequest>,
) -> Response {
    if let Err(resp) = workspace_query_required(&req.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_rate_limit(&s, &req.workspace_id).await {
        return resp;
    }
    if let Err(resp) =
        enforce_auth(&s, &headers, &axum::http::Method::POST, &req.workspace_id).await
    {
        return resp;
    }
    if req.actor_id.is_empty() {
        return err_response(ActantError::InvalidInput("actor_id is required".into()));
    }
    if req.permission.is_empty() {
        return err_response(ActantError::InvalidInput("permission is required".into()));
    }
    let level = match parse_sensitivity(&req.level) {
        Ok(s) => s,
        Err(e) => return err_response(e),
    };
    let level_s = serde_json::to_string(&level)
        .unwrap_or_else(|_| "\"low\"".into())
        .trim_matches('"')
        .to_string();
    let actions_json = match req.allowed_actions {
        Some(v) => {
            if !v.is_array() {
                return err_response(ActantError::InvalidInput(
                    "allowed_actions must be a JSON array".into(),
                ));
            }
            v.to_string()
        }
        None => serde_json::json!(["*"]).to_string(),
    };
    let id = format!("auth_{}", ulid::Ulid::new());
    let now = now_rfc3339();
    let res = sqlx::query(
        "INSERT INTO authority_scope
            (id, workspace_id, actor_id, permission, resource_pattern,
             sensitivity_ceiling, allowed_actions, granted_by_actor_id,
             expires_at, revoked_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?)",
    )
    .bind(&id)
    .bind(&req.workspace_id)
    .bind(&req.actor_id)
    .bind(&req.permission)
    .bind(&req.scope)
    .bind(&level_s)
    .bind(&actions_json)
    .bind(&req.actor_id)
    .bind(&now)
    .execute(s.storage.pool())
    .await;
    match res {
        Ok(_) => Json(serde_json::json!({ "id": id })).into_response(),
        Err(e) => err_response(ActantError::Storage(e.to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct RevokePermissionRequest {
    workspace_id: String,
    authority_scope_id: String,
}

async fn revoke_permission(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<RevokePermissionRequest>,
) -> Response {
    if let Err(resp) = workspace_query_required(&req.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_rate_limit(&s, &req.workspace_id).await {
        return resp;
    }
    if let Err(resp) =
        enforce_auth(&s, &headers, &axum::http::Method::DELETE, &req.workspace_id).await
    {
        return resp;
    }
    if req.authority_scope_id.is_empty() {
        return err_response(ActantError::InvalidInput(
            "authority_scope_id is required".into(),
        ));
    }
    let res = sqlx::query(
        "UPDATE authority_scope SET revoked_at = ?
         WHERE id = ? AND workspace_id = ?",
    )
    .bind(now_rfc3339())
    .bind(&req.authority_scope_id)
    .bind(&req.workspace_id)
    .execute(s.storage.pool())
    .await;
    match res {
        Ok(r) if r.rows_affected() == 0 => err_response(ActantError::NotFound(format!(
            "authority_scope {} in workspace {}",
            req.authority_scope_id, req.workspace_id
        ))),
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => err_response(ActantError::Storage(e.to_string())),
    }
}

// --- setup reports + scout records (shared helpers) -------------------------

async fn write_report_event_and_artifact(
    storage: &Storage,
    workspace_id: &str,
    actor_id: &str,
    kind: &str,
    payload: serde_json::Value,
    content: &str,
    sensitivity: Sensitivity,
) -> Result<(String, String), ActantError> {
    let ws = WorkspaceId::from_string(workspace_id.to_string());
    let actor = ActorId::from_string(actor_id.to_string());
    ensure_workspace_actor(storage, &ws, &actor).await?;
    let (event_id, created_at) = append_chronicle_event(
        storage,
        &ws,
        &actor,
        kind,
        CausalityKind::Audit,
        sensitivity,
        &payload,
    )
    .await?;
    let sens_s = serde_json::to_string(&sensitivity)
        .unwrap_or_else(|_| "\"low\"".into())
        .trim_matches('"')
        .to_string();
    let artifact_id = format!("art_{}", ulid::Ulid::new());
    let content_hash = sha256_hex(content.as_bytes());
    sqlx::query(
        "INSERT INTO artifact
            (id, workspace_id, kind, uri, content_hash, bytes, sensitivity,
             created_by_actor_id, created_at, deleted_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(&artifact_id)
    .bind(workspace_id)
    .bind(kind)
    .bind(format!("actantdb://event/{}", event_id.as_str()))
    .bind(&content_hash)
    .bind(content.len() as i64)
    .bind(&sens_s)
    .bind(actor_id)
    .bind(&created_at)
    .execute(storage.pool())
    .await
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok((artifact_id, event_id.as_str().to_string()))
}

async fn ensure_workspace_actor(
    storage: &Storage,
    workspace_id: &WorkspaceId,
    actor_id: &ActorId,
) -> Result<(), ActantError> {
    if storage.get_workspace(workspace_id).await?.is_none() {
        storage
            .insert_workspace(&Workspace {
                id: workspace_id.clone(),
                name: workspace_id.as_str().to_string(),
                created_at: now_rfc3339(),
                archived_at: None,
            })
            .await?;
    }
    if storage.get_actor(actor_id).await?.is_none() {
        storage
            .insert_actor(&Actor {
                id: actor_id.clone(),
                workspace_id: workspace_id.clone(),
                kind: ActorKind::Human,
                display_name: actor_id.as_str().to_string(),
                created_at: now_rfc3339(),
                disabled_at: None,
            })
            .await?;
    }
    Ok(())
}

fn event_id_from_uri(uri: &str) -> Option<String> {
    uri.strip_prefix("actantdb://event/").map(String::from)
}

// --- setup-reports ----------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CreateSetupReportRequest {
    workspace_id: String,
    actor_id: String,
    content: String,
    #[serde(default)]
    sensitivity: Option<String>,
}

async fn create_setup_report(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateSetupReportRequest>,
) -> Response {
    if let Err(resp) = workspace_query_required(&req.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_rate_limit(&s, &req.workspace_id).await {
        return resp;
    }
    if let Err(resp) =
        enforce_auth(&s, &headers, &axum::http::Method::POST, &req.workspace_id).await
    {
        return resp;
    }
    if req.actor_id.is_empty() {
        return err_response(ActantError::InvalidInput("actor_id is required".into()));
    }
    if req.content.is_empty() {
        return err_response(ActantError::InvalidInput("content is required".into()));
    }
    let sensitivity = match req.sensitivity.as_deref() {
        None => Sensitivity::Low,
        Some(s) => match parse_sensitivity(s) {
            Ok(v) => v,
            Err(e) => return err_response(e),
        },
    };
    let payload = serde_json::json!({
        "content": req.content,
        "actor_id": req.actor_id,
    });
    match write_report_event_and_artifact(
        &s.storage,
        &req.workspace_id,
        &req.actor_id,
        "setup_report",
        payload,
        &req.content,
        sensitivity,
    )
    .await
    {
        Ok((artifact_id, event_id)) => Json(serde_json::json!({
            "artifact_id": artifact_id,
            "event_id": event_id,
        }))
        .into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Debug, Deserialize)]
struct SetupReportsQuery {
    workspace_id: String,
    #[serde(default)]
    latest: Option<bool>,
}

async fn list_setup_reports(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<SetupReportsQuery>,
) -> Response {
    if let Err(resp) = workspace_query_required(&q.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_auth(&s, &headers, &axum::http::Method::GET, &q.workspace_id).await {
        return resp;
    }
    if q.latest.unwrap_or(false) {
        let row = sqlx::query_as::<_, (String, String, String, i64, String)>(
            "SELECT id, uri, content_hash, bytes, created_at
             FROM artifact
             WHERE workspace_id = ? AND kind = 'setup_report' AND deleted_at IS NULL
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&q.workspace_id)
        .fetch_optional(s.storage.pool())
        .await;
        match row {
            Ok(None) => {
                Json(serde_json::json!({ "report": serde_json::Value::Null })).into_response()
            }
            Ok(Some((art_id, uri, content_hash, bytes, created_at))) => {
                let Some(event_id) = event_id_from_uri(&uri) else {
                    return err_response(ActantError::Storage(format!(
                        "artifact {art_id} has unexpected uri {uri}"
                    )));
                };
                let ev = sqlx::query_as::<_, (Option<String>,)>(
                    "SELECT payload_inline FROM agent_event WHERE id = ?",
                )
                .bind(&event_id)
                .fetch_optional(s.storage.pool())
                .await;
                let content = match ev {
                    Ok(Some((Some(p),))) => serde_json::from_str::<serde_json::Value>(&p)
                        .ok()
                        .and_then(|v| v.get("content").and_then(|c| c.as_str()).map(String::from))
                        .unwrap_or_default(),
                    Ok(_) => String::new(),
                    Err(e) => return err_response(ActantError::Storage(e.to_string())),
                };
                Json(serde_json::json!({
                    "report": {
                        "artifact_id": art_id,
                        "event_id": event_id,
                        "content": content,
                        "content_hash": content_hash,
                        "bytes": bytes,
                        "created_at": created_at,
                    }
                }))
                .into_response()
            }
            Err(e) => err_response(ActantError::Storage(e.to_string())),
        }
    } else {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, uri, created_at
             FROM artifact
             WHERE workspace_id = ? AND kind = 'setup_report' AND deleted_at IS NULL
             ORDER BY created_at DESC LIMIT 100",
        )
        .bind(&q.workspace_id)
        .fetch_all(s.storage.pool())
        .await;
        match rows {
            Ok(rows) => {
                let mut reports: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
                for (art_id, uri, created_at) in rows {
                    let Some(event_id) = event_id_from_uri(&uri) else {
                        continue;
                    };
                    let ev = sqlx::query_as::<_, (Option<String>,)>(
                        "SELECT payload_inline FROM agent_event WHERE id = ?",
                    )
                    .bind(&event_id)
                    .fetch_optional(s.storage.pool())
                    .await;
                    let content = match ev {
                        Ok(Some((Some(p),))) => serde_json::from_str::<serde_json::Value>(&p)
                            .ok()
                            .and_then(|v| {
                                v.get("content").and_then(|c| c.as_str()).map(String::from)
                            })
                            .unwrap_or_default(),
                        _ => String::new(),
                    };
                    reports.push(serde_json::json!({
                        "artifact_id": art_id,
                        "event_id": event_id,
                        "content": content,
                        "created_at": created_at,
                    }));
                }
                Json(serde_json::json!({ "reports": reports })).into_response()
            }
            Err(e) => err_response(ActantError::Storage(e.to_string())),
        }
    }
}

// --- scout-records ----------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CreateScoutRecordRequest {
    workspace_id: String,
    actor_id: String,
    source_id: String,
    kind: String,
    sensitivity: String,
    content: String,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

async fn create_scout_record(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateScoutRecordRequest>,
) -> Response {
    if let Err(resp) = workspace_query_required(&req.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_rate_limit(&s, &req.workspace_id).await {
        return resp;
    }
    if let Err(resp) =
        enforce_auth(&s, &headers, &axum::http::Method::POST, &req.workspace_id).await
    {
        return resp;
    }
    if req.actor_id.is_empty() {
        return err_response(ActantError::InvalidInput("actor_id is required".into()));
    }
    if req.source_id.is_empty() {
        return err_response(ActantError::InvalidInput("source_id is required".into()));
    }
    if req.kind.is_empty() {
        return err_response(ActantError::InvalidInput("kind is required".into()));
    }
    if req.content.is_empty() {
        return err_response(ActantError::InvalidInput("content is required".into()));
    }
    let sensitivity = match parse_sensitivity(&req.sensitivity) {
        Ok(v) => v,
        Err(e) => return err_response(e),
    };
    let metadata = req.metadata.unwrap_or(serde_json::Value::Null);
    let payload = serde_json::json!({
        "content": req.content,
        "source_id": req.source_id,
        "kind": req.kind,
        "actor_id": req.actor_id,
        "metadata": metadata,
    });
    match write_report_event_and_artifact(
        &s.storage,
        &req.workspace_id,
        &req.actor_id,
        "scout_record",
        payload,
        &req.content,
        sensitivity,
    )
    .await
    {
        Ok((artifact_id, event_id)) => Json(serde_json::json!({
            "artifact_id": artifact_id,
            "event_id": event_id,
        }))
        .into_response(),
        Err(e) => err_response(e),
    }
}

#[derive(Debug, Deserialize)]
struct ScoutRecordsQuery {
    workspace_id: String,
    #[serde(default)]
    source: Option<String>,
}

async fn list_scout_records(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<ScoutRecordsQuery>,
) -> Response {
    if let Err(resp) = workspace_query_required(&q.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_auth(&s, &headers, &axum::http::Method::GET, &q.workspace_id).await {
        return resp;
    }
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, uri, sensitivity, content_hash, created_at
         FROM artifact
         WHERE workspace_id = ? AND kind = 'scout_record' AND deleted_at IS NULL
         ORDER BY created_at DESC LIMIT 100",
    )
    .bind(&q.workspace_id)
    .fetch_all(s.storage.pool())
    .await;
    let rows = match rows {
        Ok(r) => r,
        Err(e) => return err_response(ActantError::Storage(e.to_string())),
    };
    let mut records: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
    for (art_id, uri, sensitivity, _content_hash, created_at) in rows {
        let Some(event_id) = event_id_from_uri(&uri) else {
            continue;
        };
        let ev = sqlx::query_as::<_, (Option<String>,)>(
            "SELECT payload_inline FROM agent_event WHERE id = ?",
        )
        .bind(&event_id)
        .fetch_optional(s.storage.pool())
        .await;
        let payload: serde_json::Value = match ev {
            Ok(Some((Some(p),))) => serde_json::from_str(&p).unwrap_or(serde_json::Value::Null),
            _ => serde_json::Value::Null,
        };
        let source_id = payload
            .get("source_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(filter) = q.source.as_deref() {
            if !filter.is_empty() && source_id != filter {
                continue;
            }
        }
        let content = payload
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let kind = payload
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let metadata = payload
            .get("metadata")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        records.push(serde_json::json!({
            "artifact_id": art_id,
            "event_id": event_id,
            "source_id": source_id,
            "kind": kind,
            "sensitivity": sensitivity,
            "content": content,
            "metadata": metadata,
            "created_at": created_at,
        }));
    }
    Json(serde_json::json!({ "records": records })).into_response()
}

// --- entities + entity-relations -------------------------------------------

#[derive(Debug, Deserialize)]
struct EntitiesQuery {
    workspace_id: String,
    #[serde(rename = "type", default)]
    entity_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateEntityRequest {
    workspace_id: String,
    #[serde(rename = "type")]
    entity_type: String,
    canonical_name: String,
    #[serde(default)]
    aliases: Option<Vec<String>>,
    #[serde(default)]
    sensitivity: Option<String>,
    #[serde(default)]
    capsule_id: Option<String>,
    #[serde(default)]
    source_events: Option<Vec<String>>,
}

async fn create_entity(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateEntityRequest>,
) -> Response {
    if let Err(resp) = workspace_query_required(&req.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_rate_limit(&s, &req.workspace_id).await {
        return resp;
    }
    if let Err(resp) =
        enforce_auth(&s, &headers, &axum::http::Method::POST, &req.workspace_id).await
    {
        return resp;
    }
    if req.entity_type.is_empty() {
        return err_response(ActantError::InvalidInput("type is required".into()));
    }
    if req.canonical_name.is_empty() {
        return err_response(ActantError::InvalidInput(
            "canonical_name is required".into(),
        ));
    }
    let sensitivity = match parse_sensitivity(req.sensitivity.as_deref().unwrap_or("low")) {
        Ok(s) => s,
        Err(e) => return err_response(e),
    };
    let sens_s = serde_json::to_string(&sensitivity)
        .unwrap_or_else(|_| "\"low\"".into())
        .trim_matches('"')
        .to_string();
    let aliases_json =
        serde_json::to_string(&req.aliases.unwrap_or_default()).unwrap_or_else(|_| "[]".into());
    let sources_json = serde_json::to_string(&req.source_events.unwrap_or_default())
        .unwrap_or_else(|_| "[]".into());
    let id = format!("ent_{}", ulid::Ulid::new());
    let now = now_rfc3339();
    let res = sqlx::query(
        "INSERT INTO entity
            (id, workspace_id, type, canonical_name, aliases, sensitivity,
             source_events, capsule_id, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.workspace_id)
    .bind(&req.entity_type)
    .bind(&req.canonical_name)
    .bind(&aliases_json)
    .bind(&sens_s)
    .bind(&sources_json)
    .bind(&req.capsule_id)
    .bind(&now)
    .execute(s.storage.pool())
    .await;
    match res {
        Ok(_) => Json(serde_json::json!({ "id": id })).into_response(),
        Err(e) => err_response(ActantError::Storage(e.to_string())),
    }
}

type EntityRowTuple = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
);

async fn list_entities(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<EntitiesQuery>,
) -> Response {
    if let Err(resp) = workspace_query_required(&q.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_auth(&s, &headers, &axum::http::Method::GET, &q.workspace_id).await {
        return resp;
    }
    let rows: Result<Vec<EntityRowTuple>, _> = match q.entity_type.as_deref() {
        Some(t) if !t.is_empty() => {
            sqlx::query_as(
                "SELECT id, workspace_id, type, canonical_name, aliases, sensitivity,
                    source_events, capsule_id, created_at
             FROM entity WHERE workspace_id = ? AND type = ?
             ORDER BY created_at DESC LIMIT 500",
            )
            .bind(&q.workspace_id)
            .bind(t)
            .fetch_all(s.storage.pool())
            .await
        }
        _ => {
            sqlx::query_as(
                "SELECT id, workspace_id, type, canonical_name, aliases, sensitivity,
                    source_events, capsule_id, created_at
             FROM entity WHERE workspace_id = ?
             ORDER BY created_at DESC LIMIT 500",
            )
            .bind(&q.workspace_id)
            .fetch_all(s.storage.pool())
            .await
        }
    };
    match rows {
        Ok(rows) => {
            let entities: Vec<_> = rows
                .into_iter()
                .map(
                    |(
                        id,
                        workspace_id,
                        entity_type,
                        canonical_name,
                        aliases,
                        sensitivity,
                        source_events,
                        capsule_id,
                        created_at,
                    )| {
                        let aliases_v: serde_json::Value = serde_json::from_str(&aliases)
                            .unwrap_or(serde_json::Value::Array(vec![]));
                        let sources_v: serde_json::Value = serde_json::from_str(&source_events)
                            .unwrap_or(serde_json::Value::Array(vec![]));
                        serde_json::json!({
                            "id": id,
                            "workspace_id": workspace_id,
                            "type": entity_type,
                            "canonical_name": canonical_name,
                            "aliases": aliases_v,
                            "sensitivity": sensitivity,
                            "source_events": sources_v,
                            "capsule_id": capsule_id,
                            "created_at": created_at,
                        })
                    },
                )
                .collect();
            Json(serde_json::json!({ "entities": entities })).into_response()
        }
        Err(e) => err_response(ActantError::Storage(e.to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct EntityRelationsQuery {
    workspace_id: String,
    /// When set, returns relations where `entity` appears as source OR target.
    #[serde(default)]
    entity: Option<String>,
    /// When set, restrict to a specific relation_type.
    #[serde(default)]
    relation_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateEntityRelationRequest {
    workspace_id: String,
    source_entity: String,
    relation_type: String,
    target_entity: String,
    #[serde(default = "default_confidence")]
    confidence: f64,
    #[serde(default)]
    evidence_events: Option<Vec<String>>,
}

fn default_confidence() -> f64 {
    1.0
}

async fn create_entity_relation(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateEntityRelationRequest>,
) -> Response {
    if let Err(resp) = workspace_query_required(&req.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_rate_limit(&s, &req.workspace_id).await {
        return resp;
    }
    if let Err(resp) =
        enforce_auth(&s, &headers, &axum::http::Method::POST, &req.workspace_id).await
    {
        return resp;
    }
    if req.source_entity.is_empty() || req.target_entity.is_empty() || req.relation_type.is_empty()
    {
        return err_response(ActantError::InvalidInput(
            "source_entity, target_entity, relation_type are required".into(),
        ));
    }
    if !req.confidence.is_finite() || !(0.0..=1.0).contains(&req.confidence) {
        return err_response(ActantError::InvalidInput(
            "confidence must be in [0.0, 1.0]".into(),
        ));
    }
    let evidence_json = serde_json::to_string(&req.evidence_events.unwrap_or_default())
        .unwrap_or_else(|_| "[]".into());
    let id = format!("rel_{}", ulid::Ulid::new());
    let res = sqlx::query(
        "INSERT INTO entity_relation
            (id, workspace_id, source_entity, relation_type, target_entity,
             confidence, evidence_events)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.workspace_id)
    .bind(&req.source_entity)
    .bind(&req.relation_type)
    .bind(&req.target_entity)
    .bind(req.confidence)
    .bind(&evidence_json)
    .execute(s.storage.pool())
    .await;
    match res {
        Ok(_) => Json(serde_json::json!({ "id": id })).into_response(),
        Err(e) => err_response(ActantError::Storage(e.to_string())),
    }
}

async fn list_entity_relations(
    State(s): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(q): Query<EntityRelationsQuery>,
) -> Response {
    if let Err(resp) = workspace_query_required(&q.workspace_id) {
        return resp;
    }
    if let Err(resp) = enforce_auth(&s, &headers, &axum::http::Method::GET, &q.workspace_id).await {
        return resp;
    }
    type RelationRowTuple = (String, String, String, String, String, f64, String);
    let rows: Result<Vec<RelationRowTuple>, _> =
        match (q.entity.as_deref(), q.relation_type.as_deref()) {
            (Some(e), Some(rt)) if !e.is_empty() && !rt.is_empty() => {
                sqlx::query_as(
                    "SELECT id, workspace_id, source_entity, relation_type, target_entity,
                    confidence, evidence_events
             FROM entity_relation
             WHERE workspace_id = ? AND relation_type = ?
               AND (source_entity = ? OR target_entity = ?)
             ORDER BY id DESC LIMIT 500",
                )
                .bind(&q.workspace_id)
                .bind(rt)
                .bind(e)
                .bind(e)
                .fetch_all(s.storage.pool())
                .await
            }
            (Some(e), _) if !e.is_empty() => {
                sqlx::query_as(
                    "SELECT id, workspace_id, source_entity, relation_type, target_entity,
                    confidence, evidence_events
             FROM entity_relation
             WHERE workspace_id = ? AND (source_entity = ? OR target_entity = ?)
             ORDER BY id DESC LIMIT 500",
                )
                .bind(&q.workspace_id)
                .bind(e)
                .bind(e)
                .fetch_all(s.storage.pool())
                .await
            }
            (_, Some(rt)) if !rt.is_empty() => {
                sqlx::query_as(
                    "SELECT id, workspace_id, source_entity, relation_type, target_entity,
                    confidence, evidence_events
             FROM entity_relation
             WHERE workspace_id = ? AND relation_type = ?
             ORDER BY id DESC LIMIT 500",
                )
                .bind(&q.workspace_id)
                .bind(rt)
                .fetch_all(s.storage.pool())
                .await
            }
            _ => {
                sqlx::query_as(
                    "SELECT id, workspace_id, source_entity, relation_type, target_entity,
                    confidence, evidence_events
             FROM entity_relation
             WHERE workspace_id = ?
             ORDER BY id DESC LIMIT 500",
                )
                .bind(&q.workspace_id)
                .fetch_all(s.storage.pool())
                .await
            }
        };
    match rows {
        Ok(rows) => {
            let relations: Vec<_> = rows
                .into_iter()
                .map(
                    |(
                        id,
                        workspace_id,
                        source_entity,
                        relation_type,
                        target_entity,
                        confidence,
                        evidence_events,
                    )| {
                        let evidence_v: serde_json::Value = serde_json::from_str(&evidence_events)
                            .unwrap_or(serde_json::Value::Array(vec![]));
                        serde_json::json!({
                            "id": id,
                            "workspace_id": workspace_id,
                            "source_entity": source_entity,
                            "relation_type": relation_type,
                            "target_entity": target_entity,
                            "confidence": confidence,
                            "evidence_events": evidence_v,
                        })
                    },
                )
                .collect();
            Json(serde_json::json!({ "relations": relations })).into_response()
        }
        Err(e) => err_response(ActantError::Storage(e.to_string())),
    }
}

/// Bootstrap helper: open storage, build state, return router + pool. Used
/// by the binary and by tests. Defaults to local mode (loopback-trusted) so
/// existing tests keep working without changes.
pub async fn bootstrap(
    db_path: Option<std::path::PathBuf>,
) -> Result<(Router, AppState), ActantError> {
    let (router, state, _link_code) = bootstrap_with_mode(db_path, true, false, false).await?;
    Ok((router, state))
}

/// Extended bootstrap that returns a freshly-minted link code when the
/// workspace is unowned and the server is bound non-loopback.
///
/// * `local_mode = true` — loopback bind, no link code is minted.
/// * `local_mode = false` and there is no `workspace_owner` row for
///   `ws_default` — a fresh code is generated; any prior unconsumed code
///   is invalidated. The returned `String` is the display form
///   (`xxxx-xxxx-xxxx`) for the binary to print on stderr.
/// * `local_mode = false` and the workspace is already claimed — no code,
///   `None`.
/// * `trust_proxy` — when true, the chokepoint honors `X-Forwarded-For`
///   even in `local_mode`. See [`AppState::trust_proxy`].
pub async fn bootstrap_with_mode(
    db_path: Option<std::path::PathBuf>,
    local_mode: bool,
    tls_enabled: bool,
    trust_proxy: bool,
) -> Result<(Router, AppState, Option<String>), ActantError> {
    let cfg = match db_path {
        Some(p) => actant_storage::StorageConfig::file(p),
        None => actant_storage::StorageConfig::in_memory(),
    };
    let storage = Storage::open(cfg).await?;
    let state = AppState::new(storage)
        .with_local_mode(local_mode)
        .with_tls_enabled(tls_enabled)
        .with_trust_proxy(trust_proxy);
    seed_if_empty(&state).await?;
    let link_code = if local_mode {
        None
    } else {
        auth_routes::mint_link_code_for(&state.storage, auth_routes::DEFAULT_WORKSPACE_ID).await?
    };
    let router = router(state.clone());
    Ok((router, state, link_code))
}

/// Bootstrap the Postgres-backed core HTTP server.
pub async fn bootstrap_postgres(
    database_url: &str,
) -> Result<(Router, PostgresAppState), ActantError> {
    bootstrap_postgres_with_mode(database_url, true, false).await
}

/// Bootstrap the Postgres-backed core HTTP server with explicit auth mode.
pub async fn bootstrap_postgres_with_mode(
    database_url: &str,
    local_mode: bool,
    trust_proxy: bool,
) -> Result<(Router, PostgresAppState), ActantError> {
    let storage = PgStorage::open(database_url).await?;
    let state = PostgresAppState::new(storage)
        .with_local_mode(local_mode)
        .with_trust_proxy(trust_proxy);
    seed_postgres_if_empty(&state).await?;
    let router = postgres_router(state.clone());
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

async fn seed_postgres_if_empty(state: &PostgresAppState) -> Result<(), ActantError> {
    let ws_id = WorkspaceId::from_string("ws_default".to_string());
    if state.storage.get_workspace(&ws_id).await?.is_some() {
        return Ok(());
    }
    let ws = actant_core::Workspace {
        id: ws_id,
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
