//! actant-sync — cluster / multi-device synchronization.
//!
//! This crate ships two layers:
//!
//! 1. The Phase-1 deterministic diff ([`missing_in`]) — a pure function used
//!    by the two-node convergence test and any consumer that needs to compute
//!    "what's on A that's not on B" without I/O.
//! 2. The Phase-6 push-only sync engine: a [`Destination`] trait + concrete
//!    backends ([`FilesystemDestination`], plus feature-gated S3 / GCS /
//!    Azure / IPFS), and a [`SyncRunner`] that pulls events out of
//!    `actant-storage` and feeds them to a destination.
//!
//! The destination layout is uniform across backends:
//!
//! ```text
//! <root>/<workspace_id>/<YYYY-MM-DD>/<event_id>.json     -- per-event
//! <root>/<workspace_id>/_cursor.txt                      -- resume token
//! ```
//!
//! See `crates/actant-sync/README.md` for end-to-end usage examples.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::*;

mod destination;
mod destinations;
mod error;
mod runner;
mod storage_query;

pub use destination::{BatchSummary, Destination, DynDestination};
pub use destinations::FilesystemDestination;
pub use error::SyncError;
pub use runner::{CancelToken, SyncRunner, SyncRunnerConfig, SyncStats};
pub use storage_query::events_after;

#[cfg(feature = "s3")]
pub use destinations::S3Destination;

#[cfg(feature = "gcs")]
pub use destinations::{GcsConfig, GcsDestination};

#[cfg(feature = "azure")]
pub use destinations::{AzureConfig, AzureDestination};

#[cfg(feature = "ipfs")]
pub use destinations::IpfsDestination;

/// Compute the set of event ids in `a` not present in `b`.
///
/// Pure function — exists for the in-process diff that powers the two-node
/// convergence test. Network sync goes through [`SyncRunner`] instead.
pub fn missing_in(a: &[AgentEvent], b: &[AgentEvent]) -> Vec<EventId> {
    let set: std::collections::HashSet<&str> = b.iter().map(|e| e.id.as_str()).collect();
    a.iter()
        .filter(|e| !set.contains(e.id.as_str()))
        .map(|e| e.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: &str) -> AgentEvent {
        AgentEvent {
            id: EventId::from_string(id.to_string()),
            workspace_id: WorkspaceId::new(),
            actor_id: ActorId::new(),
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
            created_at: now_rfc3339(),
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
    fn missing_set() {
        let a = vec![ev("e1"), ev("e2"), ev("e3")];
        let b = vec![ev("e1")];
        let m = missing_in(&a, &b);
        assert_eq!(m.len(), 2);
    }
}
