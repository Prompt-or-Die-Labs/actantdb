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
//! - GAPS row #42 — HLC clock + `device_id` columns in `agent_event`.
//!   Until those columns land in the SQLite schema, [`ActantHandle::events_since`]
//!   returns zero-valued defaults for `hlc_physical_ms`, `hlc_logical`, and
//!   `"_legacy_"` for `device_id`. The SQL projection is the only thing that
//!   needs to change when the migration lands.
//! - GAPS row #43 — `Storage::ingest_events()` for idempotent batch ingest.
//!   Until that method exists in `actant-storage`, [`ActantHandle::ingest`]
//!   is a clearly-named stub that returns
//!   `FfiError::Storage("pending GAPS #43 …")`. The FFI surface itself is
//!   stable; only the body needs to be re-pointed.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::sync::Arc;

use actant_command::Engine;
use actant_core::{now_rfc3339, ActantError, ActorId, Workspace, WorkspaceId};
use actant_storage::{Storage, StorageConfig};

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
/// Until GAPS row #42 lands `device_id` + HLC columns in the schema, the
/// replication fields default to `"_legacy_"` / `0` / `0`. The SQL
/// projection in [`ActantHandle::events_since`] is the only place that
/// changes when the migration arrives.
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
    /// Originating device. `"_legacy_"` until GAPS #42 lands.
    pub device_id: String,
    /// HLC physical component (ms). `0` until GAPS #42 lands.
    pub hlc_physical_ms: u64,
    /// HLC logical component. `0` until GAPS #42 lands.
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
        let engine = Engine::new(storage.clone());
        Ok(Arc::new(Self {
            storage,
            engine,
            workspace: ws_id,
            actor: ActorId::from_string(actor_id),
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
    /// Until GAPS row #42 lands HLC columns in `agent_event`, the cursor
    /// args are accepted for shape stability but ignored — the query
    /// orders by `created_at, id` and the row-level HLC / device fields
    /// return the documented `"_legacy_"` / `0` defaults.
    pub async fn events_since(
        &self,
        cursor_hlc_physical_ms: Option<u64>,
        cursor_hlc_logical: Option<u32>,
        limit: u32,
    ) -> FfiResult<Vec<EventRow>> {
        let _ = (cursor_hlc_physical_ms, cursor_hlc_logical);
        let limit = limit.clamp(1, 10_000) as i64;
        // Direct sqlx query against the shared pool. Doing it here (instead
        // of through a `Storage::*` repo method) keeps the boundary clean —
        // the GAPS #42 migration will swap the SELECT list for the real HLC
        // columns without churning `actant-storage`'s public API.
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
            ),
        >(
            "SELECT id, session_id, event_type, payload_inline, payload_hash, created_at
             FROM agent_event
             WHERE workspace_id = ?
             ORDER BY created_at ASC, id ASC
             LIMIT ?",
        )
        .bind(self.workspace.as_str())
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| FfiError::Storage(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(
                |(id, session_id, event_type, payload_inline, payload_hash, created_at)| {
                    EventRow {
                        id,
                        session_id,
                        event_type,
                        payload_json: payload_inline.unwrap_or_else(|| "{}".to_string()),
                        payload_hash,
                        created_at,
                        // Pending GAPS #42 — defaults until the migration lands.
                        device_id: "_legacy_".to_string(),
                        hlc_physical_ms: 0,
                        hlc_logical: 0,
                    }
                },
            )
            .collect())
    }

    /// Batch-ingest events from a peer.
    ///
    /// **Stubbed pending GAPS row #43** (`Storage::ingest_events()`). The
    /// FFI surface is final — only the body needs to be re-pointed at the
    /// real storage method when it lands.
    pub async fn ingest(&self, events: Vec<EventRow>) -> FfiResult<IngestReport> {
        // Touch the field so the compiler doesn't warn before #43 is wired up.
        let _ = events.len();
        Err(FfiError::Storage(
            "pending GAPS #43: Storage::ingest_events() not yet implemented; \
             see docs/IOS_EMBEDDING.md §4 for the target semantics"
                .to_string(),
        ))
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
