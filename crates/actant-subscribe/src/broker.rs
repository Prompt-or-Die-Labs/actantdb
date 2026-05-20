//! Generic named-topic pub/sub broker.
//!
//! Closes DEVX_GAPS row #X93. Sits alongside [`crate::SubscribeHub`] (which
//! is the in-memory broadcast bus the existing `/v1/ws` route fans out
//! through). The broker adds:
//!
//! * **Persistence** — every publish writes a row to `pubsub_message` so a
//!   subscriber that reconnects with a `since` cursor can backfill.
//! * **Workspace isolation** — the broker key is `(workspace_id, topic)`.
//!   Publishing to workspace `A` never reaches a subscriber listening on
//!   workspace `B`, even if the topic name is identical.
//! * **Cursor-based replay** — `subscribe(.., since)` first replays every
//!   persisted row strictly greater than the cursor, then attaches to the
//!   live broadcast for the tail.
//!
//! The persistence model is intentionally lightweight: pubsub messages are
//! application-layer fan-out, not Chronicle events. They don't chain and
//! they don't go through the command engine. Use [`crate::SubscribeHub`]
//! (the topic-keyed `Topic { workspace_id, session_id, kind }` API) when
//! you want to ride the existing event broadcast; use [`Broker`] when you
//! want named-topic pub/sub with backfill.
//!
//! See `/migrations/0006_pubsub.sql`.

use std::collections::HashMap;
use std::sync::Arc;

use actant_core::{now_rfc3339, ActantError, WorkspaceId};
use actant_storage::Storage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use tokio::sync::{broadcast, RwLock};

/// One persisted pub/sub envelope. The broker emits these to both
/// subscribers and persistence; the WebSocket route at
/// `/v1/pubsub/<workspace>/<topic>` serializes them as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// ULID, lexicographically sortable; usable as a `since` cursor.
    pub id: String,
    /// Topic the publisher named.
    pub topic: String,
    /// Application payload.
    pub payload: Value,
    /// RFC3339 publication time.
    pub ts: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    workspace_id: String,
    topic: String,
}

struct Inner {
    storage: Storage,
    channels: RwLock<HashMap<Key, broadcast::Sender<Envelope>>>,
    /// Per-channel broadcast capacity. Tuneable via [`Broker::with_capacity`].
    capacity: usize,
}

/// Named-topic pub/sub broker. Cheap to clone (wraps an `Arc`).
#[derive(Clone)]
pub struct Broker {
    inner: Arc<Inner>,
}

impl Broker {
    /// New broker bound to the given storage handle. Default broadcast
    /// capacity (256). Use [`Self::with_capacity`] to override.
    pub fn new(storage: Storage) -> Self {
        Self::with_capacity(storage, 256)
    }

