//! FastEmbed (ONNX) provider stub.
//!
//! Gated behind the `fastembed` feature. The default CI build never enables
//! this feature — model weights are not vendored. When the feature *is*
//! enabled the user is expected to have FastEmbed's download path
//! configured; instantiation will block on a one-time model download.

use actant_embed::{Embedder, Embedding};
use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Mutex;

/// FastEmbed-backed embedder. Wraps a single [`TextEmbedding`] instance.
///
/// `TextEmbedding::embed` is `&mut self`, so we wrap it in a `Mutex` to keep
/// the embedder `Send + Sync`. Throughput here is single-threaded; high-
/// concurrency consumers should pool multiple `FastEmbedEmbedder`s.
pub struct FastEmbedEmbedder {
    inner: Mutex<TextEmbedding>,
    model_name: &'static str,
    dim: usize,
}

impl FastEmbedEmbedder {
    /// Construct using `BGE-small-en-v1.5` (384-dim, 33M params).
    pub fn bge_small_en_v15() -> anyhow::Result<Self> {
        let opts = InitOptions::new(EmbeddingModel::BGESmallENV15);
        let inner = TextEmbedding::try_new(opts)?;
        Ok(Self {
            inner: Mutex::new(inner),
            model_name: "fastembed:bge-small-en-v1.5",
            dim: 384,
        })
    }
}

#[async_trait]
impl Embedder for FastEmbedEmbedder {
    fn provider(&self) -> &'static str {
        "fastembed"
    }

    fn dimension(&self) -> usize {
        self.dim
    }

    async fn embed(&self, text: &str) -> anyhow::Result<Embedding> {
        // FastEmbed's API is sync + batch-only. Lock, embed the single
        // input, and return the first row.
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| anyhow::anyhow!("fastembed mutex poisoned: {e}"))?;
        let mut out = guard.embed(vec![text.to_string()], None)?;
        let vector = out.pop().ok_or_else(|| {
            anyhow::anyhow!("fastembed returned no rows for input")
        })?;
        Ok(Embedding {
            provider: "fastembed".into(),
            model: self.model_name.into(),
            vector,
        })
    }
}
