//! IPFS destination — wraps [`actant_objectstore::IpfsStore`].
//!
//! Available when the `ipfs` feature is enabled. IPFS addresses content by
//! its hash, so the "key" we hand the store is a logical label only — the
//! returned URI contains the canonical CID. The cursor is kept as a separate
//! IPFS-addressed object whose payload is the last event id; the runner
//! looks it up by re-pinning the same logical label key.
//!
//! Because IPFS stores cannot enumerate by prefix or fetch by arbitrary
//! key (you have to know the CID), the IPFS destination keeps an in-memory
//! `(workspace_id) -> cursor_id` map that is refreshed from the most recent
//! `put` so a running runner can resume across restarts in-process only.
//! For durable cross-process resume against IPFS, the operator should sync
//! the cursor through a separate side channel (e.g. a small filesystem
//! sidecar). This is called out in the README.

#![cfg(feature = "ipfs")]

use std::collections::HashMap;
use std::sync::Mutex;

use actant_core::{AgentEvent, EventId, WorkspaceId};
use actant_objectstore::{BlobStore, IpfsConfig, IpfsStore};
use async_trait::async_trait;
use bytes::Bytes;

use crate::destination::{cursor_key, key_for, Destination};
use crate::SyncError;

/// IPFS sync destination.
pub struct IpfsDestination {
    store: IpfsStore,
    cursors: Mutex<HashMap<String, EventId>>,
}

impl std::fmt::Debug for IpfsDestination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpfsDestination").finish_non_exhaustive()
    }
}

impl IpfsDestination {
    /// Construct from an [`IpfsConfig`].
    pub fn from_config(config: IpfsConfig) -> Result<Self, SyncError> {
        Ok(Self {
            store: IpfsStore::new(config)?,
            cursors: Mutex::new(HashMap::new()),
        })
    }

    /// Construct against the default `http://localhost:5001` Kubo endpoint.
    pub fn local() -> Result<Self, SyncError> {
        Self::from_config(IpfsConfig::default())
    }
}

#[async_trait]
impl Destination for IpfsDestination {
    async fn push(
        &self,
        workspace_id: &WorkspaceId,
        _since_event_id: Option<&EventId>,
        batch: &[AgentEvent],
    ) -> Result<Option<EventId>, SyncError> {
        if batch.is_empty() {
            return Ok(self.cursor(workspace_id).await?);
        }
        for e in batch {
            let key = key_for(workspace_id, e);
            let body = Bytes::from(serde_json::to_vec_pretty(e)?);
            self.store.put(&key, body).await?;
        }
        let final_id = batch.last().unwrap().id.clone();
        let cursor_body = Bytes::from(final_id.as_str().as_bytes().to_vec());
        // Pin the cursor as its own IPFS object so it is recoverable through
        // any operator-side sidecar.
        self.store
            .put(&cursor_key(workspace_id), cursor_body)
            .await?;
        self.cursors
            .lock()
            .unwrap()
            .insert(workspace_id.as_str().to_string(), final_id.clone());
        Ok(Some(final_id))
    }

    async fn cursor(&self, workspace_id: &WorkspaceId) -> Result<Option<EventId>, SyncError> {
        Ok(self
            .cursors
            .lock()
            .unwrap()
            .get(workspace_id.as_str())
            .cloned())
    }

    fn name(&self) -> &str {
        "ipfs"
    }
}
