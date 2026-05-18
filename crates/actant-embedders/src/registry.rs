//! Provider registry.
//!
//! Name-keyed lookup over the three trait surfaces (dense embedder, sparse
//! encoder, reranker). [`Registry::with_defaults`] wires the always-on
//! offline-safe providers; feature-gated providers are registered through
//! the `register_*` setters at construction time by the host process.

use std::collections::HashMap;
use std::sync::Arc;

use actant_embed::Embedder;

use crate::rerank::{IdentityReranker, Reranker};
use crate::space::{cross_space_check, SpaceError};
use crate::sparse::{Bm25Encoder, SparseEncoder};
use crate::HashEmbedder;

/// Provider registry. Cheaply cloneable — internals are `Arc`s.
#[derive(Clone, Default)]
pub struct Registry {
    embedders: HashMap<String, Arc<dyn Embedder>>,
    sparse: HashMap<String, Arc<dyn SparseEncoder>>,
    rerankers: HashMap<String, Arc<dyn Reranker>>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("embedders", &self.embedders.keys().collect::<Vec<_>>())
            .field("sparse", &self.sparse.keys().collect::<Vec<_>>())
            .field("rerankers", &self.rerankers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Registry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registry pre-wired with offline-safe defaults:
    ///
    /// * `embedder("hash")` -> [`HashEmbedder`]
    /// * `sparse("bm25")`   -> [`Bm25Encoder`] (unfinalized; the host is
    ///   responsible for calling `index_document` + `finalize` on its own
    ///   instance if it needs corpus-aware weights)
    /// * `reranker("identity")` -> [`IdentityReranker`]
    ///
    /// Feature-gated providers (`fastembed`, `openai`) are *not* wired here
    /// because they require external state (model weights, API keys). The
    /// host registers them via [`Registry::register_embedder`] when ready.
    pub fn with_defaults() -> Self {
        let mut r = Self::new();
        r.register_embedder("hash", Arc::new(HashEmbedder::new()));
        r.register_sparse("bm25", Arc::new(Bm25Encoder::new()));
        r.register_reranker("identity", Arc::new(IdentityReranker::new()));
        r
    }

    /// Register an embedder under `name`.
    pub fn register_embedder(&mut self, name: impl Into<String>, e: Arc<dyn Embedder>) {
        self.embedders.insert(name.into(), e);
    }

    /// Register a sparse encoder under `name`.
    pub fn register_sparse(&mut self, name: impl Into<String>, s: Arc<dyn SparseEncoder>) {
        self.sparse.insert(name.into(), s);
    }

    /// Register a reranker under `name`.
    pub fn register_reranker(&mut self, name: impl Into<String>, r: Arc<dyn Reranker>) {
        self.rerankers.insert(name.into(), r);
    }

    /// Look up an embedder by registry name.
    ///
    /// Notably: `embedder("fastembed:bge-small-en-v1.5")` returns `None`
    /// unless the host has explicitly registered a FastEmbed embedder under
    /// that key (the `fastembed` cargo feature is necessary but not
    /// sufficient — the host still has to register the instance).
    pub fn embedder(&self, name: &str) -> Option<Arc<dyn Embedder>> {
        self.embedders.get(name).cloned()
    }

    /// Look up a sparse encoder.
    pub fn sparse(&self, name: &str) -> Option<Arc<dyn SparseEncoder>> {
        self.sparse.get(name).cloned()
    }

    /// Look up a reranker.
    pub fn reranker(&self, name: &str) -> Option<Arc<dyn Reranker>> {
        self.rerankers.get(name).cloned()
    }

    /// Names of registered embedders (unordered).
    pub fn embedder_names(&self) -> Vec<&str> {
        self.embedders.keys().map(String::as_str).collect()
    }

    /// Reject mixing two embedders whose `provider()` strings disagree. The
    /// retrieval planner uses this before feeding two stores into the same
    /// similarity computation.
    pub fn check_cross_space(
        &self,
        lhs: &str,
        rhs: &str,
    ) -> Result<(), RegistryError> {
        let l = self.embedder(lhs).ok_or_else(|| RegistryError::Unknown(lhs.into()))?;
        let r = self.embedder(rhs).ok_or_else(|| RegistryError::Unknown(rhs.into()))?;
        cross_space_check(l.provider(), r.provider()).map_err(RegistryError::Space)
    }
}

/// Registry-level errors.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// No provider registered under that name.
    #[error("unknown provider: {0}")]
    Unknown(String),
    /// Two providers cannot be mixed without an explicit cross-space adapter.
    #[error(transparent)]
    Space(#[from] SpaceError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn defaults_wire_hash_bm25_identity() {
        let r = Registry::with_defaults();
        assert!(r.embedder("hash").is_some());
        assert!(r.sparse("bm25").is_some());
        assert!(r.reranker("identity").is_some());
    }

    #[test]
    fn fastembed_not_wired_by_default() {
        let r = Registry::with_defaults();
        assert!(r.embedder("fastembed:bge-small-en-v1.5").is_none());
    }
}
