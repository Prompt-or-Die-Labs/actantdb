//! actant-ffi — embeddable ActantDB surface exposed via uniffi-rs.
//!
//! Closes GAPS row #39 (substrate side of the iOS embedded-mode plan in
//! `docs/IOS_EMBEDDING.md` §1). The same `cdylib`/`staticlib` produced by
//! this crate becomes the slice content of the iOS XCFramework that GAPS
//! row #41 ships, and feeds the Swift `Actant.embedded(storeDir:)` mode
//! that GAPS row #46 wires up on top.
//!
//! ## What's exported across the FFI boundary
//!
//! - [`ActantHandle`] — opaque object wrapping a SQLite [`Storage`] +
//!   [`Engine`] + caller-supplied workspace / actor identity.
//! - [`CommandOutcome`] — JSON-flat mirror of [`actant_command::CommandOutcome`].
//! - [`EventRow`] — JSON-flat mirror of [`actant_core::AgentEvent`] with the
//!   replication-relevant columns hoisted (`device_id`, `hlc_*`).
//! - [`IngestReport`] — accepted / skipped / rejected counts from a batch
//!   ingest.
//! - [`FfiError`] — flat, FFI-safe enum mapped from [`ActantError`].
//!
//! ## Async story
//!
//! All methods are `async`. The uniffi `tokio` feature is enabled at the
//! workspace level so the host (Swift / Kotlin) drives the Rust futures on
//! a uniffi-managed tokio runtime — no `pollster` or hand-rolled bridges
//! required.
//!
//! ## Open dependencies (in flight in other agents)
//!
//! - GAPS row #43 — `Storage::ingest_events()` backs [`ActantHandle::ingest`].
//!   The FFI event shape intentionally stays flat and uses the handle-bound
//!   workspace / actor for inbound rows.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::sync::Arc;

use actant_command::Engine;
use actant_core::{now_rfc3339, ActantError, Actor, ActorId, ActorKind, Workspace, WorkspaceId};
use actant_storage::{IngestEvent, Storage, StorageConfig};

uniffi::setup_scaffolding!();

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

/// Result of dispatching a command across the FFI boundary.
///
/// `result_json` is the canonical JSON form of
/// [`actant_command::CommandOutcome::result`] so the consumer can decode it
/// in whatever Swift / Kotlin type they want without us baking a schema in
/// here.
#[derive(uniffi::Record, Debug, Clone)]
pub struct CommandOutcome {
    /// Command record id assigned by the engine.
    pub command_id: String,
    /// Chronicle event id, when the command produced one (most do; idempotent
    /// replays do not).
    pub event_id: Option<String>,
    /// Canonical JSON-encoded `serde_json::Value` returned by the engine.
    pub result_json: String,
}

/// One row from the Chronicle, hoisted to a flat FFI-safe shape.
///
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct EventRow {
    /// Stable event id (today: ULID; post-#42: HLC + sha256 content hash).
    pub id: String,
    /// Session id this event belongs to, when applicable.
    pub session_id: Option<String>,
    /// Free-form event type discriminator (`session_created`, `tool_call_*`, …).
    pub event_type: String,
    /// Canonical JSON payload (whatever was in `payload_inline`).
    pub payload_json: String,
    /// SHA-256 of `payload_json`.
    pub payload_hash: String,
    /// RFC-3339 wall-clock stamp recorded at append time.
    pub created_at: String,
    /// Originating device.
    pub device_id: String,
    /// HLC physical component (ms).
    pub hlc_physical_ms: u64,
    /// HLC logical component.
    pub hlc_logical: u32,
}