    /// Build a broker with a custom per-channel broadcast capacity.
    pub fn with_capacity(storage: Storage, capacity: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                storage,
                channels: RwLock::new(HashMap::new()),
                capacity: capacity.max(1),
            }),
        }
    }

    /// Persist an envelope and fan it out to live subscribers of
    /// `(workspace_id, topic)`.
    pub async fn publish(
        &self,
        workspace_id: &WorkspaceId,
        topic: &str,
        payload: Value,
    ) -> Result<Envelope, ActantError> {
        if topic.is_empty() {
            return Err(ActantError::InvalidInput("topic is required".into()));
        }
        let id = format!("psm_{}", ulid::Ulid::new());
        let ts = now_rfc3339();
        // Persist first. If the write fails, we don't fan out — better to
        // surface the error than to deliver something that won't survive
        // a reconnect.
        let payload_text =
            serde_json::to_string(&payload).map_err(|e| ActantError::Storage(e.to_string()))?;
        sqlx::query(
            "INSERT INTO pubsub_message (id, workspace_id, topic, payload, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(workspace_id.as_str())
        .bind(topic)
        .bind(&payload_text)
        .bind(&ts)
        .execute(self.inner.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;

        let env = Envelope {
            id,
            topic: topic.to_string(),
            payload,
            ts,
        };
        // Fan-out (best effort — a topic with zero subscribers is fine).
        let key = Key {
            workspace_id: workspace_id.as_str().to_string(),
            topic: topic.to_string(),
        };
        let guard = self.inner.channels.read().await;
        if let Some(tx) = guard.get(&key) {
            let _ = tx.send(env.clone());
        }
        Ok(env)
    }

    /// Subscribe to `(workspace_id, topic)`. If `since` is `Some(cursor)`,
    /// the returned receiver first sees every persisted row whose id is
    /// strictly greater than the cursor (in ULID order), then receives any
    /// future live publishes.
    ///
    /// If `since` is `None`, only future publishes are delivered.
    pub async fn subscribe(
        &self,
        workspace_id: &WorkspaceId,
        topic: &str,
        since: Option<String>,
    ) -> Result<broadcast::Receiver<Envelope>, ActantError> {
        if topic.is_empty() {
            return Err(ActantError::InvalidInput("topic is required".into()));
        }
        let key = Key {
            workspace_id: workspace_id.as_str().to_string(),
            topic: topic.to_string(),
        };

        // Get (or create) the live broadcast channel. We acquire the *live*
        // subscriber BEFORE running the backfill query so any publish that
        // races with us is captured on the live tail and won't be lost.
        let tx = {
            let mut guard = self.inner.channels.write().await;
            guard
                .entry(key.clone())
                .or_insert_with(|| {
                    let (tx, _) = broadcast::channel(self.inner.capacity);
                    tx
                })
                .clone()
        };
        let live_rx = tx.subscribe();

        let Some(cursor) = since else {
            return Ok(live_rx);
        };

        // Build a fresh channel that funnels backfill first, then bridges
        // the live broadcast. We use a second broadcast channel so the
        // returned receiver has the same type as the live-only path; the
        // bridge task forwards live messages until the channel closes.
        let (bridge_tx, bridge_rx) = broadcast::channel(self.inner.capacity);
        let backfill = self.replay_since(workspace_id, topic, &cursor).await?;
        for env in backfill {
            // Backfilled rows always succeed because the receiver hasn't
            // been polled yet; if the buffer overflows here the consumer
            // gets a Lagged on the first recv, which is the documented
            // contract for any broadcast receiver.
            let _ = bridge_tx.send(env);
        }
        // Spawn a forwarder that bridges live → bridge until the
        // subscriber drops bridge_rx.
        let bridge_tx_clone = bridge_tx.clone();
        let mut live_rx_for_task = live_rx;
        tokio::spawn(async move {
            loop {
                match live_rx_for_task.recv().await {
                    Ok(env) => {
                        if bridge_tx_clone.send(env).is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        drop(bridge_tx); // sole keep-alive is the forwarder's clone
        Ok(bridge_rx)
    }

    /// Replay every persisted envelope for `(workspace_id, topic)` whose
    /// id is strictly greater than `cursor`. Bounded at 10_000 rows; if a
    /// caller is that far behind they should fall back to a full pull
    /// against the ledger.
    pub async fn replay_since(
        &self,
        workspace_id: &WorkspaceId,
        topic: &str,
        cursor: &str,
    ) -> Result<Vec<Envelope>, ActantError> {
        let rows = sqlx::query(
            "SELECT id, topic, payload, created_at
             FROM pubsub_message
             WHERE workspace_id = ? AND topic = ? AND id > ?
             ORDER BY id ASC
             LIMIT 10000",
        )
        .bind(workspace_id.as_str())
        .bind(topic)
        .bind(cursor)
        .fetch_all(self.inner.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let id: String = r.try_get("id").map_err(map_sqlx)?;
            let topic: String = r.try_get("topic").map_err(map_sqlx)?;
            let payload_text: String = r.try_get("payload").map_err(map_sqlx)?;
            let ts: String = r.try_get("created_at").map_err(map_sqlx)?;
            let payload: Value = serde_json::from_str(&payload_text)
                .map_err(|e| ActantError::Storage(format!("payload decode: {e}")))?;
            out.push(Envelope {
                id,
                topic,
                payload,
                ts,
            });
        }
        Ok(out)
    }

    /// Snapshot of active topics and subscriber counts.
    pub async fn stats(&self) -> Vec<(String, String, usize)> {
        let guard = self.inner.channels.read().await;
        guard
            .iter()
            .map(|(k, tx)| (k.workspace_id.clone(), k.topic.clone(), tx.receiver_count()))
            .collect()
    }
}

fn map_sqlx(e: sqlx::Error) -> ActantError {
    ActantError::Storage(e.to_string())
}
