//! actant-worker-protocol — shared types/traits used by every worker
//! binary in the substrate. Workers implement `Handler` for one or more
//! effect types and the protocol takes care of claim/heartbeat/complete.
//!
//! See `/specs/04-effect-protocol.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::time::Duration;

use actant_core::{
    now_rfc3339, ActantError, ActorId, ActorKind, Worker as WorkerRow, WorkerId, WorkspaceId,
};
use actant_effects::{EffectQueue, Lease};
use async_trait::async_trait;
use tokio::sync::watch;

/// A worker registration descriptor.
#[derive(Debug, Clone)]
pub struct WorkerDescriptor {
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Actor id assigned to this worker.
    pub actor_id: ActorId,
    /// Display name.
    pub name: String,
    /// Capabilities advertised.
    pub capabilities: Vec<String>,
}

/// Result of a worker handling an effect.
pub type HandlerResult = Result<serde_json::Value, ActantError>;

/// A handler for one or more effect types.
#[async_trait]
pub trait Handler: Send + Sync + 'static {
    /// Primary effect type. Used as a stable identifier in logs.
    fn effect_type(&self) -> &'static str;
    /// All effect types this handler serves. Default: just `effect_type`.
    /// Override when one handler can dispatch on input shape (`file.read`
    /// vs `file.write`, `browser.navigate|click|type|screenshot`, ...).
    fn effect_types(&self) -> &'static [&'static str] {
        // Returning a slice that aliases the &str returned by `effect_type`
        // is awkward in Rust; concrete handlers override this when they
        // serve multiple types.
        &[]
    }
    /// Execute the effect. `input` is the JSON the command engine enqueued.
    async fn handle(&self, input: serde_json::Value) -> HandlerResult;
}

/// The worker runner. Polls the queue, dispatches to handlers, heartbeats.
pub struct WorkerRunner {
    queue: EffectQueue,
    descriptor: WorkerDescriptor,
    worker_id: WorkerId,
    handlers: Vec<Box<dyn Handler>>,
    /// How long to sleep between empty poll iterations.
    poll_interval: Duration,
    shutdown_rx: watch::Receiver<bool>,
}

impl WorkerRunner {
    /// New runner using `queue` and `descriptor`. Handlers must implement at
    /// least one effect type per the `capabilities` list.
    pub fn new(
        queue: EffectQueue,
        descriptor: WorkerDescriptor,
        handlers: Vec<Box<dyn Handler>>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            queue,
            descriptor,
            worker_id: WorkerId::new(),
            handlers,
            poll_interval: Duration::from_millis(200),
            shutdown_rx,
        }
    }

    /// Register the worker row + capabilities.
    pub async fn register(&self) -> Result<(), ActantError> {
        let row = WorkerRow {
            id: self.worker_id.clone(),
            workspace_id: self.descriptor.workspace_id.clone(),
            actor_id: self.descriptor.actor_id.clone(),
            name: self.descriptor.name.clone(),
            host: None,
            version: Some(env!("CARGO_PKG_VERSION").into()),
            status: "online".into(),
            last_heartbeat_at: Some(now_rfc3339()),
            created_at: now_rfc3339(),
            disabled_at: None,
        };
        let cap_strs: Vec<&str> = self
            .descriptor
            .capabilities
            .iter()
            .map(String::as_str)
            .collect();
        self.queue.register_worker(&row, &cap_strs).await
    }

    /// Block until shutdown, polling for effects.
    pub async fn run(&mut self) -> Result<(), ActantError> {
        self.register().await?;
        let _ = self.maybe_ensure_actor().await;
        let caps: Vec<&str> = self
            .descriptor
            .capabilities
            .iter()
            .map(String::as_str)
            .collect();

        loop {
            if *self.shutdown_rx.borrow() {
                return Ok(());
            }
            let lease = self
                .queue
                .claim_one(&self.worker_id, &self.descriptor.workspace_id, &caps)
                .await?;
            match lease {
                Some(lease) => self.execute(lease).await,
                None => {
                    tokio::select! {
                        _ = tokio::time::sleep(self.poll_interval) => {}
                        _ = self.shutdown_rx.changed() => {}
                    }
                }
            }
            let _ = self.queue.heartbeat(&self.worker_id, 0).await;
        }
    }

    async fn execute(&self, lease: Lease) {
        let handler = self
            .handlers
            .iter()
            .find(|h| h.effect_type() == lease.effect_type);
        let Some(h) = handler else {
            let _ = self
                .queue
                .fail(&lease.effect_id, "no handler for effect_type")
                .await;
            return;
        };
        let _ = self.queue.start(&lease.effect_id).await;
        let input: serde_json::Value = lease
            .input_inline
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::json!({}));
        match h.handle(input).await {
            Ok(output) => {
                let _ = self.queue.complete(&lease.effect_id, &output).await;
            }
            Err(e) => {
                let _ = self.queue.fail(&lease.effect_id, &e.to_string()).await;
            }
        }
    }

    async fn maybe_ensure_actor(&self) -> Result<(), ActantError> {
        let s = self.queue.storage();
        if s.get_actor(&self.descriptor.actor_id).await?.is_none() {
            s.insert_actor(&actant_core::Actor {
                id: self.descriptor.actor_id.clone(),
                workspace_id: self.descriptor.workspace_id.clone(),
                kind: ActorKind::Worker,
                display_name: self.descriptor.name.clone(),
                created_at: now_rfc3339(),
                disabled_at: None,
            })
            .await?;
        }
        Ok(())
    }
}
