//! actant-embed — abstract interface for embedding providers.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use async_trait::async_trait;

pub use actant_contracts::Embedding;

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
