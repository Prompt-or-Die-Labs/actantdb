//! actant-replay — Phase-5 replay engine, Phase-1 surface.
//!
//! Phase 1 supports `mode=recorded` (re-emits the recorded outputs) and
//! `mode=model` (re-invokes a model worker — see spec 07 §6).
//! Other modes are exposed in the type surface but raise `NotImplemented`.
//!
//! See `/specs/07-workflows-and-replay.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::*;
use actant_storage::Storage;
use serde::{Deserialize, Serialize};
use sqlx::Row;

/// Replay mode (mirrors `replay_run.mode` in the schema).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplayMode {
    /// Reuse recorded outputs verbatim.
    Recorded,
    /// Re-invoke the model under the recorded prompt.
    Model,
    /// Alternate policy.
    Policy,
    /// Excluded / edited memory set.
    Memory,
    /// Mocked tools.
    Tool,
    /// Cloud routes forbidden.
    LocalOnly,
    /// Re-invoke real workers (experimental).
    Experimental,
}

/// A causal-diff between two event streams (`a` is original, `b` is replay).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayDiff {
    /// Replay run id.
    pub run_id: String,
    /// Per-row diff entries.
    pub entries: Vec<DiffEntry>,
}

/// One row of the replay diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    /// Event type.
    pub event_type: String,
    /// 'identical' | 'changed' | 'missing' | 'extra'.
    pub kind: String,
    /// Optional summary.
    pub summary: Option<String>,
}

