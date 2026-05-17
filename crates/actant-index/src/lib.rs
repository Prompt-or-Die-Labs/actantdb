//! actant-index — hybrid retrieval over indexed objects.
//!
//! Phase 1: dense cosine similarity over an in-memory `Index`.
//! Phase 6+: pluggable `VectorStore` trait so the same `Index` API can be
//! backed by Qdrant / LanceDB / pgvector.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_embed::{Embedder, Embedding};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// One indexed object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedItem {
    /// Identifier.
    pub id: String,
    /// Free-text canonical content.
    pub text: String,
    /// Computed embedding.
    pub embedding: Embedding,
}

/// A scored search hit.
#[derive(Debug, Clone)]
pub struct Hit {
    /// Stored item.
    pub item: IndexedItem,
    /// Similarity score.
    pub score: f32,
}

/// Pluggable vector store.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Persist an item.
    async fn upsert(&mut self, item: IndexedItem) -> anyhow::Result<()>;
    /// Top-k by cosine similarity to the query embedding.
    async fn search(&self, query: &Embedding, k: usize) -> anyhow::Result<Vec<Hit>>;
    /// Number of items stored.
    async fn len(&self) -> anyhow::Result<usize>;
    /// Whether the store is empty.
    async fn is_empty(&self) -> anyhow::Result<bool> {
        Ok(self.len().await? == 0)
    }
}

/// Default in-memory backend.
#[derive(Debug, Default)]
pub struct InMemoryStore {
    items: Vec<IndexedItem>,
}

impl InMemoryStore {
    /// New empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl VectorStore for InMemoryStore {
    async fn upsert(&mut self, item: IndexedItem) -> anyhow::Result<()> {
        if let Some(slot) = self.items.iter_mut().find(|x| x.id == item.id) {
            *slot = item;
        } else {
            self.items.push(item);
        }
        Ok(())
    }
    async fn search(&self, query: &Embedding, k: usize) -> anyhow::Result<Vec<Hit>> {
        let mut scored: Vec<Hit> = self
            .items
            .iter()
            .map(|it| Hit {
                item: it.clone(),
                score: cosine(&it.embedding.vector, &query.vector),
            })
            .collect();
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(k);
        Ok(scored)
    }
    async fn len(&self) -> anyhow::Result<usize> {
        Ok(self.items.len())
    }
}

/// Qdrant adapter scaffold. Wraps a configured HTTP endpoint; calls land on
/// the standard Qdrant REST API. Implementations of the trait methods are
/// stubbed for Phase 6; full integration is deferred until a design partner
/// asks for it.
#[derive(Debug, Clone)]
pub struct QdrantStore {
    /// Base URL.
    pub base_url: String,
    /// Collection name.
    pub collection: String,
}

#[async_trait]
impl VectorStore for QdrantStore {
    async fn upsert(&mut self, _item: IndexedItem) -> anyhow::Result<()> {
        anyhow::bail!("QdrantStore::upsert: integration deferred; configure InMemoryStore for now")
    }
    async fn search(&self, _q: &Embedding, _k: usize) -> anyhow::Result<Vec<Hit>> {
        anyhow::bail!("QdrantStore::search: integration deferred")
    }
    async fn len(&self) -> anyhow::Result<usize> {
        anyhow::bail!("QdrantStore::len: integration deferred")
    }
}

/// In-memory index — a convenience facade over `InMemoryStore` for the
/// existing call-sites. Equivalent to constructing an `InMemoryStore` and
/// invoking its trait methods directly.
#[derive(Debug, Default)]
pub struct Index {
    inner: InMemoryStore,
}

impl Index {
    /// New empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an item synchronously (blocks the runtime briefly).
    pub fn insert(&mut self, item: IndexedItem) {
        self.inner.items.push(item);
    }

    /// Top-k by cosine similarity. Synchronous over the in-memory backend.
    pub fn search(&self, query: &Embedding, k: usize) -> Vec<(f32, &IndexedItem)> {
        let mut scored: Vec<(f32, &IndexedItem)> = self
            .inner
            .items
            .iter()
            .map(|it| (cosine(&it.embedding.vector, &query.vector), it))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }

    /// Convenience: embed `text` and store with `id`.
    pub async fn index_text<E: Embedder>(
        &mut self,
        id: impl Into<String>,
        text: impl Into<String>,
        embedder: &E,
    ) -> anyhow::Result<()> {
        let text = text.into();
        let embedding = embedder.embed(&text).await?;
        self.insert(IndexedItem {
            id: id.into(),
            text,
            embedding,
        });
        Ok(())
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na * nb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emb(v: Vec<f32>) -> Embedding {
        Embedding {
            provider: "t".into(),
            model: "t".into(),
            vector: v,
        }
    }

    #[test]
    fn cosine_ranks_obvious() {
        let mut idx = Index::new();
        idx.insert(IndexedItem {
            id: "a".into(),
            text: "x".into(),
            embedding: emb(vec![1.0, 0.0]),
        });
        idx.insert(IndexedItem {
            id: "b".into(),
            text: "y".into(),
            embedding: emb(vec![0.0, 1.0]),
        });
        let r = idx.search(&emb(vec![0.99, 0.05]), 1);
        assert_eq!(r[0].1.id, "a");
    }

    #[tokio::test]
    async fn in_memory_store_round_trip() {
        let mut s = InMemoryStore::new();
        s.upsert(IndexedItem {
            id: "x".into(),
            text: "hello".into(),
            embedding: emb(vec![1.0, 0.0, 0.0]),
        })
        .await
        .unwrap();
        s.upsert(IndexedItem {
            id: "y".into(),
            text: "world".into(),
            embedding: emb(vec![0.0, 1.0, 0.0]),
        })
        .await
        .unwrap();
        assert_eq!(s.len().await.unwrap(), 2);
        // Upsert same id replaces.
        s.upsert(IndexedItem {
            id: "x".into(),
            text: "hello2".into(),
            embedding: emb(vec![1.0, 0.0, 0.0]),
        })
        .await
        .unwrap();
        assert_eq!(s.len().await.unwrap(), 2);
        let hits = s.search(&emb(vec![0.95, 0.1, 0.0]), 1).await.unwrap();
        assert_eq!(hits[0].item.id, "x");
        assert_eq!(hits[0].item.text, "hello2");
    }

    #[tokio::test]
    async fn qdrant_stub_errors_clearly() {
        let q = QdrantStore {
            base_url: "http://localhost:6333".into(),
            collection: "test".into(),
        };
        let r = q.search(&emb(vec![1.0]), 1).await;
        assert!(r.is_err());
    }
}
