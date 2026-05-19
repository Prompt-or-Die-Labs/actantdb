//! actant-index — hybrid retrieval over indexed objects.
//!
//! In-memory cosine similarity over `Index`. The previous `VectorStore`
//! trait + deferred `QdrantStore` stub were dropped — single-impl trait
//! was dead weight and the stub returned `anyhow::bail!("integration
//! deferred")` on every method. Reintroduce a trait when a second real
//! backend (LanceDB, pgvector, etc.) actually lands; until then this is
//! a concrete type with a focused API.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_embed::{Embedder, Embedding};
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

/// In-memory vector index. Cosine similarity over a flat `Vec<IndexedItem>`.
/// Reasonable up to ~10k items per index; beyond that, a real ANN backend
/// is the answer (and gets its own dedicated type, not a re-introduced
/// `VectorStore` trait — see the crate-level doc).
#[derive(Debug, Default)]
pub struct Index {
    items: Vec<IndexedItem>,
}

impl Index {
    /// New empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of items stored.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Insert an item. Replaces any existing item with the same `id`.
    pub fn insert(&mut self, item: IndexedItem) {
        if let Some(slot) = self.items.iter_mut().find(|x| x.id == item.id) {
            *slot = item;
        } else {
            self.items.push(item);
        }
    }

    /// Top-k by cosine similarity.
    pub fn search(&self, query: &Embedding, k: usize) -> Vec<(f32, &IndexedItem)> {
        let mut scored: Vec<(f32, &IndexedItem)> = self
            .items
            .iter()
            .map(|it| (cosine(&it.embedding.vector, &query.vector), it))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }

    /// Top-k as owned `Hit` records — convenience for callers who don't
    /// want to borrow the index.
    pub fn search_owned(&self, query: &Embedding, k: usize) -> Vec<Hit> {
        self.search(query, k)
            .into_iter()
            .map(|(score, item)| Hit {
                item: item.clone(),
                score,
            })
            .collect()
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

    #[test]
    fn upsert_replaces_existing_id() {
        let mut idx = Index::new();
        idx.insert(IndexedItem {
            id: "x".into(),
            text: "hello".into(),
            embedding: emb(vec![1.0, 0.0, 0.0]),
        });
        idx.insert(IndexedItem {
            id: "y".into(),
            text: "world".into(),
            embedding: emb(vec![0.0, 1.0, 0.0]),
        });
        assert_eq!(idx.len(), 2);
        // Upsert same id replaces.
        idx.insert(IndexedItem {
            id: "x".into(),
            text: "hello2".into(),
            embedding: emb(vec![1.0, 0.0, 0.0]),
        });
        assert_eq!(idx.len(), 2);
        let hits = idx.search_owned(&emb(vec![0.95, 0.1, 0.0]), 1);
        assert_eq!(hits[0].item.id, "x");
        assert_eq!(hits[0].item.text, "hello2");
    }
}
