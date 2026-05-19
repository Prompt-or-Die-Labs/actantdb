//! S3 destination — wraps [`actant_objectstore::S3Store`].
//!
//! Available when the `s3` feature is enabled. Writes one `application/json`
//! object per event to `<workspace>/<YYYY-MM-DD>/<event>.json` and keeps a
//! cursor at `<workspace>/_cursor.txt`. The same layout the filesystem
//! destination uses, just keyed against an S3 bucket.

#![cfg(feature = "s3")]

use actant_core::{AgentEvent, EventId, WorkspaceId};
use actant_objectstore::{BlobStore, S3Config, S3Store};
use async_trait::async_trait;
use bytes::Bytes;

use crate::destination::{cursor_key, key_for, Destination};
use crate::SyncError;

/// S3-compatible destination.
#[derive(Debug, Clone)]
pub struct S3Destination {
    store: S3Store,
}

impl S3Destination {
    /// Construct from a fully-configured [`S3Config`].
    pub fn from_config(config: S3Config) -> Result<Self, SyncError> {
        Ok(Self {
            store: S3Store::from_config(config)?,
        })
    }

    /// Wrap an already-built [`S3Store`] (useful for tests where the bucket
    /// handle is shared with another caller).
    pub fn from_store(store: S3Store) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Destination for S3Destination {
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
        self.store
            .put(&cursor_key(workspace_id), cursor_body)
            .await?;
        Ok(Some(final_id))
    }

    async fn cursor(&self, workspace_id: &WorkspaceId) -> Result<Option<EventId>, SyncError> {
        let key = cursor_key(workspace_id);
        match self.store.get(&key).await {
            Ok(bytes) => {
                let s = std::str::from_utf8(&bytes)
                    .map_err(|e| SyncError::Backend {
                        backend: "s3".into(),
                        message: format!("cursor not UTF-8: {e}"),
                    })?
                    .trim()
                    .to_string();
                if s.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(EventId::from_string(s)))
                }
            }
            Err(actant_objectstore::BlobError::NotFound(_)) => Ok(None),
            Err(e) => Err(SyncError::Backend {
                backend: "s3".into(),
                message: e.to_string(),
            }),
        }
    }

    fn name(&self) -> &str {
        "s3"
    }
}
