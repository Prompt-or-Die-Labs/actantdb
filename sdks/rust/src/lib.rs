//! # actantdb-client
//!
//! Rust HTTP + WebSocket client for an [ActantDB] server (the `actantdb serve`
//! binary built from the `actant-server` crate).
//!
//! ## Quick start
//!
//! ```no_run
//! # async fn run() -> Result<(), actantdb_client::ActantError> {
//! use actantdb_client::ActantClient;
//! use url::Url;
//!
//! let client = ActantClient::new(Url::parse("http://127.0.0.1:4555").unwrap())
//!     .with_token("dev")
//!     .with_workspace_id("ws_demo")
//!     .with_actor_id("act_user");
//!
//! let session_id = client.create_session(None, None, Some("fix tests")).await?;
//! client.append_user_message(None, None, &session_id, "rm -rf build").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Domain vs. wire types
//!
//! Domain types (`Sensitivity`, `Risk`, `ApprovalRequest`, `ApprovalDecision`,
//! `PolicyVerdict`, `ReplayDiff`, …) are re-exported from `actant-contracts` —
//! the single source of truth per the F2/F3 binding rules. *Wire envelopes*
//! (`CommandRequest`, `CommandResponse`, `AgentEvent`, `PendingApproval`, …)
//! are defined inline here because they describe HTTP transport, not the
//! domain — the Swift SDK uses the same pattern.
//!
//! [ActantDB]: https://github.com/Prompt-or-Die-Labs/actantdb

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod client;
mod error;
mod subscribe;
mod types;

pub use client::ActantClient;
pub use error::{ActantError, Result};
pub use subscribe::{SubscriptionKind, SubscriptionMessage};
pub use types::{
    AgentEvent, CommandRequest, CommandResponse, EventsResponse, Healthz, MemoryRow,
    PendingApproval, ReplayCheckpointResponse, ReplayMode, SubscriptionTopic, SyncEvent,
    SyncSinceResponse,
};

// Re-export the domain types from `actant-contracts` so callers don't need to
// add a second dependency.
pub use actant_contracts::{
    ApprovalDecision, ApprovalRequest, CheckpointRef, DiffEntry, DiffKind, Policy, PolicyVerdict,
    ReplayDiff, Risk, Sensitivity, ToolCallRequest, ToolCallStatus,
};
