//! In-memory [`BlobStore`] for tests and embedded use. Each instance owns its
//! own map — two `MemoryStore`s never share state, which the
//! `memory_isolation` integration test relies on.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bytes::Bytes;
use url::Url;

use crate::{sha256_hex, BlobError, BlobRef, BlobResult, BlobStore};

/// In-memory blob store. Cheap to clone (clones share the same map). The
/// identifier in the canonical URI (`mem://<id>/<key>`) is per-instance, so
/// independently constructed stores have disjoint key spaces even if they
/// happen to share keys.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    id: String,
    inner: Arc<Mutex<HashMap<String, Bytes>>>,
}

impl MemoryStore {
    /// Create a fresh, isolated in-memory store. The id is derived from a
    /// monotonic counter; each call produces a distinct namespace.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            id: format!("mem-{n}"),
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a store with an explicit id (useful for deterministic URIs in
    /// tests).
    pub fn with_id(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Per-instance identifier, surfaced in the URI scheme.
    pub fn id(&self) -> &str {
        &self.id
    }

    fn uri_for(&self, key: &str) -> String {
        format!("mem://{}/{}", self.id, key)
    }

    fn key_from<'a>(&self, key_or_uri: &'a str) -> BlobResult<&'a str> {
        if let Some(rest) = key_or_uri.strip_prefix("mem://") {
            let prefix = format!("{}/", self.id);
            if let Some(k) = rest.strip_prefix(&prefix) {
                Ok(k)
            } else {
                Err(BlobError::InvalidKey(format!(
                    "URI {:?} belongs to a different MemoryStore (this one is {:?})",
                    key_or_uri, self.id
                )))
            }
        } else {
            Ok(key_or_uri)
        }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BlobStore for MemoryStore {
    async fn put(&self, key: &str, body: Bytes) -> BlobResult<BlobRef> {
        let size = body.len() as u64;
        let content_hash = sha256_hex(&body);
        let uri = self.uri_for(key);
        self.inner.lock().unwrap().insert(key.to_string(), body);
        Ok(BlobRef {
            uri,
            size,
            content_hash,
        })
    }

    async fn get(&self, key_or_uri: &str) -> BlobResult<Bytes> {
        let key = self.key_from(key_or_uri)?;
        self.inner
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or_else(|| BlobError::NotFound(key.to_string()))
    }

    async fn delete(&self, key_or_uri: &str) -> BlobResult<()> {
        let key = self.key_from(key_or_uri)?;
        self.inner.lock().unwrap().remove(key);
        Ok(())
    }

    async fn exists(&self, key_or_uri: &str) -> BlobResult<bool> {
        let key = self.key_from(key_or_uri)?;
        Ok(self.inner.lock().unwrap().contains_key(key))
    }

    async fn presign_get(
        &self,
        _key_or_uri: &str,
        _ttl: std::time::Duration,
    ) -> BlobResult<Option<Url>> {
        Ok(None)
    }
}