/// Outcome of a batch ingest.
#[derive(uniffi::Record, Debug, Clone)]
pub struct IngestReport {
    /// Events newly written.
    pub accepted: u32,
    /// Events that were already present (id collision; idempotent skip).
    pub skipped: u32,
    /// Per-event rejection reasons. Empty when everything was either
    /// accepted or skipped.
    pub rejected: Vec<String>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// FFI-safe flattening of [`ActantError`].
///
/// Several `ActantError` variants collapse into the same FFI variant on
/// purpose — Swift / Kotlin consumers don't need every internal distinction
/// the Rust side draws. The mapping table:
///
/// | `ActantError`           | `FfiError`           |
/// |-------------------------|----------------------|
/// | `Storage`               | `Storage`            |
/// | `Internal`              | `Storage`            |
/// | `NotImplemented`        | `Storage`            |
/// | `NotFound`              | `NotFound`           |
/// | `InvalidInput`          | `InvalidInput`       |
/// | `Conflict`              | `InvalidInput`       |
/// | `PermissionDenied`      | `PermissionDenied`   |
/// | `PolicyHalt`            | `PermissionDenied`   |
/// | `ApprovalDenied`        | `PermissionDenied`   |
/// | `ApprovalRequired`      | `ApprovalRequired`   |
/// | `IdempotentReplay`      | `IdempotentReplay`   |
///
/// `RateLimited` has no current source in `ActantError`; it's reserved for
/// future throttle-driven failures wired through [`actant-reliability`].
#[derive(uniffi::Error, thiserror::Error, Debug, Clone)]
#[uniffi(flat_error)]
pub enum FfiError {
    /// Underlying storage / I/O failure.
    #[error("storage error: {0}")]
    Storage(String),
    /// The named resource doesn't exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// Caller-supplied input failed validation or a conflict check.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Policy refused the action outright.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    /// Policy requires a human approval before the action can proceed.
    #[error("approval required")]
    ApprovalRequired,
    /// Caller is being throttled (reserved; not produced today).
    #[error("rate limited")]
    RateLimited,
    /// Idempotency key matched a prior command — see the prior outcome.
    #[error("idempotent replay")]
    IdempotentReplay,
}

impl From<ActantError> for FfiError {
    fn from(e: ActantError) -> Self {
        match e {
            ActantError::Storage(s) => FfiError::Storage(s),
            ActantError::Internal(s) => FfiError::Storage(s),
            ActantError::NotImplemented(s) => FfiError::Storage(s),
            ActantError::NotFound(s) => FfiError::NotFound(s),
            ActantError::InvalidInput(s) => FfiError::InvalidInput(s),
            ActantError::Conflict(s) => FfiError::InvalidInput(s),
            ActantError::PermissionDenied(s) => FfiError::PermissionDenied(s),
            ActantError::PolicyHalt(s) => FfiError::PermissionDenied(s),
            ActantError::ApprovalDenied(s) => FfiError::PermissionDenied(s),
            ActantError::ApprovalRequired(_) => FfiError::ApprovalRequired,
            ActantError::IdempotentReplay(_) => FfiError::IdempotentReplay,
        }
    }
}

type FfiResult<T> = Result<T, FfiError>;

// ---------------------------------------------------------------------------
// Handle
// ---------------------------------------------------------------------------

/// Opaque, reference-counted ActantDB handle exposed to Swift / Kotlin.
///
/// Wraps the SQLite-backed [`Storage`] + the command [`Engine`] + the
/// caller-supplied workspace / actor identity so every `dispatch()` call
/// over the FFI boundary doesn't have to re-thread those args.
///
/// The struct itself is not `Clone`; uniffi hands out `Arc<Self>` to the
/// host language and keeps lifetime correct without explicit close calls
/// (though [`Self::close`] is exposed for consumers that want to release
/// the underlying connection pool eagerly).
#[derive(uniffi::Object)]
pub struct ActantHandle {
    storage: Storage,
    engine: Engine,
    workspace: WorkspaceId,
    actor: ActorId,
}

#[uniffi::export(async_runtime = "tokio")]
impl ActantHandle {
    /// Open (or create) an ActantDB at `store_dir`, applying every bundled
    /// migration, and bind the handle to `workspace_id` + `actor_id`.
    ///
    /// `store_dir` is treated as a directory; the SQLite file lives at
    /// `<store_dir>/actantdb.sqlite`. iOS callers should pass the
    /// app-sandbox Application Support directory.
    #[uniffi::constructor]
    pub async fn open(
        store_dir: String,
        workspace_id: String,
        actor_id: String,
    ) -> FfiResult<Arc<Self>> {
        if workspace_id.is_empty() {
            return Err(FfiError::InvalidInput(
                "workspace_id must not be empty".into(),
            ));
        }
        if actor_id.is_empty() {
            return Err(FfiError::InvalidInput("actor_id must not be empty".into()));
        }
        let cfg = if store_dir.is_empty() || store_dir == ":memory:" {
            StorageConfig::in_memory()
        } else {
            // Make sure the directory exists; if the caller passed a
            // file path, treat it as a literal db file.
            let path = std::path::Path::new(&store_dir);
            let db_path = if path.extension().map(|e| e == "sqlite").unwrap_or(false) {
                path.to_path_buf()
            } else {
                std::fs::create_dir_all(path)
                    .map_err(|e| FfiError::Storage(format!("create store_dir: {e}")))?;
                path.join("actantdb.sqlite")
            };
            StorageConfig::file(db_path)
        };

        let storage = Storage::open(cfg).await?;
        let ws_id = WorkspaceId::from_string(workspace_id);
        // Auto-bootstrap the workspace so first-time consumers (Swoosh, an
        // example iOS app) don't have to know about an explicit setup step
        // before the first dispatch. Mirrors the actor auto-bootstrap that
        // `Engine::dispatch` already does. Idempotent — if the row exists
        // we leave it alone.
        if storage.get_workspace(&ws_id).await?.is_none() {
            storage
                .insert_workspace(&Workspace {
                    id: ws_id.clone(),
                    name: ws_id.as_str().to_string(),
                    created_at: now_rfc3339(),
                    archived_at: None,
                })
                .await?;
        }
        let actor = ActorId::from_string(actor_id);
        if storage.get_actor(&actor).await?.is_none() {
            storage
                .insert_actor(&Actor {
                    id: actor.clone(),
                    workspace_id: ws_id.clone(),
                    kind: ActorKind::Human,
                    display_name: actor.as_str().to_string(),
                    created_at: now_rfc3339(),
                    disabled_at: None,
                })
                .await?;
        }
        let engine = Engine::new(storage.clone());
        Ok(Arc::new(Self {
            storage,
            engine,
            workspace: ws_id,
            actor,
        }))
    }