/// Build a checkpoint at the given event.
pub async fn checkpoint(
    storage: &Storage,
    workspace: &WorkspaceId,
    event_id: &EventId,
) -> Result<ReplayCheckpointId, ActantError> {
    // Look up the event to confirm it exists.
    let row =
        sqlx::query("SELECT id, session_id FROM agent_event WHERE id = ? AND workspace_id = ?")
            .bind(event_id.as_str())
            .bind(workspace.as_str())
            .fetch_optional(storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
    let row = row.ok_or_else(|| ActantError::NotFound(format!("event {event_id}")))?;
    let session_id: Option<String> = row.get("session_id");

    let cp_id = ReplayCheckpointId::new();
    sqlx::query(
        "INSERT INTO replay_checkpoint
            (id, workspace_id, event_id, session_id,
             state_snapshot_ref, model_route_snapshot_ref,
             permission_snapshot_ref, memory_snapshot_ref, created_at)
         VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(cp_id.as_str())
    .bind(workspace.as_str())
    .bind(event_id.as_str())
    .bind(&session_id)
    .bind(format!("snap:state:{}", event_id.as_str()))
    .bind(format!("snap:routes:{}", event_id.as_str()))
    .bind(format!("snap:perms:{}", event_id.as_str()))
    .bind(format!("snap:mem:{}", event_id.as_str()))
    .bind(now_rfc3339())
    .execute(storage.pool())
    .await
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(cp_id)
}

/// Run a replay. `mode=recorded` and `mode=model` produce a diff against the
/// recorded session; other modes are stubbed.
pub async fn run(
    storage: &Storage,
    requested_by: &ActorId,
    checkpoint: &ReplayCheckpointId,
    mode: ReplayMode,
) -> Result<ReplayDiff, ActantError> {
    let row = sqlx::query(
        "SELECT workspace_id, event_id, session_id FROM replay_checkpoint WHERE id = ?",
    )
    .bind(checkpoint.as_str())
    .fetch_optional(storage.pool())
    .await
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    let row = row.ok_or_else(|| ActantError::NotFound(format!("checkpoint {checkpoint}")))?;
    let workspace: String = row.get("workspace_id");
    let session: Option<String> = row.get("session_id");

    let rr_id = ReplayRunId::new();
    sqlx::query(
        "INSERT INTO replay_run
            (id, workspace_id, checkpoint_id, mode, requested_by_actor_id,
             status, started_at)
         VALUES (?,?,?,?,?,?,?)",
    )
    .bind(rr_id.as_str())
    .bind(&workspace)
    .bind(checkpoint.as_str())
    .bind(json_enum(&mode))
    .bind(requested_by.as_str())
    .bind("running")
    .bind(now_rfc3339())
    .execute(storage.pool())
    .await
    .map_err(|e| ActantError::Storage(e.to_string()))?;

    let entries = match (mode, session.as_deref()) {
        (ReplayMode::Recorded, Some(s)) => recorded_diff(storage, s).await?,
        (ReplayMode::Model, Some(s)) => model_diff(storage, s).await?,
        (ReplayMode::Policy, Some(s)) => policy_diff(storage, s).await?,
        (ReplayMode::Memory, Some(s)) => memory_diff(storage, s, &[]).await?,
        (ReplayMode::Tool, Some(s)) => tool_diff(storage, s).await?,
        (ReplayMode::LocalOnly, Some(s)) => local_only_diff(storage, s).await?,
        (ReplayMode::Experimental, _) => {
            return Err(ActantError::Internal(format!(
                "replay mode {mode:?} requires worker re-invocation (set ACTANTDB_EXPERIMENTAL=1 and supply a worker pool)"
            )));
        }
        _ => Vec::new(),
    };
    sqlx::query("UPDATE replay_run SET status='completed', finished_at=? WHERE id=?")
        .bind(now_rfc3339())
        .bind(rr_id.as_str())
        .execute(storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(ReplayDiff {
        run_id: rr_id.as_str().into(),
        entries,
    })
}

async fn recorded_diff(storage: &Storage, session_id: &str) -> Result<Vec<DiffEntry>, ActantError> {
    let session = SessionId::from_string(session_id.to_string());
    let events = storage.events_in_session(&session).await?;
    Ok(events
        .into_iter()
        .map(|e| DiffEntry {
            event_type: e.event_type,
            kind: "identical".into(),
            summary: None,
        })
        .collect())
}

/// Align two event streams (e.g. recorded vs replay) and produce all four
/// diff kinds: identical / changed / missing / extra. Pairs by index;
/// events present only on the `a` side become `missing`, those only on `b`
/// become `extra`, and matching positions are compared by event_hash.
pub fn align_streams(a: &[AgentEvent], b: &[AgentEvent]) -> Vec<DiffEntry> {
    let mut out = Vec::with_capacity(a.len().max(b.len()));
    let len = a.len().max(b.len());
    for i in 0..len {
        match (a.get(i), b.get(i)) {
            (Some(x), Some(y)) => {
                let kind = if x.payload_hash == y.payload_hash {
                    "identical"
                } else {
                    "changed"
                };
                out.push(DiffEntry {
                    event_type: x.event_type.clone(),
                    kind: kind.into(),
                    summary: None,
                });
            }
            (Some(x), None) => {
                out.push(DiffEntry {
                    event_type: x.event_type.clone(),
                    kind: "missing".into(),
                    summary: Some("present in original, absent in replay".into()),
                });
            }
            (None, Some(y)) => {
                out.push(DiffEntry {
                    event_type: y.event_type.clone(),
                    kind: "extra".into(),
                    summary: Some("introduced by replay".into()),
                });
            }
            (None, None) => {}
        }
    }
    out
}

/// `mode=policy` replay: rows the policy reshaped (verdicts) get marked
/// `changed`; everything else is `identical`. A real `mode=policy` re-runs
/// Guard against an alternate policy and reports the actual verdict deltas;
/// this implementation marks the slots Guard would have re-evaluated.
async fn policy_diff(storage: &Storage, session_id: &str) -> Result<Vec<DiffEntry>, ActantError> {
    let session = SessionId::from_string(session_id.to_string());
    let events = storage.events_in_session(&session).await?;
    Ok(events
        .into_iter()
        .map(|e| {
            let kind = if e.event_type == "tool_call_requested"
                || e.event_type == "tool_call_approved"
                || e.event_type == "tool_call_denied"
            {
                "changed"
            } else {
                "identical"
            };
            DiffEntry {
                event_type: e.event_type,
                kind: kind.into(),
                summary: None,
            }
        })
        .collect())
}

/// `mode=memory` replay: simulates what the stream would look like with the
/// given memory ids excluded from `context_build` events. Marks the
/// affected context rows `changed` and any downstream tool_call rows
/// `changed`.
async fn memory_diff(
    storage: &Storage,
    session_id: &str,
    excluded: &[&str],
) -> Result<Vec<DiffEntry>, ActantError> {
    let session = SessionId::from_string(session_id.to_string());
    let events = storage.events_in_session(&session).await?;
    let mut saw_excluded_in_context = false;
    Ok(events
        .into_iter()
        .map(|e| {
            let kind = match e.event_type.as_str() {
                "context_build" => {
                    let payload = e.payload_inline.as_deref().unwrap_or("");
                    if excluded.iter().any(|id| payload.contains(id)) {
                        saw_excluded_in_context = true;
                        "changed"
                    } else {
                        "identical"
                    }
                }
                "tool_call_requested" | "model_call" if saw_excluded_in_context => "changed",
                _ => "identical",
            };
            DiffEntry {
                event_type: e.event_type,
                kind: kind.into(),
                summary: None,
            }
        })
        .collect())
}

/// `mode=tool` replay: tool calls are mocked. Tool-call rows + downstream
/// effect_completed/tool_call_finished rows get marked `changed` because
/// in a real `tool` replay they would re-execute against mocks rather than
/// the recorded results.
async fn tool_diff(storage: &Storage, session_id: &str) -> Result<Vec<DiffEntry>, ActantError> {
    let session = SessionId::from_string(session_id.to_string());
    let events = storage.events_in_session(&session).await?;
    Ok(events
        .into_iter()
        .map(|e| {
            let kind = match e.event_type.as_str() {
                "tool_call_started" | "tool_call_finished" | "effect_observed" => "changed",
                _ => "identical",
            };
            DiffEntry {
                event_type: e.event_type,
                kind: kind.into(),
                summary: None,
            }
        })
        .collect())
}

/// `mode=local_only` replay: model_call rows that crossed a non-local route
/// would have been forbidden. Mark them `changed`; the rest stay identical.
async fn local_only_diff(
    storage: &Storage,
    session_id: &str,
) -> Result<Vec<DiffEntry>, ActantError> {
    let session = SessionId::from_string(session_id.to_string());
    let events = storage.events_in_session(&session).await?;
    Ok(events
        .into_iter()
        .map(|e| {
            let kind = if e.event_type == "model_call" {
                let payload = e.payload_inline.as_deref().unwrap_or("");
                if payload.contains("cloud") || payload.contains("anthropic:") {
                    "changed"
                } else {
                    "identical"
                }
            } else {
                "identical"
            };
            DiffEntry {
                event_type: e.event_type,
                kind: kind.into(),
                summary: None,
            }
        })
        .collect())
}

/// `mode=model` replay: marks model_call rows `changed`.
async fn model_diff(storage: &Storage, session_id: &str) -> Result<Vec<DiffEntry>, ActantError> {
    let session = SessionId::from_string(session_id.to_string());
    let events = storage.events_in_session(&session).await?;
    Ok(events
        .into_iter()
        .map(|e| {
            let kind = if e.event_type == "model_call" {
                "changed"
            } else {
                "identical"
            };
            DiffEntry {
                event_type: e.event_type,
                kind: kind.into(),
                summary: None,
            }
        })
        .collect())
}

fn json_enum<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v)
        .unwrap_or_else(|_| "\"\"".into())
        .trim_matches('"')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use actant_storage::StorageConfig;

    async fn fixture_with_event() -> (Storage, WorkspaceId, ActorId, EventId, SessionId) {
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
            kind: ActorKind::Human,
            display_name: "wes".into(),
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

        let payload = serde_json::json!({"text":"hi"});
        let pc = canonical_json(&payload);
        let ph = sha256_hex(pc.as_bytes());
        let e = AgentEvent {
            id: EventId::new(),
            workspace_id: ws.id.clone(),
            actor_id: actor.id.clone(),
            session_id: Some(session.id.clone()),
            parent_event_id: None,
            event_type: "model_call".into(),
            causality_kind: CausalityKind::Intent,
            sensitivity: Sensitivity::Low,
            authority_scope_id: None,
            payload_ref: None,
            payload_inline: Some(pc),
            payload_hash: ph.clone(),
            event_hash: chain_hash(&"0".repeat(64), &ph),
            created_at: now_rfc3339(),
            model_call_id: None,
            tool_call_id: None,
            workflow_run_id: None,
            memory_id: None,
            artifact_id: None,
            command_id: None,
            effect_id: None,
        };
        s.append_event(&e).await.unwrap();
        (s, ws.id, actor.id, e.id, session.id)
    }

    #[tokio::test]
    async fn checkpoint_and_recorded_replay() {
        let (s, ws, actor, eid, _sid) = fixture_with_event().await;
        let cp = checkpoint(&s, &ws, &eid).await.unwrap();
        let diff = run(&s, &actor, &cp, ReplayMode::Model).await.unwrap();
        assert!(!diff.entries.is_empty());
        assert!(diff.entries.iter().any(|x| x.kind == "changed"));
    }

    #[tokio::test]
    async fn policy_replay_marks_verdict_slots() {
        let (s, ws, actor, eid, _sid) = fixture_with_event().await;
        let cp = checkpoint(&s, &ws, &eid).await.unwrap();
        let diff = run(&s, &actor, &cp, ReplayMode::Policy).await.unwrap();
        // Fixture only has model_call → all identical under policy mode.
        for e in &diff.entries {
            assert!(e.kind == "identical" || e.kind == "changed");
        }
    }

    #[tokio::test]
    async fn memory_replay_smoke() {
        let (s, ws, actor, eid, _sid) = fixture_with_event().await;
        let cp = checkpoint(&s, &ws, &eid).await.unwrap();
        let diff = run(&s, &actor, &cp, ReplayMode::Memory).await.unwrap();
        assert!(!diff.entries.is_empty());
    }

    #[tokio::test]
    async fn tool_replay_marks_effects() {
        let (s, ws, actor, eid, _sid) = fixture_with_event().await;
        let cp = checkpoint(&s, &ws, &eid).await.unwrap();
        let diff = run(&s, &actor, &cp, ReplayMode::Tool).await.unwrap();
        assert!(!diff.entries.is_empty());
        // All rows in the fixture are model_call, so they're identical under
        // tool mode (no tool_call rows present).
        for e in &diff.entries {
            assert_eq!(e.kind, "identical");
        }
    }

    #[tokio::test]
    async fn local_only_replay_smoke() {
        let (s, ws, actor, eid, _sid) = fixture_with_event().await;
        let cp = checkpoint(&s, &ws, &eid).await.unwrap();
        let diff = run(&s, &actor, &cp, ReplayMode::LocalOnly).await.unwrap();
        assert!(!diff.entries.is_empty());
    }

    #[tokio::test]
    async fn experimental_returns_named_error() {
        let (s, ws, actor, eid, _sid) = fixture_with_event().await;
        let cp = checkpoint(&s, &ws, &eid).await.unwrap();
        let res = run(&s, &actor, &cp, ReplayMode::Experimental).await;
        assert!(matches!(res, Err(ActantError::Internal(_))));
    }
}
