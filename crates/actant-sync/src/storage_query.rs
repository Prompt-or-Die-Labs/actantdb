//! Cursor-paginated read of `agent_event` rows.
//!
//! `actant-storage` exposes `events_in_session` but not `events_since(cursor)`.
//! Adding that method belongs to a future `actant-storage` PR — this row
//! ([GAPS.md #16]) is intentionally scoped to `actant-sync/` so the query
//! lives here for now. Once `actant-storage` grows the proper API, the body
//! of this module collapses to a one-line delegation.
//!
//! The query is the same shape as `events_in_session`: select the full row
//! and re-hydrate into `AgentEvent`. Pagination uses `(created_at, id)` so
//! pulling 100 events at a time over a long-lived feed is monotonic — even
//! if multiple events share a timestamp the secondary id breaks ties and the
//! next call resumes correctly.
//!
//! [GAPS.md #16]: ../../../GAPS.md

use actant_core::*;
use actant_storage::Storage;
use sqlx::Row;

use crate::SyncError;

/// Fetch events for `workspace`, strictly newer than `since` (by
/// `(created_at, id)`), up to `limit` rows.
pub async fn events_after(
    storage: &Storage,
    workspace: &WorkspaceId,
    since: Option<&EventId>,
    limit: u32,
) -> Result<Vec<AgentEvent>, SyncError> {
    // Look up the cursor's created_at so we can paginate on
    // `(created_at, id) > (cursor.created_at, cursor.id)`. ULIDs sort
    // lexicographically by time but the schema does not require ULIDs, so we
    // also key on created_at to be safe.
    let cursor_created_at: Option<String> = if let Some(id) = since {
        sqlx::query("SELECT created_at FROM agent_event WHERE id = ? AND workspace_id = ?")
            .bind(id.as_str())
            .bind(workspace.as_str())
            .fetch_optional(storage.pool())
            .await
            .map_err(|e| SyncError::Storage(e.to_string()))?
            .map(|r| r.get("created_at"))
    } else {
        None
    };

    let rows = if let (Some(id), Some(created)) = (since, cursor_created_at.as_deref()) {
        sqlx::query(
            "SELECT id, workspace_id, actor_id, session_id, parent_event_id,
                    event_type, causality_kind, sensitivity, authority_scope_id,
                    payload_ref, payload_inline, payload_hash,
                    model_call_id, tool_call_id, workflow_run_id, memory_id,
                    artifact_id, command_id, effect_id, event_hash, created_at
             FROM agent_event
             WHERE workspace_id = ?
               AND (created_at > ? OR (created_at = ? AND id > ?))
             ORDER BY created_at ASC, id ASC
             LIMIT ?",
        )
        .bind(workspace.as_str())
        .bind(created)
        .bind(created)
        .bind(id.as_str())
        .bind(limit as i64)
        .fetch_all(storage.pool())
        .await
    } else {
        sqlx::query(
            "SELECT id, workspace_id, actor_id, session_id, parent_event_id,
                    event_type, causality_kind, sensitivity, authority_scope_id,
                    payload_ref, payload_inline, payload_hash,
                    model_call_id, tool_call_id, workflow_run_id, memory_id,
                    artifact_id, command_id, effect_id, event_hash, created_at
             FROM agent_event
             WHERE workspace_id = ?
             ORDER BY created_at ASC, id ASC
             LIMIT ?",
        )
        .bind(workspace.as_str())
        .bind(limit as i64)
        .fetch_all(storage.pool())
        .await
    }
    .map_err(|e| SyncError::Storage(e.to_string()))?;

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
