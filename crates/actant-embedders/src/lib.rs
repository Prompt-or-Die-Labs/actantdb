//! actant-embedders — embedder, sparse encoder, and reranker providers.
//!
//! The crate ships [`HashEmbedder`] (deterministic SHA-256, 32-dim local
//! default), [`Bm25Encoder`] (pure-Rust sparse encoder), [`IdentityReranker`]
//! (no-op reranker with a populated `reason`), and a [`Registry`] that wires
//! all three under name-keyed lookup. Heavier providers (FastEmbed ONNX,
//! OpenAI HTTP) are feature-gated under `fastembed` and `openai`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_embed::{Embedder, Embedding};
use async_trait::async_trait;
use sha2::{Digest, Sha256};

pub mod providers;
pub mod registry;
pub mod rerank;
pub mod space;
pub mod sparse;

pub use registry::{Registry, RegistryError};
pub use rerank::{IdentityReranker, RerankResult, Reranker};
pub use space::{cross_space_check, SpaceError};
pub use sparse::{Bm25Encoder, SparseEncoder, SparseVector};

/// Deterministic hash-bucket embedder. Useful for offline tests and the
/// local-first default mode (zero network, deterministic across runs).
#[derive(Debug, Clone, Default)]
pub struct HashEmbedder;

impl HashEmbedder {
    /// New embedder.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Embedder for HashEmbedder {
    fn provider(&self) -> &'static str {
        "hash"
    }
    fn dimension(&self) -> usize {
        32
    }
    async fn embed(&self, text: &str) -> anyhow::Result<Embedding> {
        let mut h = Sha256::new();
        h.update(text.as_bytes());
        let digest = h.finalize();
        // 32 bytes -> 32 floats normalized to [-1, 1].
        let vector: Vec<f32> = digest.iter().map(|b| (*b as f32) / 127.5 - 1.0).collect();
        Ok(Embedding {
            provider: "hash".into(),
            model: "sha256-32d".into(),
            vector,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deterministic_dim_32() {
        let e = HashEmbedder::new();
        let a = e.embed("hello").await.unwrap();
        let b = e.embed("hello").await.unwrap();
        assert_eq!(a.vector, b.vector);
        assert_eq!(a.vector.len(), 32);
    }
}
