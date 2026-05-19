//! [`Destination`] — push-only sync target with a resumable cursor.
//!
//! A destination is an addressable place — local directory, S3 bucket, GCS
//! prefix, IPFS node — to which the [`crate::SyncRunner`] streams Chronicle
//! events as one immutable JSON object per event. Destinations persist a
//! per-workspace cursor next to the data so a runner can crash mid-batch and
//! resume without re-uploading or losing events.
//!
//! # Contract
//!
//! - `push(workspace_id, since_event_id, batch)` writes every event in the
//!   batch (idempotently — re-pushing the same `(workspace, event_id)` pair
//!   MUST NOT produce a duplicate object) and advances the cursor to the
//!   final event in the batch *atomically with the last write*. If a batch
//!   write fails partway, the persisted cursor stays at `since_event_id`.
//! - `cursor(workspace_id)` returns the last `EventId` durably written, or
//!   `None` if the workspace has never been synced through this destination.
//! - `since_event_id` is the runner's assertion of what it believes the
//!   current cursor to be. Destinations MAY treat a mismatch as a soft error
//!   (just retry from the persisted cursor) — it exists for diagnostics.
//! - `name()` is a short human-readable identifier used in tracing output.
//!
//! # Key layout
//!
//! Every destination MUST lay objects out as
//! `<workspace_id>/<YYYY-MM-DD>/<event_id>.json`, where the date partition
//! is taken from `AgentEvent::created_at`. The cursor lives at
//! `<workspace_id>/_cursor.txt` and contains the bare event id as UTF-8.
//! This layout is content-addressed at the event-id level, so idempotency
//! falls out for free.

use std::fmt;

use actant_core::{AgentEvent, EventId, WorkspaceId};
use async_trait::async_trait;

use crate::SyncError;

/// Trait every sync destination implements.
///
/// Implementations are `Send + Sync` so they can sit inside `Arc` and be
/// shared between the runner and any control-plane callers (e.g. a CLI that
/// pokes the destination directly to inspect the cursor).
#[async_trait]
pub trait Destination: Send + Sync {
    /// Push `batch` to the destination. `since_event_id` is the cursor the
    /// runner *believes* is current; the destination MAY use it for an
    /// optimistic consistency check. Returns the event id of the last event
    /// that was durably written (which becomes the new cursor).
    async fn push(
        &self,
        workspace_id: &WorkspaceId,
        since_event_id: Option<&EventId>,
        batch: &[AgentEvent],
    ) -> Result<Option<EventId>, SyncError>;

    /// The cursor currently persisted at the destination for `workspace_id`,
    /// or `None` if nothing has been written for this workspace yet.
    async fn cursor(&self, workspace_id: &WorkspaceId) -> Result<Option<EventId>, SyncError>;

    /// Short identifier used in tracing spans. e.g. `"filesystem"`, `"s3"`.
    fn name(&self) -> &str;
}

/// Convenience: an `Arc<dyn Destination>` is the natural container.
pub type DynDestination = std::sync::Arc<dyn Destination>;

/// The path partition used inside every destination implementation.
///
/// Centralised so that filesystem / S3 / GCS / Azure / IPFS agree on the key
/// shape. `created_at` is the canonical RFC-3339 timestamp from the event;
/// the partition is the leading `YYYY-MM-DD` of that timestamp.
pub(crate) fn key_for(workspace: &WorkspaceId, event: &AgentEvent) -> String {
    let partition = event.created_at.get(..10).unwrap_or("0000-00-00");
    format!(
        "{ws}/{partition}/{id}.json",
        ws = sanitize(workspace.as_str()),
        partition = partition,
        id = sanitize(event.id.as_str()),
    )
}

/// Cursor key for `workspace`.
pub(crate) fn cursor_key(workspace: &WorkspaceId) -> String {
    format!("{}/_cursor.txt", sanitize(workspace.as_str()))
}

/// Replace any character outside the allow-list with `_`. The allow-list is
/// the same one `actant_objectstore::is_safe_key` permits for filesystem
/// stores (`[A-Za-z0-9_-.]`); destinations that talk to object stores tolerate
/// a wider character set but keeping the projection consistent across
/// backends means a workspace's key shape does not depend on the backend.
fn sanitize(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Human-readable batch summary used by the runner's tracing output.
#[derive(Debug, Clone)]
pub struct BatchSummary {
    /// Workspace these events came from.
    pub workspace_id: WorkspaceId,
    /// Number of events in the batch.
    pub count: usize,
    /// Cursor *before* the batch was applied. `None` if this was the first
    /// push for the workspace.
    pub from: Option<EventId>,
    /// Cursor *after* the batch was applied. Matches the id of the final
    /// event in the batch, or `from` if the batch was empty.
    pub to: Option<EventId>,
}

impl fmt::Display for BatchSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let from = self.from.as_ref().map(|c| c.as_str()).unwrap_or("∅");
        let to = self.to.as_ref().map(|c| c.as_str()).unwrap_or("∅");
        write!(
            f,
            "ws={} count={} {from}→{to}",
            self.workspace_id.as_str(),
            self.count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actant_core::{ActorId, CausalityKind, Sensitivity};

    fn ev(id: &str, created: &str) -> AgentEvent {
        AgentEvent {
            id: EventId::from_string(id.to_string()),
            workspace_id: WorkspaceId::from_string("ws_abc"),
            actor_id: ActorId::from_string("act_x"),
            session_id: None,
            parent_event_id: None,
            event_type: "x".into(),
            causality_kind: CausalityKind::Audit,
            sensitivity: Sensitivity::Low,
            authority_scope_id: None,
            payload_ref: None,
            payload_inline: None,
            payload_hash: "h".into(),
            event_hash: "h".into(),
            created_at: created.into(),
            model_call_id: None,
            tool_call_id: None,
            workflow_run_id: None,
            memory_id: None,
            artifact_id: None,
            command_id: None,
            effect_id: None,
        }
    }

    #[test]
    fn key_uses_yyyy_mm_dd_partition() {
        let ws = WorkspaceId::from_string("ws_abc");
        let e = ev("evt_001", "2026-05-19T12:34:56Z");
        assert_eq!(key_for(&ws, &e), "ws_abc/2026-05-19/evt_001.json");
    }

    #[test]
    fn cursor_path_is_workspace_scoped() {
        let ws = WorkspaceId::from_string("ws_abc");
        assert_eq!(cursor_key(&ws), "ws_abc/_cursor.txt");
    }

    #[test]
    fn sanitize_replaces_unsafe_chars() {
        let ws = WorkspaceId::from_string("ws/with/slash");
        assert_eq!(cursor_key(&ws), "ws_with_slash/_cursor.txt");
    }
}
