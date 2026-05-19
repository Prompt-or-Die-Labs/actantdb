//! [`Layered`] — a [`BlobStore`] router that dispatches reads by URI scheme.
//!
//! Writes always go to the default backend (the one passed at construction).
//! Reads, deletes, exists, and presign attempts parse `key_or_uri` as a URL;
//! if a scheme matches a registered backend, the call is forwarded to that
//! backend; otherwise it falls through to the default.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use url::Url;

use crate::{BlobError, BlobRef, BlobResult, BlobStore};

/// Scheme-dispatching wrapper.
///
/// ```no_run
/// use actant_objectstore::{Layered, MemoryStore};
/// use std::sync::Arc;
/// let fs_default = Arc::new(MemoryStore::with_id("default"));
/// let mem_aux = Arc::new(MemoryStore::with_id("aux"));
/// let layered = Layered::new(fs_default).with_scheme("mem", mem_aux.clone());
/// # let _ = layered;
/// ```
pub struct Layered {
    default: Arc<dyn BlobStore>,
    by_scheme: HashMap<String, Arc<dyn BlobStore>>,
}

impl std::fmt::Debug for Layered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Layered")
            .field("schemes", &self.by_scheme.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Layered {
    /// Wrap `default` as the write target and the fallback read target.
    pub fn new(default: Arc<dyn BlobStore>) -> Self {
        Self {
            default,
            by_scheme: HashMap::new(),
        }
    }

    /// Register `store` to handle URIs whose scheme is `scheme`.
    pub fn with_scheme(mut self, scheme: impl Into<String>, store: Arc<dyn BlobStore>) -> Self {
        self.by_scheme.insert(scheme.into(), store);
        self
    }

    /// Pick the backend for a given key-or-URI. Returns `(store, normalised)`
    /// where `normalised` is what to forward (URI for routed schemes; original
    /// for the default).
    fn route<'a>(&'a self, key_or_uri: &'a str) -> (&'a Arc<dyn BlobStore>, &'a str) {
        if let Some(scheme) = parse_scheme(key_or_uri) {
            if let Some(store) = self.by_scheme.get(scheme) {
                return (store, key_or_uri);
            }
        }
        (&self.default, key_or_uri)
    }
}

fn parse_scheme(s: &str) -> Option<&str> {
    // url::Url::parse rejects relative paths and unknown schemes inconsistently;
    // we only need the leading scheme. Manual split is more permissive.
    let idx = s.find("://")?;
    let scheme = &s[..idx];
    if scheme.is_empty() {
        return None;
    }
    // Sanity-check via url crate to confirm it's a valid scheme grammar.
    if Url::parse(s).is_ok() || Url::parse(&format!("{scheme}://x")).is_ok() {
        Some(scheme)
    } else {
        None
    }
}

#[async_trait]
impl BlobStore for Layered {
    async fn put(&self, key: &str, body: Bytes) -> BlobResult<BlobRef> {
        // Writes always go to the default; the returned BlobRef carries the
        // canonical URI the default produces, which subsequent reads route on.
        self.default.put(key, body).await
    }

    async fn get(&self, key_or_uri: &str) -> BlobResult<Bytes> {
        let (store, k) = self.route(key_or_uri);
        store.get(k).await
    }

    async fn delete(&self, key_or_uri: &str) -> BlobResult<()> {
        let (store, k) = self.route(key_or_uri);
        store.delete(k).await
    }

    async fn exists(&self, key_or_uri: &str) -> BlobResult<bool> {
        let (store, k) = self.route(key_or_uri);
        store.exists(k).await
    }

    async fn presign_get(
        &self,
        key_or_uri: &str,
        ttl: std::time::Duration,
    ) -> BlobResult<Option<Url>> {
        let (store, k) = self.route(key_or_uri);
        store.presign_get(k, ttl).await
    }
}

impl From<BlobError> for std::io::Error {
    fn from(value: BlobError) -> Self {
        std::io::Error::other(value.to_string())
    }
}
