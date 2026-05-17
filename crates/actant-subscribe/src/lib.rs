//! actant-subscribe — live subscription engine.
//!
//! A subscription is `(workspace, filter)`. The engine broadcasts a JSON
//! event over a tokio broadcast channel whenever any matching row is
//! written. Phase 1 supports event-stream subscriptions only; richer
//! row-level patches arrive in Phase 2.
//!
//! See `/specs/08-api-spec.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::HashMap;
use std::sync::Arc;

use actant_core::*;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

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
}
