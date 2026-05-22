//! Postgres mirror of `repo.rs`. Each method on [`PgStorage`] here matches a
//! method on [`crate::Storage`] in shape and behaviour; the SQL bodies differ
//! only in dialect:
//!
//!   * `?` placeholders -> `$1`, `$2`, ...
//!   * `INSERT OR IGNORE` -> `INSERT ... ON CONFLICT DO NOTHING`
//!
//! Booleans remain stored as `INTEGER 0/1` (matching the SQLite schema), and
//! timestamps remain RFC3339 strings -- keeping types identical means the
//! Rust bindings are identical between the two backends. The schema is
//! ported 1:1 in `/migrations/pg/*.sql`; the parity gate in
//! `.github/workflows/ci.yml` enforces that on every PR.
//!
//! Coverage parity: every public `impl Storage` method that the substrate
//! calls (insert_workspace / get_workspace / insert_actor / get_actor /
//! insert_session / append_event / last_event_hash / insert_command /
//! idempotency_lookup / idempotency_record / put_artifact / get_artifact_uri
//! / events_in_session) has a `PgStorage` counterpart here.

use actant_core::*;
use bytes::Bytes;
use sqlx::Row;

use crate::{blob_sha256_hex, PgStorage};

impl PgStorage {
    /// Insert a workspace.
    pub async fn insert_workspace(&self, ws: &Workspace) -> Result<(), ActantError> {
        sqlx::query("INSERT INTO workspace (id, name, created_at) VALUES ($1, $2, $3)")
            .bind(ws.id.as_str())
            .bind(&ws.name)
            .bind(&ws.created_at)
            .execute(self.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Fetch a workspace by id.
    pub async fn get_workspace(&self, id: &WorkspaceId) -> Result<Option<Workspace>, ActantError> {
        let row =
            sqlx::query("SELECT id, name, created_at, archived_at FROM workspace WHERE id = $1")
                .bind(id.as_str())
                .fetch_optional(self.pool())
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
             VALUES ($1, $2, $3, $4, $5)",
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
        .execute(self.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Fetch an actor by id.
    pub async fn get_actor(&self, id: &ActorId) -> Result<Option<Actor>, ActantError> {
        let row = sqlx::query(
            "SELECT id, workspace_id, kind, display_name, created_at, disabled_at
             FROM actor WHERE id = $1",
        )
        .bind(id.as_str())
        .fetch_optional(self.pool())
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
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
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
        .execute(self.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Append an event to the Chronicle. Caller computes the chain hash.
    pub async fn append_event(&self, e: &AgentEvent) -> Result<(), ActantError> {
        sqlx::query(
            "INSERT INTO agent_event
                (id, workspace_id, actor_id, session_id, parent_event_id,
                 event_type, causality_kind, sensitivity, authority_scope_id,
                 payload_ref, payload_inline, payload_hash,
                 model_call_id, tool_call_id, workflow_run_id, memory_id,
                 artifact_id, command_id, effect_id, event_hash, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)",
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
        .execute(self.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
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
                 WHERE workspace_id = $1 AND session_id = $2
                 ORDER BY created_at DESC, id DESC LIMIT 1",
            )
            .bind(workspace_id.as_str())
            .bind(s.as_str())
            .fetch_optional(self.pool())
            .await
        } else {
            sqlx::query(
                "SELECT event_hash FROM agent_event
                 WHERE workspace_id = $1
                 ORDER BY created_at DESC, id DESC LIMIT 1",
            )
            .bind(workspace_id.as_str())
            .fetch_optional(self.pool())
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
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
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
        .execute(self.pool())
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
             WHERE workspace_id = $1 AND idempotency_key = $2",
        )
        .bind(workspace_id.as_str())
        .bind(key)
        .fetch_optional(self.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(row.and_then(|r| r.get::<Option<String>, _>("result_ref")))
    }

    /// Record an idempotency key against a command.
    ///
    /// Translates `INSERT OR IGNORE` to `ON CONFLICT DO NOTHING` against the
    /// composite primary key `(workspace_id, idempotency_key)`.
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
            "INSERT INTO idempotency_record
                (workspace_id, idempotency_key, actor_id, command_type,
                 input_hash, result_ref, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (workspace_id, idempotency_key) DO NOTHING",
        )
        .bind(workspace_id.as_str())
        .bind(key)
        .bind(actor_id.as_str())
        .bind(command_type)
        .bind(input_hash)
        .bind(result_ref)
        .bind(now_rfc3339())
        .execute(self.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Store an artifact body via the injected [`crate::BlobStore`] and insert the
    /// metadata row. Mirrors [`crate::Storage::put_artifact`].
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
            .blob_store()
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
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL)",
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
        .execute(self.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(id)
    }

    /// Fetch an artifact uri by id.
    pub async fn get_artifact_uri(&self, id: &ArtifactId) -> Result<Option<String>, ActantError> {
        let row = sqlx::query("SELECT uri FROM artifact WHERE id = $1")
            .bind(id.as_str())
            .fetch_optional(self.pool())
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
             FROM agent_event WHERE session_id = $1
             ORDER BY created_at ASC, id ASC",
        )
        .bind(session_id.as_str())
        .fetch_all(self.pool())
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
