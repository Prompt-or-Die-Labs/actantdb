//! HTTP client for ActantDB.

use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client as Http, Method, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use url::Url;

use crate::error::{ActantError, Result};
use crate::types::{
    AgentEvent, ApprovalsResponse, CommandRequest, CommandResponse, EventsResponse, Healthz,
    MemoryRow, PendingApproval, ReplayCheckpointResponse, ReplayMode, SyncSinceResponse,
};
use actant_contracts::{ApprovalDecision, ApprovalRequest, ReplayDiff, Sensitivity};

/// HTTP client for an ActantDB server.
///
/// Cheap to clone (everything is `Arc`-shared internally), so build one at
/// startup and pass it around.
#[derive(Debug, Clone)]
pub struct ActantClient {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    base_url: Url,
    token: Option<String>,
    workspace_id: Option<String>,
    actor_id: Option<String>,
    http: Http,
}

impl ActantClient {
    /// Build a new client against the given base URL.
    ///
    /// `base_url` should not include a path — endpoints are joined under
    /// `/v1/`. Example: `Url::parse("http://127.0.0.1:4555").unwrap()`.
    pub fn new(base_url: Url) -> Self {
        let http = Http::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Http::new());
        Self {
            inner: Arc::new(Inner {
                base_url,
                token: None,
                workspace_id: None,
                actor_id: None,
                http,
            }),
        }
    }

    /// Builder: supply a bearer token. Pass nothing to talk to a loopback
    /// server in `local_mode`.
    pub fn with_token(self, token: impl Into<String>) -> Self {
        self.with_modified(|i| i.token = Some(token.into()))
    }

    /// Builder: set a default workspace id used by convenience methods that
    /// accept `None` for `workspace_id`.
    pub fn with_workspace_id(self, workspace_id: impl Into<String>) -> Self {
        self.with_modified(|i| i.workspace_id = Some(workspace_id.into()))
    }

    /// Builder: set a default actor id used by convenience methods that
    /// accept `None` for `actor_id`.
    pub fn with_actor_id(self, actor_id: impl Into<String>) -> Self {
        self.with_modified(|i| i.actor_id = Some(actor_id.into()))
    }

    /// Builder: override the underlying `reqwest::Client` (custom timeout /
    /// proxy / TLS config).
    pub fn with_http_client(self, http: Http) -> Self {
        self.with_modified(|i| i.http = http)
    }

    fn with_modified(self, f: impl FnOnce(&mut Inner)) -> Self {
        let mut owned = Arc::try_unwrap(self.inner).unwrap_or_else(|arc| Inner {
            base_url: arc.base_url.clone(),
            token: arc.token.clone(),
            workspace_id: arc.workspace_id.clone(),
            actor_id: arc.actor_id.clone(),
            http: arc.http.clone(),
        });
        f(&mut owned);
        Self {
            inner: Arc::new(owned),
        }
    }

    /// Base URL the client was constructed with.
    pub fn base_url(&self) -> &Url {
        &self.inner.base_url
    }

    /// Bearer token in use, if any.
    pub fn token(&self) -> Option<&str> {
        self.inner.token.as_deref()
    }

    /// Default workspace id, if any.
    pub fn default_workspace_id(&self) -> Option<&str> {
        self.inner.workspace_id.as_deref()
    }

    /// Default actor id, if any.
    pub fn default_actor_id(&self) -> Option<&str> {
        self.inner.actor_id.as_deref()
    }

    // -----------------------------------------------------------------
    // Health probes

    /// `GET /v1/healthz`.
    pub async fn healthz(&self) -> Result<Healthz> {
        self.request_json(Method::GET, "/v1/healthz", &[], None::<&()>)
            .await
    }

    /// `GET /v1/healthz/startup`.
    pub async fn healthz_startup(&self) -> Result<Healthz> {
        self.request_json(Method::GET, "/v1/healthz/startup", &[], None::<&()>)
            .await
    }

    /// `GET /v1/healthz/live`.
    pub async fn healthz_live(&self) -> Result<Healthz> {
        self.request_json(Method::GET, "/v1/healthz/live", &[], None::<&()>)
            .await
    }

    /// `GET /v1/healthz/ready`.
    pub async fn healthz_ready(&self) -> Result<Healthz> {
        self.request_json(Method::GET, "/v1/healthz/ready", &[], None::<&()>)
            .await
    }

    // -----------------------------------------------------------------
    // Metadata

    /// `GET /v1/metadata/commands` — list of registered command types.
    ///
    /// Returns the wire shape as a free-form JSON value, then projects the
    /// commands list. The server emits `{ "commands": [{ "name": "...", … }] }`.
    pub async fn metadata_commands(&self) -> Result<Vec<String>> {
        #[derive(serde::Deserialize)]
        struct Meta {
            commands: Vec<Cmd>,
        }
        #[derive(serde::Deserialize)]
        struct Cmd {
            name: String,
        }
        let m: Meta = self
            .request_json(Method::GET, "/v1/metadata/commands", &[], None::<&()>)
            .await?;
        Ok(m.commands.into_iter().map(|c| c.name).collect())
    }

    /// `GET /v1/openapi.yaml` — raw OpenAPI document as a string.
    pub async fn openapi(&self) -> Result<String> {
        let resp = self
            .send(Method::GET, "/v1/openapi.yaml", &[], None::<&()>)
            .await?;
        let status = resp.status().as_u16();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ActantError::Transport(e.to_string()))?;
        if (200..300).contains(&status) {
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        } else {
            Err(ActantError::from_response(status, bytes.to_vec()))
        }
    }

    /// `GET /v1/metrics` — Prometheus exposition format.
    pub async fn metrics(&self) -> Result<String> {
        let resp = self
            .send(Method::GET, "/v1/metrics", &[], None::<&()>)
            .await?;
        let status = resp.status().as_u16();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ActantError::Transport(e.to_string()))?;
        if (200..300).contains(&status) {
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        } else {
            Err(ActantError::from_response(status, bytes.to_vec()))
        }
    }

    // -----------------------------------------------------------------
    // Command dispatch

    /// Generic command dispatch. Prefer the typed convenience methods unless
    /// you need to call a command this SDK version doesn't know about.
    pub async fn dispatch(
        &self,
        workspace_id: Option<&str>,
        actor_id: Option<&str>,
        command_type: &str,
        input: Value,
        idempotency_key: Option<String>,
    ) -> Result<CommandResponse> {
        let body = CommandRequest {
            workspace_id: self.resolve_workspace_id(workspace_id)?.to_string(),
            actor_id: self.resolve_actor_id(actor_id)?.to_string(),
            command_type: command_type.to_string(),
            input,
            idempotency_key,
        };
        self.request_json(Method::POST, "/v1/command", &[], Some(&body))
            .await
    }

    /// Create a new session. Returns the new `session_id`.
    pub async fn create_session(
        &self,
        workspace_id: Option<&str>,
        actor_id: Option<&str>,
        title: Option<&str>,
    ) -> Result<String> {
        let mut input = serde_json::Map::new();
        if let Some(t) = title {
            input.insert("title".into(), Value::String(t.to_string()));
        }
        let resp = self
            .dispatch(
                workspace_id,
                actor_id,
                "create_session",
                Value::Object(input),
                None,
            )
            .await?;
        resp.result
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| ActantError::Decoding {
                message: "create_session result missing session_id".into(),
                body: serde_json::to_vec(&resp.result).unwrap_or_default(),
            })
    }

    /// Append a user message to a session.
    pub async fn append_user_message(
        &self,
        workspace_id: Option<&str>,
        actor_id: Option<&str>,
        session_id: &str,
        text: &str,
    ) -> Result<CommandResponse> {
        self.dispatch(
            workspace_id,
            actor_id,
            "append_user_message",
            serde_json::json!({ "session_id": session_id, "text": text }),
            None,
        )
        .await
    }

    /// Append an agent message to a session.
    pub async fn append_agent_message(
        &self,
        workspace_id: Option<&str>,
        actor_id: Option<&str>,
        session_id: &str,
        text: &str,
    ) -> Result<CommandResponse> {
        self.dispatch(
            workspace_id,
            actor_id,
            "append_agent_message",
            serde_json::json!({ "session_id": session_id, "text": text }),
            None,
        )
        .await
    }

    /// Propose a tool call. Returns the raw command response — `result`
    /// typically carries `{ tool_call_id, status, verdict }`.
    pub async fn request_tool_call(
        &self,
        workspace_id: Option<&str>,
        actor_id: Option<&str>,
        session_id: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<CommandResponse> {
        self.dispatch(
            workspace_id,
            actor_id,
            "request_tool_call",
            serde_json::json!({
                "session_id": session_id,
                "tool_name":  tool_name,
                "arguments":  arguments,
            }),
            None,
        )
        .await
    }

    /// Approve a previously-requested tool call.
    pub async fn approve_tool_call(
        &self,
        workspace_id: Option<&str>,
        actor_id: Option<&str>,
        tool_call_id: &str,
        scope: &str,
    ) -> Result<CommandResponse> {
        self.dispatch(
            workspace_id,
            actor_id,
            "approve_tool_call",
            serde_json::json!({ "tool_call_id": tool_call_id, "scope": scope }),
            None,
        )
        .await
    }

    /// Deny a previously-requested tool call.
    pub async fn deny_tool_call(
        &self,
        workspace_id: Option<&str>,
        actor_id: Option<&str>,
        tool_call_id: &str,
        reason: &str,
    ) -> Result<CommandResponse> {
        self.dispatch(
            workspace_id,
            actor_id,
            "deny_tool_call",
            serde_json::json!({ "tool_call_id": tool_call_id, "reason": reason }),
            None,
        )
        .await
    }

    /// Record the result of an executed tool call.
    pub async fn record_tool_result(
        &self,
        workspace_id: Option<&str>,
        actor_id: Option<&str>,
        tool_call_id: &str,
        result: Value,
    ) -> Result<CommandResponse> {
        self.dispatch(
            workspace_id,
            actor_id,
            "record_tool_result",
            serde_json::json!({ "tool_call_id": tool_call_id, "result": result }),
            None,
        )
        .await
    }

    /// Propose a new memory candidate.
    #[allow(clippy::too_many_arguments)]
    pub async fn propose_memory(
        &self,
        workspace_id: Option<&str>,
        actor_id: Option<&str>,
        text: &str,
        category: &str,
        sensitivity: Sensitivity,
        confidence: f64,
        evidence: Value,
    ) -> Result<CommandResponse> {
        let sens_wire = serde_json::to_value(sensitivity)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "low".to_string());
        self.dispatch(
            workspace_id,
            actor_id,
            "propose_memory",
            serde_json::json!({
                "text": text,
                "category": category,
                "sensitivity": sens_wire,
                "confidence": confidence,
                "evidence": evidence,
            }),
            None,
        )
        .await
    }

    /// Approve a memory candidate.
    pub async fn approve_memory(
        &self,
        workspace_id: Option<&str>,
        actor_id: Option<&str>,
        candidate_id: &str,
    ) -> Result<CommandResponse> {
        self.dispatch(
            workspace_id,
            actor_id,
            "approve_memory",
            serde_json::json!({ "candidate_id": candidate_id }),
            None,
        )
        .await
    }

    /// Reject a memory candidate, with an optional reason.
    pub async fn reject_memory(
        &self,
        workspace_id: Option<&str>,
        actor_id: Option<&str>,
        candidate_id: &str,
        reason: Option<&str>,
    ) -> Result<CommandResponse> {
        let mut input = serde_json::Map::new();
        input.insert(
            "candidate_id".into(),
            Value::String(candidate_id.to_string()),
        );
        if let Some(r) = reason {
            input.insert("reason".into(), Value::String(r.to_string()));
        }
        self.dispatch(
            workspace_id,
            actor_id,
            "reject_memory",
            Value::Object(input),
            None,
        )
        .await
    }

    // -----------------------------------------------------------------
    // Queries

    /// `GET /v1/events?session_id=...` — Chronicle events for a session.
    pub async fn events(&self, session_id: &str) -> Result<Vec<AgentEvent>> {
        let resp: EventsResponse = self
            .request_json(
                Method::GET,
                "/v1/events",
                &[("session_id", session_id)],
                None::<&()>,
            )
            .await?;
        Ok(resp.events)
    }

    /// `GET /v1/approvals?workspace_id=...` — pending approvals.
    pub async fn approvals(&self, workspace_id: Option<&str>) -> Result<Vec<PendingApproval>> {
        let ws = self.resolve_workspace_id(workspace_id)?;
        let resp: ApprovalsResponse = self
            .request_json(
                Method::GET,
                "/v1/approvals",
                &[("workspace_id", ws)],
                None::<&()>,
            )
            .await?;
        Ok(resp.approvals)
    }

    /// `GET /v1/memories?workspace_id=...&status=...`.
    pub async fn memories(
        &self,
        workspace_id: Option<&str>,
        status: &str,
    ) -> Result<Vec<MemoryRow>> {
        #[derive(serde::Deserialize)]
        struct Wrap {
            #[serde(default)]
            memories: Vec<Value>,
        }
        let ws = self.resolve_workspace_id(workspace_id)?;
        let r: Wrap = self
            .request_json(
                Method::GET,
                "/v1/memories",
                &[("workspace_id", ws), ("status", status)],
                None::<&()>,
            )
            .await?;
        Ok(r.memories)
    }

    // -----------------------------------------------------------------
    // Replay

    /// `POST /v1/replay/checkpoint` — capture a checkpoint anchored to an
    /// event id.
    pub async fn replay_checkpoint(
        &self,
        workspace_id: Option<&str>,
        event_id: &str,
    ) -> Result<String> {
        let ws = self.resolve_workspace_id(workspace_id)?.to_string();
        let body = serde_json::json!({
            "workspace_id": ws,
            "event_id": event_id,
        });
        let resp: ReplayCheckpointResponse = self
            .request_json(Method::POST, "/v1/replay/checkpoint", &[], Some(&body))
            .await?;
        Ok(resp.checkpoint_id)
    }

    /// `POST /v1/replay/run` — run a replay against a checkpoint.
    pub async fn replay_run(
        &self,
        actor_id: Option<&str>,
        checkpoint_id: &str,
        mode: ReplayMode,
    ) -> Result<ReplayDiff> {
        let actor = self.resolve_actor_id(actor_id)?.to_string();
        let body = serde_json::json!({
            "actor_id": actor,
            "checkpoint_id": checkpoint_id,
            "mode": mode.as_str(),
        });
        self.request_json(Method::POST, "/v1/replay/run", &[], Some(&body))
            .await
    }

    // -----------------------------------------------------------------
    // Cluster sync

    /// `POST /v1/sync/since` — pull events strictly after `since_event_id`.
    /// Pass an empty string for the first call.
    pub async fn sync_since(
        &self,
        workspace_id: Option<&str>,
        since_event_id: &str,
        limit: u32,
    ) -> Result<SyncSinceResponse> {
        let ws = self.resolve_workspace_id(workspace_id)?.to_string();
        let body = serde_json::json!({
            "workspace_id": ws,
            "since_event_id": since_event_id,
            "limit": limit,
        });
        self.request_json(Method::POST, "/v1/sync/since", &[], Some(&body))
            .await
    }

    // -----------------------------------------------------------------
    // WebSocket subscribe — implemented in subscribe.rs to keep this file
    // focused on the HTTP surface.

    // -----------------------------------------------------------------
    // Internals — accessed from subscribe.rs.

    pub(crate) fn token_ref(&self) -> Option<&str> {
        self.inner.token.as_deref()
    }

    pub(crate) fn join_url(&self, path: &str, query: &[(&str, &str)]) -> Result<Url> {
        let mut url = self
            .inner
            .base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| ActantError::InvalidUrl(e.to_string()))?;
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in query {
                pairs.append_pair(k, v);
            }
        }
        Ok(url)
    }

    fn resolve_workspace_id<'a>(&'a self, override_: Option<&'a str>) -> Result<&'a str> {
        override_
            .or_else(|| self.default_workspace_id())
            .ok_or_else(|| ActantError::InvalidInput {
                message: "workspace_id required (set via with_workspace_id or pass Some(\"...\"))"
                    .into(),
                body: Vec::new(),
            })
    }

    fn resolve_actor_id<'a>(&'a self, override_: Option<&'a str>) -> Result<&'a str> {
        override_
            .or_else(|| self.default_actor_id())
            .ok_or_else(|| ActantError::InvalidInput {
                message: "actor_id required (set via with_actor_id or pass Some(\"...\"))".into(),
                body: Vec::new(),
            })
    }

    async fn send<B: Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&B>,
    ) -> Result<Response> {
        let url = self.join_url(path, query)?;
        let mut req: RequestBuilder = self.inner.http.request(method, url.clone());
        req = req
            .header("accept", "application/json")
            .header("content-type", "application/json");
        if let Some(tok) = self.inner.token.as_deref() {
            req = req.bearer_auth(tok);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        req.send()
            .await
            .map_err(|e| ActantError::Transport(e.to_string()))
    }

    async fn request_json<T, B>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&B>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let resp = self.send(method, path, query, body).await?;
        let status = resp.status().as_u16();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ActantError::Transport(e.to_string()))?;
        let body_vec = bytes.to_vec();

        // The server reuses `{"error":"<kind>","message":"..."}` for some 2xx
        // codes (202 approval_required, 200 idempotent_replay), so peek at
        // the body's `error` field regardless of HTTP status. Matches the
        // Swift SDK's `rawRequest` path.
        if has_error_field(&body_vec) {
            return Err(ActantError::from_response(status, body_vec));
        }
        if !(200..300).contains(&status) {
            return Err(ActantError::from_response(status, body_vec));
        }
        if body_vec.is_empty() {
            // Some endpoints return `{}` with content-type json. Try parsing
            // an empty JSON value if the target type supports it.
            return serde_json::from_slice(b"null").map_err(|e| ActantError::Decoding {
                message: e.to_string(),
                body: body_vec,
            });
        }
        serde_json::from_slice(&body_vec).map_err(|e| ActantError::Decoding {
            message: e.to_string(),
            body: body_vec,
        })
    }
}

/// Cheap peek — returns true when the body parses as a JSON object containing
/// a non-null `error` field. Used to detect 2xx-with-error-body wire shapes.
fn has_error_field(body: &[u8]) -> bool {
    #[derive(serde::Deserialize)]
    struct Peek {
        #[serde(default)]
        error: Option<Value>,
    }
    matches!(
        serde_json::from_slice::<Peek>(body),
        Ok(Peek { error: Some(v) }) if !v.is_null()
    )
}

#[allow(dead_code)]
fn _assert_contract_imports(_: &ApprovalDecision, _: &ApprovalRequest) {}
