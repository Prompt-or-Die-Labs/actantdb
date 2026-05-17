//! actant-embed — abstract interface for embedding providers.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// One embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// Provider id.
    pub provider: String,
    /// Model name.
    pub model: String,
    /// Vector.
    pub vector: Vec<f32>,
}

/// An embedder.
#[async_trait]
pub trait Embedder: Send + Sync + 'static {
    /// Provider identifier.
    fn provider(&self) -> &'static str;
    /// Embedding dimension.
    fn dimension(&self) -> usize;
    /// Embed one text.
    async fn embed(&self, text: &str) -> anyhow::Result<Embedding>;
}
