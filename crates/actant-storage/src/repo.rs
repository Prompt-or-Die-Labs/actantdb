//! Lightweight repositories for the canonical tables. These wrap raw SQL so
//! the higher-level crates don't reach for `sqlx` directly.

use actant_core::*;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{blob_sha256_hex, Storage};

/// A replicated event ready to be ingested by [`Storage::ingest_events`].
///
/// Wraps an [`AgentEvent`] (the existing on-disk row shape) with the
/// replication-only fields stored in the columns added by migration
/// `0007_replication.sql`.
///
/// New code (the FFI sync path, in-process publishers) should produce
/// these by:
///
/// 1. Allocating an HLC via [`HlcClock::local_tick`].
/// 2. Building the canonical-JSON payload that ends up in `payload_inline`.
/// 3. Computing the event id via [`EventId::content_derived`].
/// 4. Filling the rest of the `AgentEvent` row.
///
/// Ingest will re-derive the id from the supplied payload+hlc+actor and
/// reject rows whose recomputed id doesn't match — the only way to land
/// in the ledger is to have produced the event through the canonical
/// flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestEvent {
    /// The agent_event row to insert.
    pub event: AgentEvent,
    /// HLC stamp produced when the event was created.
    pub hlc: Hlc,
    /// Producing device id (must be non-empty; pass a stable per-device id).
    pub device_id: String,
    /// Canonical-JSON serialization of the payload as used when deriving
    /// the event id. Ingest recomputes
    /// `sha256(canonical_payload || hlc.physical_ms || hlc.logical || actor_id)`
    /// and rejects on mismatch with `event.id`.
    pub canonical_payload: Vec<u8>,
}

/// Per-row rejection reason produced by [`Storage::ingest_events`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestReject {
    /// Position of the rejected row in the input batch.
    pub index: usize,
    /// Event id (if it could be extracted).
    pub event_id: Option<String>,
    /// One of `"hash_mismatch"` | `"missing_fields"` | `"workspace_unknown"`.
    pub reason: String,
}

/// Summary of an [`Storage::ingest_events`] call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IngestReport {
    /// Rows that were newly inserted.
    pub accepted: u32,
    /// Rows that were skipped because the primary key already existed.
    pub skipped: u32,
    /// Rows that failed validation; see [`IngestReject::reason`].
    pub rejected: Vec<IngestReject>,
}

