//! actant-drift — autonomy-drift signal producer.
//!
//! Computes a drift score for a session and writes a `drift_signal` row
//! when the score crosses the workspace's threshold. Phase 6+ surface.
//!
//! v0.1 score components (all in [0, 1]):
//!   - `approval_density` — fraction of tool calls that triggered an approval
//!   - `denial_rate`      — fraction of approvals that were denied
//!   - `error_rate`       — fraction of tool_call_finished rows with status=error
//!
//! Drift score is the average; threshold default = 0.5.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::{now_rfc3339, ActantError, ActorId, SessionId, WorkspaceId};
use actant_storage::Storage;
use serde::{Deserialize, Serialize};

/// Drift score breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftScore {
    /// Composite score in [0, 1].
    pub score: f64,
    /// Approvals per tool call.
    pub approval_density: f64,
    /// Denials per approval.
    pub denial_rate: f64,
    /// Errors per finished tool call.
    pub error_rate: f64,
    /// Total tool calls observed.
    pub tool_calls: u64,
}

impl DriftScore {
    /// Returns true if the score exceeds `threshold`.
    pub fn exceeds(&self, threshold: f64) -> bool {
        self.score > threshold
    }
}

/// Compute the drift score for a session.
pub async fn score_session(
    storage: &Storage,
    session: &SessionId,
) -> Result<DriftScore, ActantError> {
    let rows = storage.events_in_session(session).await?;
    let mut tool_calls = 0u64;
    let mut approvals_required = 0u64;
    let mut approvals_denied = 0u64;
    let mut tool_finished = 0u64;
    let mut tool_errors = 0u64;
    for e in &rows {
        match e.event_type.as_str() {
            "tool_call_requested" => tool_calls += 1,
            "approval_required" => approvals_required += 1,
            "approval_denied" => approvals_denied += 1,
            "tool_call_finished" => {
                tool_finished += 1;
                if let Some(p) = &e.payload_inline {
                    if p.contains("\"status\":\"error\"") || p.contains("\"error\":") {
                        tool_errors += 1;
                    }
                }
            }
            _ => {}
        }
    }
    let approval_density = if tool_calls == 0 {
        0.0
    } else {
        approvals_required as f64 / tool_calls as f64
    };
    let denial_rate = if approvals_required == 0 {
        0.0
    } else {
        approvals_denied as f64 / approvals_required as f64
    };
    let error_rate = if tool_finished == 0 {
        0.0
    } else {
        tool_errors as f64 / tool_finished as f64
    };
    let score = (approval_density + denial_rate + error_rate) / 3.0;
    Ok(DriftScore {
        score,
        approval_density,
        denial_rate,
        error_rate,
        tool_calls,
    })
}

/// Compute the score and persist a `drift_signal` row if it exceeds
/// `threshold`. Returns `Some(id)` when a row was inserted.
pub async fn record_if_drifting(
    storage: &Storage,
    workspace: &WorkspaceId,
    actor: &ActorId,
    session: &SessionId,
    threshold: f64,
) -> Result<Option<String>, ActantError> {
    let score = score_session(storage, session).await?;
    if !score.exceeds(threshold) {
        return Ok(None);
    }
    let id = format!("ds_{}", ulid::Ulid::new());
    let components = serde_json::to_string(&score).unwrap_or_else(|_| "{}".into());
    sqlx::query(
        "INSERT INTO drift_signal (id, workspace_id, session_id, actor_id, score,
                                    components, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace.as_str())
    .bind(session.as_str())
    .bind(actor.as_str())
    .bind(score.score)
    .bind(&components)
    .bind(now_rfc3339())
    .execute(storage.pool())
    .await
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actant_core::*;
    use actant_storage::StorageConfig;

    async fn fixture() -> (Storage, WorkspaceId, ActorId, SessionId) {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let ws = Workspace {
            id: WorkspaceId::new(),
            name: "t".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        s.insert_workspace(&ws).await.unwrap();
        let actor = Actor {
            id: ActorId::new(),
            workspace_id: ws.id.clone(),
            kind: ActorKind::Agent,
            display_name: "agent".into(),
            created_at: now_rfc3339(),
            disabled_at: None,
        };
        s.insert_actor(&actor).await.unwrap();
        let session = Session {
            id: SessionId::new(),
            workspace_id: ws.id.clone(),
            title: None,
            initiator_actor_id: actor.id.clone(),
            agent_actor_id: None,
            status: SessionStatus::Active,
            created_at: now_rfc3339(),
            closed_at: None,
        };
        s.insert_session(&session).await.unwrap();
        (s, ws.id, actor.id, session.id)
    }

    async fn emit(
        s: &Storage,
        ws: &WorkspaceId,
        actor: &ActorId,
        session: &SessionId,
        kind: &str,
        payload: serde_json::Value,
    ) {
        let p = canonical_json(&payload);
        let h = sha256_hex(p.as_bytes());
        let prev = s
            .last_event_hash(ws, Some(session))
            .await
            .unwrap()
            .unwrap_or_else(|| "0".repeat(64));
        let ev = AgentEvent {
            id: EventId::new(),
            workspace_id: ws.clone(),
            actor_id: actor.clone(),
            session_id: Some(session.clone()),
            parent_event_id: None,
            event_type: kind.into(),
            causality_kind: CausalityKind::Audit,
            sensitivity: Sensitivity::Low,
            authority_scope_id: None,
            payload_ref: None,
            payload_inline: Some(p),
            payload_hash: h.clone(),
            event_hash: chain_hash(&prev, &h),
            created_at: now_rfc3339(),
            model_call_id: None,
            tool_call_id: None,
            workflow_run_id: None,
            memory_id: None,
            artifact_id: None,
            command_id: None,
            effect_id: None,
        };
        s.append_event(&ev).await.unwrap();
    }

    #[tokio::test]
    async fn quiet_session_scores_low() {
        let (s, ws, actor, sess) = fixture().await;
        emit(
            &s,
            &ws,
            &actor,
            &sess,
            "tool_call_requested",
            serde_json::json!({}),
        )
        .await;
        emit(
            &s,
            &ws,
            &actor,
            &sess,
            "tool_call_finished",
            serde_json::json!({"status":"ok"}),
        )
        .await;
        let score = score_session(&s, &sess).await.unwrap();
        assert_eq!(score.tool_calls, 1);
        assert!(score.score < 0.1);
    }

    #[tokio::test]
    async fn drifting_session_writes_signal() {
        let (s, ws, actor, sess) = fixture().await;
        for _ in 0..3 {
            emit(
                &s,
                &ws,
                &actor,
                &sess,
                "tool_call_requested",
                serde_json::json!({}),
            )
            .await;
            emit(
                &s,
                &ws,
                &actor,
                &sess,
                "approval_required",
                serde_json::json!({}),
            )
            .await;
            emit(
                &s,
                &ws,
                &actor,
                &sess,
                "approval_denied",
                serde_json::json!({}),
            )
            .await;
            emit(
                &s,
                &ws,
                &actor,
                &sess,
                "tool_call_finished",
                serde_json::json!({"status":"error"}),
            )
            .await;
        }
        let signal_id = record_if_drifting(&s, &ws, &actor, &sess, 0.5)
            .await
            .unwrap();
        assert!(signal_id.is_some());
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM drift_signal")
            .fetch_one(s.pool())
            .await
            .unwrap();
        assert_eq!(n, 1);
    }
}