    /// Dispatch a command. `input_json` must parse as a JSON object.
    ///
    /// Mirrors [`actant_command::Engine::dispatch`] one-to-one; the only
    /// shape change is `result` being re-encoded as a JSON string so the
    /// FFI ABI stays flat.
    pub async fn dispatch(
        &self,
        command_type: String,
        input_json: String,
        idempotency_key: Option<String>,
    ) -> FfiResult<CommandOutcome> {
        let input: serde_json::Value = serde_json::from_str(&input_json)
            .map_err(|e| FfiError::InvalidInput(format!("input_json: {e}")))?;
        let outcome = self
            .engine
            .dispatch(
                &self.workspace,
                &self.actor,
                &command_type,
                input,
                idempotency_key.as_deref(),
            )
            .await?;
        Ok(CommandOutcome {
            command_id: outcome.command_id.as_str().to_string(),
            event_id: outcome.event_id.as_ref().map(|e| e.as_str().to_string()),
            result_json: serde_json::to_string(&outcome.result)
                .map_err(|e| FfiError::Storage(format!("encode result_json: {e}")))?,
        })
    }

    /// Read events for the bound workspace newer than the supplied HLC
    /// cursor, oldest first.
    ///
    pub async fn events_since(
        &self,
        cursor_hlc_physical_ms: Option<u64>,
        cursor_hlc_logical: Option<u32>,
        limit: u32,
    ) -> FfiResult<Vec<EventRow>> {
        let cursor_physical = cursor_hlc_physical_ms.map(|v| v as i64);
        let cursor_logical = cursor_hlc_logical.map(|v| v as i64);
        let limit = limit.clamp(1, 10_000) as i64;
        let pool = self.storage.pool();
        let rows = sqlx::query_as::<
            _,
            (
                String,         // id
                Option<String>, // session_id
                String,         // event_type
                Option<String>, // payload_inline
                String,         // payload_hash
                String,         // created_at
                String,         // device_id
                i64,            // hlc_physical_ms
                i64,            // hlc_logical
            ),
        >(
            "SELECT id, session_id, event_type, payload_inline, payload_hash, created_at,
                    COALESCE(device_id, '_legacy_') AS device_id,
                    COALESCE(hlc_physical_ms, 0) AS hlc_physical_ms,
                    COALESCE(hlc_logical, 0) AS hlc_logical
             FROM agent_event
             WHERE workspace_id = ?
               AND (
                    ? IS NULL OR ? IS NULL
                    OR hlc_physical_ms > ?
                    OR (hlc_physical_ms = ? AND hlc_logical > ?)
               )
             ORDER BY hlc_physical_ms ASC, hlc_logical ASC, created_at ASC, id ASC
             LIMIT ?",
        )
        .bind(self.workspace.as_str())
        .bind(cursor_physical)
        .bind(cursor_logical)
        .bind(cursor_physical)
        .bind(cursor_physical)
        .bind(cursor_logical)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| FfiError::Storage(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    session_id,
                    event_type,
                    payload_inline,
                    payload_hash,
                    created_at,
                    device_id,
                    hlc_physical_ms,
                    hlc_logical,
                )| {
                    EventRow {
                        id,
                        session_id,
                        event_type,
                        payload_json: payload_inline.unwrap_or_else(|| "{}".to_string()),
                        payload_hash,
                        created_at,
                        device_id,
                        hlc_physical_ms: hlc_physical_ms.max(0) as u64,
                        hlc_logical: hlc_logical.max(0) as u32,
                    }
                },
            )
            .collect())
    }

    /// Batch-ingest events from a peer.
    pub async fn ingest(&self, events: Vec<EventRow>) -> FfiResult<IngestReport> {
        let batch = events
            .into_iter()
            .map(|row| {
                let canonical_payload = row.payload_json.clone().into_bytes();
                let payload_hash = row.payload_hash.clone();
                IngestEvent {
                    event: actant_core::AgentEvent {
                        id: actant_core::EventId::from_string(row.id),
                        workspace_id: self.workspace.clone(),
                        actor_id: self.actor.clone(),
                        session_id: row.session_id.map(actant_core::SessionId::from_string),
                        parent_event_id: None,
                        event_type: row.event_type,
                        causality_kind: actant_core::CausalityKind::Audit,
                        sensitivity: actant_core::Sensitivity::Low,
                        authority_scope_id: None,
                        payload_inline: Some(row.payload_json),
                        payload_ref: None,
                        payload_hash: row.payload_hash,
                        event_hash: payload_hash,
                        created_at: row.created_at,
                        model_call_id: None,
                        tool_call_id: None,
                        workflow_run_id: None,
                        memory_id: None,
                        artifact_id: None,
                        command_id: None,
                        effect_id: None,
                    },
                    hlc: actant_core::Hlc::new(row.hlc_physical_ms, row.hlc_logical),
                    device_id: row.device_id,
                    canonical_payload,
                }
            })
            .collect::<Vec<_>>();
        let report = self.storage.ingest_events(&batch, None).await?;
        Ok(IngestReport {
            accepted: report.accepted,
            skipped: report.skipped,
            rejected: report
                .rejected
                .into_iter()
                .map(|r| match r.event_id {
                    Some(event_id) => format!("{}:{}:{event_id}", r.index, r.reason),
                    None => format!("{}:{}", r.index, r.reason),
                })
                .collect(),
        })
    }

    /// Release the underlying connection pool eagerly.
    ///
    /// uniffi will also drop the handle when the host releases its last
    /// reference; calling `close()` is only necessary when the consumer
    /// wants the SQLite file unlocked before the wrapper falls out of
    /// scope (e.g. before deleting it from disk).
    pub async fn close(&self) {
        self.storage.pool().close().await;
    }
}
