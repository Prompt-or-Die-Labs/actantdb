//! [`SyncRunner`] — pumps Chronicle events from storage to a [`Destination`].
//!
//! Two operating modes:
//!
//! - [`SyncRunner::run_once`] — pull-until-empty, then return. Useful as a
//!   one-shot job invoked by a cron / CLI / test.
//! - [`SyncRunner::run_tail`] — long-running; sleeps `idle_backoff` after
//!   every empty poll and exits when the supplied cancellation token fires.
//!
//! Per-batch flow:
//!
//! 1. Read the destination's persisted cursor.
//! 2. Query storage for up to `batch_size` events strictly after the cursor.
//! 3. Push them via `Destination::push`.
//! 4. Record a [`crate::BatchSummary`] and repeat until the read returns 0.

use std::sync::Arc;
use std::time::Duration;

use actant_core::WorkspaceId;
use actant_storage::Storage;
use tracing::{debug, info, warn};

use crate::destination::BatchSummary;
use crate::storage_query::events_after;
use crate::{Destination, SyncError};

/// Configuration for [`SyncRunner`].
#[derive(Debug, Clone)]
pub struct SyncRunnerConfig {
    /// Maximum events per `push` call. Default 100.
    pub batch_size: u32,
    /// Sleep between empty polls in tail mode. Default 1s.
    pub idle_backoff: Duration,
}

impl Default for SyncRunnerConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            idle_backoff: Duration::from_secs(1),
        }
    }
}

/// Outcome of [`SyncRunner::run_once`].
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    /// Total events pushed across all batches in this invocation.
    pub events_pushed: usize,
    /// Number of distinct batches issued to the destination.
    pub batches: usize,
    /// Final cursor at the destination after the run.
    pub final_cursor: Option<actant_core::EventId>,
}

/// Cooperative cancellation handle used by [`SyncRunner::run_tail`].
///
/// Cheap to clone — both the runner and the caller hold the same `Arc`.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<std::sync::atomic::AtomicBool>,
}

impl CancelToken {
    /// New, un-fired token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the token fired. Idempotent.
    pub fn cancel(&self) {
        self.flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Whether the token has been fired.
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// The runner itself. Holds a clone of `Storage` (cheap — `Arc`d pool) and
/// an `Arc<dyn Destination>`.
pub struct SyncRunner {
    storage: Storage,
    workspace: WorkspaceId,
    destination: Arc<dyn Destination>,
    config: SyncRunnerConfig,
}

impl SyncRunner {
    /// Construct a new runner for `(storage, workspace, destination)`.
    pub fn new(
        storage: Storage,
        workspace: WorkspaceId,
        destination: Arc<dyn Destination>,
    ) -> Self {
        Self {
            storage,
            workspace,
            destination,
            config: SyncRunnerConfig::default(),
        }
    }

    /// Builder: override the default config.
    pub fn with_config(mut self, config: SyncRunnerConfig) -> Self {
        self.config = config;
        self
    }

    /// One-shot: pump until the storage read returns zero rows, then return.
    pub async fn run_once(&self) -> Result<SyncStats, SyncError> {
        let mut stats = SyncStats::default();
        loop {
            let cursor = self.destination.cursor(&self.workspace).await?;
            let batch = events_after(
                &self.storage,
                &self.workspace,
                cursor.as_ref(),
                self.config.batch_size,
            )
            .await?;
            if batch.is_empty() {
                stats.final_cursor = cursor;
                debug!(
                    workspace = self.workspace.as_str(),
                    dest = self.destination.name(),
                    "sync run_once drained"
                );
                return Ok(stats);
            }
            let count = batch.len();
            let new_cursor = self
                .destination
                .push(&self.workspace, cursor.as_ref(), &batch)
                .await?;
            stats.events_pushed += count;
            stats.batches += 1;
            stats.final_cursor = new_cursor.clone();
            info!(
                summary = %BatchSummary {
                    workspace_id: self.workspace.clone(),
                    count,
                    from: cursor,
                    to: new_cursor,
                },
                dest = self.destination.name(),
                "sync batch pushed"
            );
            if count < self.config.batch_size as usize {
                // Source is drained; no point looping back for an empty read.
                return Ok(stats);
            }
        }
    }

    /// Tail mode: pump batches forever, sleeping `idle_backoff` between
    /// empty polls. Exits when `cancel.is_cancelled()` returns true.
    pub async fn run_tail(&self, cancel: CancelToken) -> Result<SyncStats, SyncError> {
        let mut acc = SyncStats::default();
        while !cancel.is_cancelled() {
            let round = match self.run_once().await {
                Ok(s) => s,
                Err(e) => {
                    warn!(
                        error = %e,
                        dest = self.destination.name(),
                        "sync run_tail round failed; will retry after backoff"
                    );
                    tokio::time::sleep(self.config.idle_backoff).await;
                    continue;
                }
            };
            acc.events_pushed += round.events_pushed;
            acc.batches += round.batches;
            acc.final_cursor = round.final_cursor;
            if round.events_pushed == 0 {
                // Idle — sleep before polling again.
                tokio::time::sleep(self.config.idle_backoff).await;
            }
        }
        debug!(
            workspace = self.workspace.as_str(),
            dest = self.destination.name(),
            "sync run_tail exiting (cancel fired)"
        );
        Ok(acc)
    }
}
