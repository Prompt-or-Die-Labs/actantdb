//! actant-subscribe — live subscription engine.
//!
//! A subscription is `(workspace, filter)`. The engine broadcasts a JSON
//! event over a tokio broadcast channel whenever any matching row is
//! written. Phase 1 supports event-stream subscriptions only; richer
//! row-level patches arrive in Phase 2.
//!
//! ## Row-level predicates (GAPS.md #20)
//!
//! [`SubscribeHub::subscribe`] returns a raw `broadcast::Receiver<Message>`
//! and remains the topic-keyed fan-out used by the server's WebSocket
//! route. For row-level filtering, callers use
//! [`SubscribeHub::subscribe_filtered`], which returns a
//! [`FilteredSubscription`] that wraps a broadcast receiver plus an
//! optional [`Predicate`]. Messages that fail the predicate never reach
//! the consumer's `recv()`.
//!
//! Filtering happens at fan-out (the subscriber side), not at publish
//! time. This keeps a single `broadcast::Sender` per topic (and lets
//! multiple subscribers with different predicates share one channel). The
//! trade-off: unmatched messages still occupy the receiver's broadcast
//! buffer until the wrapper drops them; the saving is only that the user
//! task isn't woken to discard them. Good enough when predicates aren't
//! ultra-selective; revisit if that changes.
//!
//! See `/specs/08-api-spec.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Predicate is a recursive enum; serde_json's Serializer trait resolution
// blows past the default 128 recursion limit on the integration tests
// that serialize nested `Predicate::And(vec![Predicate::Or(...)])`
// expressions.
#![recursion_limit = "1024"]

mod broker;
mod predicate;

use std::collections::HashMap;
use std::sync::Arc;

use actant_core::*;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

pub use broker::{Broker, Envelope};
pub use predicate::{evaluate, Predicate};

/// One subscription topic, identified by a filter object.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Topic {
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Optional session.
    pub session_id: Option<SessionId>,
    /// One of: "events", "approvals", "memories".
    pub kind: String,
}

/// A published message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Topic this was published to.
    pub topic: Topic,
    /// JSON payload (typically an event or row).
    pub payload: serde_json::Value,
    /// RFC3339 publication time.
    pub published_at: String,
}

/// The shared subscribe hub.
#[derive(Clone)]
pub struct SubscribeHub {
    inner: Arc<Inner>,
}

struct Inner {
    senders: RwLock<HashMap<Topic, broadcast::Sender<Message>>>,
}

impl SubscribeHub {
    /// New hub with no subscribers.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                senders: RwLock::new(HashMap::new()),
            }),
        }
    }

    /// Get (or create) a broadcast receiver for the topic.
    pub async fn subscribe(&self, topic: Topic) -> broadcast::Receiver<Message> {
        let mut guard = self.inner.senders.write().await;
        let entry = guard.entry(topic.clone()).or_insert_with(|| {
            let (tx, _) = broadcast::channel(256);
            tx
        });
        entry.subscribe()
    }

    /// Subscribe with an optional row-level predicate. The returned
    /// [`FilteredSubscription`] only yields messages whose JSON payload
    /// satisfies the predicate; non-matching messages are silently dropped
    /// on the receiver side.
    ///
    /// `predicate = None` (or `Some(Predicate::True)`) is equivalent to the
    /// raw [`subscribe`](Self::subscribe) call wrapped for API consistency.
    pub async fn subscribe_filtered(
        &self,
        topic: Topic,
        predicate: Option<Predicate>,
    ) -> FilteredSubscription {
        let rx = self.subscribe(topic).await;
        FilteredSubscription { rx, predicate }
    }

    /// Publish a message to a topic. Lost messages are dropped silently.
    pub async fn publish(&self, topic: Topic, payload: serde_json::Value) {
        let guard = self.inner.senders.read().await;
        if let Some(tx) = guard.get(&topic) {
            let _ = tx.send(Message {
                topic: topic.clone(),
                payload,
                published_at: now_rfc3339(),
            });
        }
    }

    /// Snapshot — count of active subscribers per topic.
    pub async fn stats(&self) -> Vec<(Topic, usize)> {
        let guard = self.inner.senders.read().await;
        guard
            .iter()
            .map(|(t, tx)| (t.clone(), tx.receiver_count()))
            .collect()
    }
}

impl Default for SubscribeHub {
    fn default() -> Self {
        Self::new()
    }
}

/// A broadcast subscription with an attached optional [`Predicate`].
///
/// `recv()` loops over the underlying broadcast channel, dropping every
/// message that fails the predicate, until either a matching message
/// arrives or the underlying channel closes / lags.
pub struct FilteredSubscription {
    rx: broadcast::Receiver<Message>,
    predicate: Option<Predicate>,
}

impl FilteredSubscription {
    /// Receive the next message that matches the predicate.
    ///
    /// Returns the underlying channel error on close / `Lagged` exactly as
    /// `broadcast::Receiver::recv` would. A `Lagged` error reflects the
    /// underlying channel and is unaffected by the predicate: lagging is
    /// always counted against the raw stream, not the filtered view.
    pub async fn recv(&mut self) -> Result<Message, broadcast::error::RecvError> {
        loop {
            let msg = self.rx.recv().await?;
            if self.matches(&msg) {
                return Ok(msg);
            }
        }
    }

    /// Try-receive: non-blocking variant. Skips non-matching messages and
    /// surfaces the first matching one or [`broadcast::error::TryRecvError::Empty`]
    /// when the queue is drained without a match.
    pub fn try_recv(&mut self) -> Result<Message, broadcast::error::TryRecvError> {
        loop {
            let msg = self.rx.try_recv()?;
            if self.matches(&msg) {
                return Ok(msg);
            }
        }
    }

    /// Borrow the underlying broadcast receiver. Bypasses the predicate.
    pub fn raw(&mut self) -> &mut broadcast::Receiver<Message> {
        &mut self.rx
    }

    /// Borrow the attached predicate, if any.
    pub fn predicate(&self) -> Option<&Predicate> {
        self.predicate.as_ref()
    }

    fn matches(&self, msg: &Message) -> bool {
        match &self.predicate {
            None => true,
            Some(p) => p.evaluate(&msg.payload),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_to_subscriber() {
        let hub = SubscribeHub::new();
        let t = Topic {
            workspace_id: WorkspaceId::new(),
            session_id: None,
            kind: "events".into(),
        };
        let mut rx = hub.subscribe(t.clone()).await;
        hub.publish(t.clone(), serde_json::json!({"hello":"world"}))
            .await;
        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.payload["hello"], "world");
    }

    #[tokio::test]
    async fn filtered_subscription_drops_non_matching() {
        let hub = SubscribeHub::new();
        let t = Topic {
            workspace_id: WorkspaceId::new(),
            session_id: None,
            kind: "events".into(),
        };
        let mut sub = hub
            .subscribe_filtered(
                t.clone(),
                Some(Predicate::Eq {
                    field: "kind".into(),
                    value: serde_json::json!("keep"),
                }),
            )
            .await;
        hub.publish(t.clone(), serde_json::json!({"kind":"drop"}))
            .await;
        hub.publish(t.clone(), serde_json::json!({"kind":"keep"}))
            .await;
        let msg = sub.recv().await.unwrap();
        assert_eq!(msg.payload["kind"], "keep");
    }
}