impl Storage {
    /// Insert a workspace.
    pub async fn insert_workspace(&self, ws: &Workspace) -> Result<(), ActantError> {
        sqlx::query("INSERT INTO workspace (id, name, created_at) VALUES (?, ?, ?)")
            .bind(ws.id.as_str())
            .bind(&ws.name)
            .bind(&ws.created_at)
            .execute(&self.pool)
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Fetch a workspace by id.
    pub async fn get_workspace(&self, id: &WorkspaceId) -> Result<Option<Workspace>, ActantError> {
        let row =
            sqlx::query("SELECT id, name, created_at, archived_at FROM workspace WHERE id = ?")
                .bind(id.as_str())
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(row.map(|r| Workspace {
            id: WorkspaceId::from_string(r.get::<String, _>("id")),
            name: r.get("name"),
            created_at: r.get("created_at"),
            archived_at: r.get("archived_at"),
        }))
    }

    /// Insert an actor.
    pub async fn insert_actor(&self, a: &Actor) -> Result<(), ActantError> {
        sqlx::query(
            "INSERT INTO actor (id, workspace_id, kind, display_name, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(a.id.as_str())
        .bind(a.workspace_id.as_str())
        .bind(
            serde_json::to_string(&a.kind)
                .unwrap()
                .trim_matches('"')
                .to_string(),
        )
        .bind(&a.display_name)
        .bind(&a.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Fetch an actor by id.
    pub async fn get_actor(&self, id: &ActorId) -> Result<Option<Actor>, ActantError> {
        let row = sqlx::query(
            "SELECT id, workspace_id, kind, display_name, created_at, disabled_at
             FROM actor WHERE id = ?",
        )
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(row.map(|r| {
            let kind_s: String = r.get("kind");
            let kind: ActorKind = serde_json::from_value(serde_json::Value::String(kind_s))
                .unwrap_or(ActorKind::System);
            Actor {
                id: ActorId::from_string(r.get::<String, _>("id")),
                workspace_id: WorkspaceId::from_string(r.get::<String, _>("workspace_id")),
                kind,
                display_name: r.get("display_name"),
                created_at: r.get("created_at"),
                disabled_at: r.get("disabled_at"),
            }
        }))
    }

    /// Insert a session.
    pub async fn insert_session(&self, s: &Session) -> Result<(), ActantError> {
        sqlx::query(
            "INSERT INTO session
                (id, workspace_id, title, initiator_actor_id, agent_actor_id, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(s.id.as_str())
        .bind(s.workspace_id.as_str())
        .bind(&s.title)
        .bind(s.initiator_actor_id.as_str())
        .bind(s.agent_actor_id.as_ref().map(|a| a.as_str()))
        .bind(
            serde_json::to_string(&s.status)
                .unwrap()
                .trim_matches('"')
                .to_string(),
        )
        .bind(&s.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Append an event to the Chronicle. Computes the chain hash.
    pub async fn append_event(&self, e: &AgentEvent) -> Result<(), ActantError> {
        sqlx::query(
            "INSERT INTO agent_event
                (id, workspace_id, actor_id, session_id, parent_event_id,
                 event_type, causality_kind, sensitivity, authority_scope_id,
                 payload_ref, payload_inline, payload_hash,
                 model_call_id, tool_call_id, workflow_run_id, memory_id,
                 artifact_id, command_id, effect_id, event_hash, created_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(e.id.as_str())
        .bind(e.workspace_id.as_str())
        .bind(e.actor_id.as_str())
        .bind(e.session_id.as_ref().map(|s| s.as_str()))
        .bind(e.parent_event_id.as_ref().map(|s| s.as_str()))
        .bind(&e.event_type)
        .bind(json_enum(&e.causality_kind))
        .bind(json_enum(&e.sensitivity))
        .bind(e.authority_scope_id.as_ref().map(|s| s.as_str()))
        .bind(&e.payload_ref)
        .bind(&e.payload_inline)
        .bind(&e.payload_hash)
        .bind(e.model_call_id.as_ref().map(|s| s.as_str()))
        .bind(e.tool_call_id.as_ref().map(|s| s.as_str()))
        .bind(e.workflow_run_id.as_ref().map(|s| s.as_str()))
        .bind(e.memory_id.as_ref().map(|s| s.as_str()))
        .bind(e.artifact_id.as_ref().map(|s| s.as_str()))
        .bind(e.command_id.as_ref().map(|s| s.as_str()))
        .bind(e.effect_id.as_ref().map(|s| s.as_str()))
        .bind(&e.event_hash)
        .bind(&e.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Idempotently ingest a batch of replicated events.
    ///
    /// Validates each row:
    ///
    /// 1. Recompute `event.id` from
    ///    `sha256(canonical_payload || hlc.physical_ms || hlc.logical || actor_id)`.
    ///    Reject on mismatch (`"hash_mismatch"`).
    /// 2. Refuse rows whose `workspace_id` doesn't exist (`"workspace_unknown"`)
    ///    or whose `device_id` is empty (`"missing_fields"`).
    /// 3. `INSERT INTO agent_event … ON CONFLICT(id) DO NOTHING` — already
    ///    present rows count as `skipped`, not failures.
    ///
    /// After processing all rows, observes the highest valid HLC against
    /// `clock` if one is supplied so that subsequent `local_tick`s never go
    /// backwards. Callers that don't care about HLC continuity can pass
    /// `None`.
    pub async fn ingest_events(
        &self,
        batch: &[IngestEvent],
        clock: Option<&HlcClock>,
    ) -> Result<IngestReport, ActantError> {
        let mut report = IngestReport::default();
        let mut max_hlc: Option<Hlc> = None;

        for (index, ie) in batch.iter().enumerate() {
            // Step 1+2: validate.
            if ie.device_id.is_empty() {
                report.rejected.push(IngestReject {
                    index,
                    event_id: Some(ie.event.id.as_str().to_string()),
                    reason: "missing_fields".into(),
                });
                continue;
            }
            let recomputed =
                EventId::content_derived(&ie.canonical_payload, ie.hlc, &ie.event.actor_id);
            if recomputed.as_str() != ie.event.id.as_str() {
                report.rejected.push(IngestReject {
                    index,
                    event_id: Some(ie.event.id.as_str().to_string()),
                    reason: "hash_mismatch".into(),
                });
                continue;
            }

            // Workspace presence check.
            let ws_present: Option<(String,)> =
                sqlx::query_as("SELECT id FROM workspace WHERE id = ?")
                    .bind(ie.event.workspace_id.as_str())
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| ActantError::Storage(e.to_string()))?;
            if ws_present.is_none() {
                report.rejected.push(IngestReject {
                    index,
                    event_id: Some(ie.event.id.as_str().to_string()),
                    reason: "workspace_unknown".into(),
                });
                continue;
            }

            // Step 3: insert with conflict-do-nothing on the primary key.
            let e = &ie.event;
            let res = sqlx::query(
                "INSERT INTO agent_event
                    (id, workspace_id, actor_id, session_id, parent_event_id,
                     event_type, causality_kind, sensitivity, authority_scope_id,
                     payload_ref, payload_inline, payload_hash,
                     model_call_id, tool_call_id, workflow_run_id, memory_id,
                     artifact_id, command_id, effect_id, event_hash, created_at,
                     device_id, hlc_physical_ms, hlc_logical)
                 VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
                 ON CONFLICT(id) DO NOTHING",
            )
            .bind(e.id.as_str())
            .bind(e.workspace_id.as_str())
            .bind(e.actor_id.as_str())
            .bind(e.session_id.as_ref().map(|s| s.as_str()))
            .bind(e.parent_event_id.as_ref().map(|s| s.as_str()))
            .bind(&e.event_type)
            .bind(json_enum(&e.causality_kind))
            .bind(json_enum(&e.sensitivity))
            .bind(e.authority_scope_id.as_ref().map(|s| s.as_str()))
            .bind(&e.payload_ref)
            .bind(&e.payload_inline)
            .bind(&e.payload_hash)
            .bind(e.model_call_id.as_ref().map(|s| s.as_str()))
            .bind(e.tool_call_id.as_ref().map(|s| s.as_str()))
            .bind(e.workflow_run_id.as_ref().map(|s| s.as_str()))
            .bind(e.memory_id.as_ref().map(|s| s.as_str()))
            .bind(e.artifact_id.as_ref().map(|s| s.as_str()))
            .bind(e.command_id.as_ref().map(|s| s.as_str()))
            .bind(e.effect_id.as_ref().map(|s| s.as_str()))
            .bind(&e.event_hash)
            .bind(&e.created_at)
            .bind(&ie.device_id)
            .bind(ie.hlc.physical_ms as i64)
            .bind(ie.hlc.logical as i64)
            .execute(&self.pool)
            .await
            .map_err(|err| ActantError::Storage(err.to_string()))?;

            if res.rows_affected() == 0 {
                report.skipped = report.skipped.saturating_add(1);
            } else {
                report.accepted = report.accepted.saturating_add(1);
            }

            // Track max-seen HLC for the post-batch observe.
            max_hlc = Some(match max_hlc {
                Some(prev) if prev >= ie.hlc => prev,
                _ => ie.hlc,
            });
        }

        if let (Some(c), Some(h)) = (clock, max_hlc) {
            c.observe(h);
        }
        Ok(report)
    }

    /// Last event hash within a session (for chaining).
    pub async fn last_event_hash(
        &self,
        workspace_id: &WorkspaceId,
        session_id: Option<&SessionId>,
    ) -> Result<Option<String>, ActantError> {
        let row = if let Some(s) = session_id {
            sqlx::query(
                "SELECT event_hash FROM agent_event
                 WHERE workspace_id = ? AND session_id = ?
                 ORDER BY created_at DESC, id DESC LIMIT 1",
            )
            .bind(workspace_id.as_str())
            .bind(s.as_str())
            .fetch_optional(&self.pool)
            .await
        } else {
            sqlx::query(
                "SELECT event_hash FROM agent_event
                 WHERE workspace_id = ?
                 ORDER BY created_at DESC, id DESC LIMIT 1",
            )
            .bind(workspace_id.as_str())
            .fetch_optional(&self.pool)
            .await
        }
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(row.map(|r| r.get("event_hash")))
    }

    /// Insert a command record.
    pub async fn insert_command(&self, c: &CommandRecord) -> Result<(), ActantError> {
        sqlx::query(
            "INSERT INTO command_record
                (id, workspace_id, actor_id, session_id, command_type,
                 input_inline, input_hash, policy_id, status, error,
                 created_at, committed_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(c.id.as_str())
        .bind(c.workspace_id.as_str())
        .bind(c.actor_id.as_str())
        .bind(c.session_id.as_ref().map(|s| s.as_str()))
        .bind(&c.command_type)
        .bind(&c.input_inline)
        .bind(&c.input_hash)
        .bind(c.policy_id.as_ref().map(|s| s.as_str()))
        .bind(json_enum(&c.status))
        .bind(&c.error)
        .bind(&c.created_at)
        .bind(&c.committed_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Look up an idempotency record (returns the stored result_ref if any).
    pub async fn idempotency_lookup(
        &self,
        workspace_id: &WorkspaceId,
        key: &str,
    ) -> Result<Option<String>, ActantError> {
        let row = sqlx::query(
            "SELECT result_ref FROM idempotency_record
             WHERE workspace_id = ? AND idempotency_key = ?",
        )
        .bind(workspace_id.as_str())
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(row.and_then(|r| r.get::<Option<String>, _>("result_ref")))
    }

    /// Record an idempotency key against a command.
    pub async fn idempotency_record(
        &self,
        workspace_id: &WorkspaceId,
        actor_id: &ActorId,
        key: &str,
        command_type: &str,
        input_hash: &str,
        result_ref: Option<&str>,
    ) -> Result<(), ActantError> {
        sqlx::query(
            "INSERT OR IGNORE INTO idempotency_record
                (workspace_id, idempotency_key, actor_id, command_type,
                 input_hash, result_ref, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(workspace_id.as_str())
        .bind(key)
        .bind(actor_id.as_str())
        .bind(command_type)
        .bind(input_hash)
        .bind(result_ref)
        .bind(now_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Store an artifact body in the configured [`crate::BlobStore`] and insert the
    /// metadata row. Returns the new [`ArtifactId`].
    ///
    /// The key handed to the blob store is the lowercase-hex SHA-256 of `body`,
    /// which doubles as the content hash recorded in `artifact.content_hash`.
    /// Calling [`Self::put_artifact`] with identical bytes is therefore
    /// idempotent at the blob level: a subsequent caller writes the same key
    /// and the underlying object is identical. Two `artifact` rows are still
    /// inserted because the artifact table is the metadata projection — the
    /// rows differ in `id`, `workspace_id`, `created_by_actor_id`, and
    /// `created_at`.
    ///
    /// The `kind` value lands in `artifact.kind` verbatim — callers should use
    /// the canonical strings documented on the table (`'screenshot'`, `'file'`,
    /// `'tool_output'`, `'transcript'`, `'report'`, `'model_output'`,
    /// `'audio'`, `'video'`).
    pub async fn put_artifact(
        &self,
        workspace_id: &WorkspaceId,
        actor_id: &ActorId,
        kind: &str,
        body: Bytes,
        sensitivity: Sensitivity,
    ) -> Result<ArtifactId, ActantError> {
        let content_hash = blob_sha256_hex(&body);
        let bytes = body.len() as i64;
        let blob_ref = self
            .blob_store
            .put(&content_hash, body)
            .await
            .map_err(|e| ActantError::Storage(format!("blob put: {e}")))?;

        let id = ArtifactId::from_string(format!("art_{}", ulid::Ulid::new()));
        let sens_s = serde_json::to_string(&sensitivity)
            .unwrap_or_else(|_| "\"low\"".into())
            .trim_matches('"')
            .to_string();
        let created_at = now_rfc3339();
        sqlx::query(
            "INSERT INTO artifact
                (id, workspace_id, kind, uri, content_hash, bytes, sensitivity,
                 created_by_actor_id, created_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
        )
        .bind(id.as_str())
        .bind(workspace_id.as_str())
        .bind(kind)
        .bind(&blob_ref.uri)
        .bind(&content_hash)
        .bind(bytes)
        .bind(&sens_s)
        .bind(actor_id.as_str())
        .bind(&created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(id)
    }

    /// Fetch an artifact metadata row by id.
    pub async fn get_artifact_uri(&self, id: &ArtifactId) -> Result<Option<String>, ActantError> {
        let row = sqlx::query("SELECT uri FROM artifact WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(row.map(|r| r.get::<String, _>("uri")))
    }

    /// Query Chronicle events for a session, oldest first.
    pub async fn events_in_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<AgentEvent>, ActantError> {
        let rows = sqlx::query(
            "SELECT id, workspace_id, actor_id, session_id, parent_event_id,
                    event_type, causality_kind, sensitivity, authority_scope_id,
                    payload_ref, payload_inline, payload_hash,
                    model_call_id, tool_call_id, workflow_run_id, memory_id,
                    artifact_id, command_id, effect_id, event_hash, created_at
             FROM agent_event WHERE session_id = ?
             ORDER BY created_at ASC, id ASC",
        )
        .bind(session_id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let causality_s: String = r.get("causality_kind");
            let sensitivity_s: String = r.get("sensitivity");
            out.push(AgentEvent {
                id: EventId::from_string(r.get::<String, _>("id")),
                workspace_id: WorkspaceId::from_string(r.get::<String, _>("workspace_id")),
                actor_id: ActorId::from_string(r.get::<String, _>("actor_id")),
                session_id: r
                    .get::<Option<String>, _>("session_id")
                    .map(SessionId::from_string),
                parent_event_id: r
                    .get::<Option<String>, _>("parent_event_id")
                    .map(EventId::from_string),
                event_type: r.get("event_type"),
                causality_kind: serde_json::from_value(serde_json::Value::String(causality_s))
                    .unwrap_or(CausalityKind::Audit),
                sensitivity: serde_json::from_value(serde_json::Value::String(sensitivity_s))
                    .unwrap_or(Sensitivity::Low),
                authority_scope_id: r
                    .get::<Option<String>, _>("authority_scope_id")
                    .map(AuthorityScopeId::from_string),
                payload_ref: r.get("payload_ref"),
                payload_inline: r.get("payload_inline"),
                payload_hash: r.get("payload_hash"),
                model_call_id: r
                    .get::<Option<String>, _>("model_call_id")
                    .map(ModelCallId::from_string),
                tool_call_id: r
                    .get::<Option<String>, _>("tool_call_id")
                    .map(ToolCallId::from_string),
                workflow_run_id: r
                    .get::<Option<String>, _>("workflow_run_id")
                    .map(WorkflowRunId::from_string),
                memory_id: r
                    .get::<Option<String>, _>("memory_id")
                    .map(MemoryId::from_string),
                artifact_id: r
                    .get::<Option<String>, _>("artifact_id")
                    .map(ArtifactId::from_string),
                command_id: r
                    .get::<Option<String>, _>("command_id")
                    .map(CommandId::from_string),
                effect_id: r
                    .get::<Option<String>, _>("effect_id")
                    .map(EffectId::from_string),
                event_hash: r.get("event_hash"),
                created_at: r.get("created_at"),
            });
        }
        Ok(out)
    }
}

fn json_enum<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v)
        .unwrap_or_else(|_| "\"\"".into())
        .trim_matches('"')
        .to_string()
}
