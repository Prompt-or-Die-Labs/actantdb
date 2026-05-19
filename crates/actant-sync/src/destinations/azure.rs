//! Azure destination — talks to Azure Blob Storage via `object_store::azure`.
//!
//! Available when the `azure` feature is enabled. `actant-objectstore` does
//! not currently expose an Azure backend, so this implementation uses the
//! upstream `MicrosoftAzureBuilder` directly.

#![cfg(feature = "azure")]

use std::sync::Arc;

use actant_core::{AgentEvent, EventId, WorkspaceId};
use async_trait::async_trait;
use bytes::Bytes;
use object_store::azure::{MicrosoftAzure, MicrosoftAzureBuilder};
use object_store::path::Path as ObjPath;
use object_store::ObjectStore;

use crate::destination::{cursor_key, key_for, Destination};
use crate::SyncError;

/// Configuration for [`AzureDestination`].
#[derive(Debug, Clone, Default)]
pub struct AzureConfig {
    /// Storage account name.
    pub account: String,
    /// Container name (Azure's bucket equivalent).
    pub container: String,
    /// Access key. When `None`, the builder falls back to standard Azure env
    /// vars / managed identity.
    pub access_key: Option<String>,
    /// Allow plain HTTP (set for local Azurite).
    pub allow_http: bool,
}

impl AzureConfig {
    /// New config for `(account, container)` with defaults from the env.
    pub fn new(account: impl Into<String>, container: impl Into<String>) -> Self {
        Self {
            account: account.into(),
            container: container.into(),
            ..Self::default()
        }
    }
}

/// Azure Blob Storage sync destination.
#[derive(Clone)]
pub struct AzureDestination {
    account: String,
    container: String,
    inner: Arc<MicrosoftAzure>,
}

impl std::fmt::Debug for AzureDestination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureDestination")
            .field("account", &self.account)
            .field("container", &self.container)
            .finish_non_exhaustive()
    }
}

impl AzureDestination {
    /// Construct from an [`AzureConfig`].
    pub fn from_config(config: AzureConfig) -> Result<Self, SyncError> {
        let mut b = MicrosoftAzureBuilder::from_env()
            .with_account(&config.account)
            .with_container_name(&config.container)
            .with_allow_http(config.allow_http);
        if let Some(k) = &config.access_key {
            b = b.with_access_key(k);
        }
        let inner = b.build().map_err(|e| SyncError::Backend {
            backend: "azure".into(),
            message: format!("MicrosoftAzureBuilder: {e}"),
        })?;
        Ok(Self {
            account: config.account,
            container: config.container,
            inner: Arc::new(inner),
        })
    }
}

#[async_trait]
impl Destination for AzureDestination {
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
                    backend: "azure".into(),
                    message: e.to_string(),
                })?;
        }
        let final_id = batch.last().unwrap().id.clone();
        let cursor_body = Bytes::from(final_id.as_str().as_bytes().to_vec());
        self.inner
            .put(&ObjPath::from(cursor_key(workspace_id)), cursor_body.into())
            .await
            .map_err(|e| SyncError::Backend {
                backend: "azure".into(),
                message: e.to_string(),
            })?;
        Ok(Some(final_id))
    }

    async fn cursor(&self, workspace_id: &WorkspaceId) -> Result<Option<EventId>, SyncError> {
        let path = ObjPath::from(cursor_key(workspace_id));
        match self.inner.get(&path).await {
            Ok(got) => {
                let bytes = got.bytes().await.map_err(|e| SyncError::Backend {
                    backend: "azure".into(),
                    message: e.to_string(),
                })?;
                let s = std::str::from_utf8(&bytes)
                    .map_err(|e| SyncError::Backend {
                        backend: "azure".into(),
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
                backend: "azure".into(),
                message: e.to_string(),
            }),
        }
    }

    fn name(&self) -> &str {
        "azure"
    }
}
