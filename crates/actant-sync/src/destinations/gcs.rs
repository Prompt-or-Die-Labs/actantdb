//! GCS destination — talks to Google Cloud Storage via `object_store::gcp`.
//!
//! Available when the `gcs` feature is enabled. `actant-objectstore` does not
//! currently expose a GCS backend, so this implementation uses the upstream
//! `GoogleCloudStorageBuilder` directly. If `actant-objectstore` grows a
//! GCS blob store in the future, this module collapses to a one-line
//! delegation similar to the S3 destination.

#![cfg(feature = "gcs")]

use std::sync::Arc;

use actant_core::{AgentEvent, EventId, WorkspaceId};
use async_trait::async_trait;
use bytes::Bytes;
use object_store::gcp::{GoogleCloudStorage, GoogleCloudStorageBuilder};
use object_store::path::Path as ObjPath;
use object_store::ObjectStore;

use crate::destination::{cursor_key, key_for, Destination};
use crate::SyncError;

/// Configuration for [`GcsDestination`].
#[derive(Debug, Clone, Default)]
pub struct GcsConfig {
    /// GCS bucket name.
    pub bucket: String,
    /// Path to a service-account JSON key file. When `None`, the builder
    /// falls back to standard GCS env vars / ADC.
    pub service_account_path: Option<String>,
    /// Inline service-account JSON (alternative to `service_account_path`).
    pub service_account_key: Option<String>,
}

impl GcsConfig {
    /// New config for `bucket` with defaults from the environment.
    pub fn new(bucket: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            ..Self::default()
        }
    }
}

/// Google Cloud Storage sync destination.
#[derive(Clone)]
pub struct GcsDestination {
    bucket: String,
    inner: Arc<GoogleCloudStorage>,
}

impl std::fmt::Debug for GcsDestination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsDestination")
            .field("bucket", &self.bucket)
            .finish_non_exhaustive()
    }
}

impl GcsDestination {
    /// Construct from a [`GcsConfig`].
    pub fn from_config(config: GcsConfig) -> Result<Self, SyncError> {
        let mut b = GoogleCloudStorageBuilder::from_env().with_bucket_name(&config.bucket);
        if let Some(p) = &config.service_account_path {
            b = b.with_service_account_path(p);
        }
        if let Some(k) = &config.service_account_key {
            b = b.with_service_account_key(k);
        }
        let inner = b.build().map_err(|e| SyncError::Backend {
            backend: "gcs".into(),
            message: format!("GoogleCloudStorageBuilder: {e}"),
        })?;
        Ok(Self {
            bucket: config.bucket,
            inner: Arc::new(inner),
        })
    }
}

#[async_trait]
impl Destination for GcsDestination {
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
            self.inner
                .put(&ObjPath::from(key), body.into())
                .await
                .map_err(|e| SyncError::Backend {
                    backend: "gcs".into(),
                    message: e.to_string(),
                })?;
        }
        let final_id = batch.last().unwrap().id.clone();
        let cursor_body = Bytes::from(final_id.as_str().as_bytes().to_vec());
        self.inner
            .put(&ObjPath::from(cursor_key(workspace_id)), cursor_body.into())
            .await
            .map_err(|e| SyncError::Backend {
                backend: "gcs".into(),
                message: e.to_string(),
            })?;
        Ok(Some(final_id))
    }

    async fn cursor(&self, workspace_id: &WorkspaceId) -> Result<Option<EventId>, SyncError> {
        let path = ObjPath::from(cursor_key(workspace_id));
        match self.inner.get(&path).await {
            Ok(got) => {
                let bytes = got.bytes().await.map_err(|e| SyncError::Backend {
                    backend: "gcs".into(),
                    message: e.to_string(),
                })?;
                let s = std::str::from_utf8(&bytes)
                    .map_err(|e| SyncError::Backend {
                        backend: "gcs".into(),
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
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(e) => Err(SyncError::Backend {
                backend: "gcs".into(),
                message: e.to_string(),
            }),
        }
    }

    fn name(&self) -> &str {
        "gcs"
    }
}
