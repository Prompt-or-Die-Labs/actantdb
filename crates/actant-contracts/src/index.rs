//! Retrieval index contract types and deterministic in-memory search helpers.

use std::collections::{BTreeSet, HashMap};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Embedding;

/// One indexed object.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantIndexedItem {
    /// Identifier.
    pub id: String,
    /// Free-text canonical content.
    pub text: String,
    /// Computed embedding.
    pub embedding: Embedding,
}

/// A scored dense search hit.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantHit {
    /// Stored item.
    pub item: ActantIndexedItem,
    /// Similarity score.
    pub score: f32,
}

/// Retrieval mode used by [`ActantIndex::search_with_options`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActantSearchMode {
    /// Dense cosine similarity only.
    Dense,
    /// Dense + sparse lexical scoring.
    Hybrid,
    /// Dense + sparse + entity graph expansion.
    Graph,
    /// Graph mode plus local deterministic rerank.
    Deep,
}

impl Default for ActantSearchMode {
    fn default() -> Self {
        Self::Hybrid
    }
}

/// Search options for hybrid retrieval.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantSearchOptions {
    /// Retrieval mode.
    pub mode: ActantSearchMode,
    /// Maximum number of results returned.
    pub top_k: usize,
    /// Dense cosine score weight.
    pub dense_weight: f32,
    /// Sparse lexical score weight.
    pub sparse_weight: f32,
    /// Entity graph expansion score weight.
    pub graph_weight: f32,
    /// Apply local deterministic rerank after candidate fusion.
    pub rerank: bool,
}

impl Default for ActantSearchOptions {
    fn default() -> Self {
        Self {
            mode: ActantSearchMode::Hybrid,
            top_k: 10,
            dense_weight: 0.65,
            sparse_weight: 0.30,
            graph_weight: 0.20,
            rerank: true,
        }
    }
}

/// Scored hybrid retrieval hit.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantSearchHit {
    /// Stored item.
    pub item: ActantIndexedItem,
    /// Dense cosine component.
    pub dense_score: f32,
    /// Sparse lexical component.
    pub sparse_score: f32,
    /// Entity graph component.
    pub graph_score: f32,
    /// Rerank component.
    pub rerank_score: f32,
    /// Final fused score.
    pub score: f32,
}

/// Relation between two known entities.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantEntityRelation {
    /// Source entity label.
    pub source_entity: String,
    /// Relation type label.
    pub relation_type: String,
    /// Target entity label.
    pub target_entity: String,
    /// Confidence in the relation, `0.0..=1.0`.
    pub confidence: f32,
}

/// In-memory retrieval index over embedded objects.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ActantIndex {
    items: Vec<ActantIndexedItem>,
    item_entities: HashMap<String, BTreeSet<String>>,
    relations: Vec<ActantEntityRelation>,
}

impl ActantIndex {
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
    pub fn insert(&mut self, item: ActantIndexedItem) {
        if let Some(slot) = self.items.iter_mut().find(|x| x.id == item.id) {
            *slot = item;
        } else {
            self.items.push(item);
        }
    }

    /// Associate an indexed item with an entity label.
    pub fn add_entity(&mut self, item_id: impl Into<String>, entity: impl Into<String>) {
        self.item_entities
            .entry(item_id.into())
            .or_default()
            .insert(normalize_entity(&entity.into()));
    }

    /// Add an entity relation used by graph-mode expansion.
    pub fn add_relation(&mut self, relation: ActantEntityRelation) {
        self.relations.push(ActantEntityRelation {
            source_entity: normalize_entity(&relation.source_entity),
            relation_type: relation.relation_type,
            target_entity: normalize_entity(&relation.target_entity),
            confidence: relation.confidence.clamp(0.0, 1.0),
        });
    }

    /// Top-k by cosine similarity.
    pub fn search(&self, query: &Embedding, k: usize) -> Vec<(f32, &ActantIndexedItem)> {
        let mut scored: Vec<(f32, &ActantIndexedItem)> = self
            .items
            .iter()
            .map(|it| (cosine(&it.embedding.vector, &query.vector), it))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }

    /// Top-k as owned records.
    pub fn search_owned(&self, query: &Embedding, k: usize) -> Vec<ActantHit> {
        self.search(query, k)
            .into_iter()
            .map(|(score, item)| ActantHit {
                item: item.clone(),
                score,
            })
            .collect()
    }

    /// Search using dense, sparse, graph, and rerank components.
    pub fn search_with_options(
        &self,
        query: &Embedding,
        query_text: &str,
        options: ActantSearchOptions,
    ) -> Vec<ActantSearchHit> {
        let mut hits = self.score_candidates(query, query_text, &options);
        hits.sort_by(score_order);
        hits.truncate(options.top_k);
        hits
    }

    fn score_candidates(
        &self,
        query: &Embedding,
        query_text: &str,
        options: &ActantSearchOptions,
    ) -> Vec<ActantSearchHit> {
        let query_terms = tokenize(query_text);
        let graph_terms = query_entities(&query_terms, &self.relations);
        let use_sparse = matches!(
            options.mode,
            ActantSearchMode::Hybrid | ActantSearchMode::Graph | ActantSearchMode::Deep
        );
        let use_graph = matches!(
            options.mode,
            ActantSearchMode::Graph | ActantSearchMode::Deep
        );
        let use_rerank = options.rerank || matches!(options.mode, ActantSearchMode::Deep);

        self.items
            .iter()
            .map(|item| {
                let dense_score = cosine(&item.embedding.vector, &query.vector);
                let sparse_score = if use_sparse {
                    sparse_match_score(&query_terms, &tokenize(&item.text))
                } else {
                    0.0
                };
                let graph_score = if use_graph {
                    self.graph_score(item, &graph_terms)
                } else {
                    0.0
                };
                let rerank_score = if use_rerank {
                    local_rerank_score(query_text, item, sparse_score, graph_score)
                } else {
                    0.0
                };
                let mut score = dense_score * options.dense_weight;
                score += sparse_score * options.sparse_weight;
                score += graph_score * options.graph_weight;
                if use_rerank {
                    score += rerank_score * 0.15;
                }
                ActantSearchHit {
                    item: item.clone(),
                    dense_score,
                    sparse_score,
                    graph_score,
                    rerank_score,
                    score,
                }
            })
            .collect()
    }

    fn graph_score(&self, item: &ActantIndexedItem, query_entities: &BTreeSet<String>) -> f32 {
        if query_entities.is_empty() {
            return 0.0;
        }
        let Some(item_entities) = self.item_entities.get(&item.id) else {
            return 0.0;
        };
        let mut score = 0.0_f32;
        for query_entity in query_entities {
            if item_entities.contains(query_entity) {
                score = score.max(1.0);
                continue;
            }
            for relation in &self.relations {
                let connects = (&relation.source_entity == query_entity
                    && item_entities.contains(&relation.target_entity))
                    || (&relation.target_entity == query_entity
                        && item_entities.contains(&relation.source_entity));
                if connects {
                    score = score.max(relation.confidence);
                }
            }
        }
        score
    }
}

fn score_order(a: &ActantSearchHit, b: &ActantSearchHit) -> std::cmp::Ordering {
    b.score
        .partial_cmp(&a.score)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.item.id.cmp(&b.item.id))
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

fn tokenize(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter_map(|raw| {
            let term = raw.trim().to_lowercase();
            if term.len() < 2 {
                None
            } else {
                Some(term)
            }
        })
        .collect()
}

fn sparse_match_score(query_terms: &BTreeSet<String>, item_terms: &BTreeSet<String>) -> f32 {
    if query_terms.is_empty() || item_terms.is_empty() {
        return 0.0;
    }
    let matched = query_terms
        .iter()
        .filter(|term| item_terms.contains(*term))
        .count() as f32;
    matched / query_terms.len() as f32
}

fn query_entities(
    query_terms: &BTreeSet<String>,
    relations: &[ActantEntityRelation],
) -> BTreeSet<String> {
    let mut entities = BTreeSet::new();
    for relation in relations {
        if entity_matches_terms(&relation.source_entity, query_terms) {
            entities.insert(relation.source_entity.clone());
        }
        if entity_matches_terms(&relation.target_entity, query_terms) {
            entities.insert(relation.target_entity.clone());
        }
    }
    entities
}

fn entity_matches_terms(entity: &str, query_terms: &BTreeSet<String>) -> bool {
    tokenize(entity)
        .iter()
        .all(|term| query_terms.contains(term))
}

fn local_rerank_score(
    query_text: &str,
    item: &ActantIndexedItem,
    sparse_score: f32,
    graph_score: f32,
) -> f32 {
    let query = query_text.trim().to_lowercase();
    let text = item.text.to_lowercase();
    let phrase = if !query.is_empty() && text.contains(&query) {
        1.0
    } else {
        0.0
    };
    (phrase * 0.45 + sparse_score * 0.40 + graph_score * 0.15).clamp(0.0, 1.0)
}

fn normalize_entity(entity: &str) -> String {
    tokenize(entity).into_iter().collect::<Vec<_>>().join(" ")
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
        let mut idx = ActantIndex::new();
        idx.insert(ActantIndexedItem {
            id: "a".into(),
            text: "x".into(),
            embedding: emb(vec![1.0, 0.0]),
        });
        idx.insert(ActantIndexedItem {
            id: "b".into(),
            text: "y".into(),
            embedding: emb(vec![0.0, 1.0]),
        });
        let r = idx.search(&emb(vec![0.99, 0.05]), 1);
        assert_eq!(r[0].1.id, "a");
    }

    #[test]
    fn upsert_replaces_existing_id() {
        let mut idx = ActantIndex::new();
        idx.insert(ActantIndexedItem {
            id: "x".into(),
            text: "hello".into(),
            embedding: emb(vec![1.0, 0.0, 0.0]),
        });
        idx.insert(ActantIndexedItem {
            id: "y".into(),
            text: "world".into(),
            embedding: emb(vec![0.0, 1.0, 0.0]),
        });
        assert_eq!(idx.len(), 2);
        idx.insert(ActantIndexedItem {
            id: "x".into(),
            text: "hello2".into(),
            embedding: emb(vec![1.0, 0.0, 0.0]),
        });
        assert_eq!(idx.len(), 2);
        let hits = idx.search_owned(&emb(vec![0.95, 0.1, 0.0]), 1);
        assert_eq!(hits[0].item.id, "x");
        assert_eq!(hits[0].item.text, "hello2");
    }

    #[test]
    fn hybrid_search_uses_sparse_terms() {
        let mut idx = ActantIndex::new();
        idx.insert(ActantIndexedItem {
            id: "dense".into(),
            text: "semantic cache invalidation".into(),
            embedding: emb(vec![1.0, 0.0]),
        });
        idx.insert(ActantIndexedItem {
            id: "sparse".into(),
            text: "postgres connection pool timeout retry".into(),
            embedding: emb(vec![0.9, 0.1]),
        });

        let hits = idx.search_with_options(
            &emb(vec![1.0, 0.0]),
            "postgres timeout",
            ActantSearchOptions {
                top_k: 1,
                dense_weight: 0.25,
                sparse_weight: 0.75,
                ..Default::default()
            },
        );

        assert_eq!(hits[0].item.id, "sparse");
        assert!(hits[0].sparse_score > 0.0);
    }

    #[test]
    fn graph_search_expands_related_entities() {
        let mut idx = ActantIndex::new();
        idx.insert(ActantIndexedItem {
            id: "policy".into(),
            text: "permission checks and authority scopes".into(),
            embedding: emb(vec![0.0, 1.0]),
        });
        idx.add_entity("policy", "authority scope");
        idx.add_relation(ActantEntityRelation {
            source_entity: "guard".into(),
            relation_type: "uses".into(),
            target_entity: "authority scope".into(),
            confidence: 0.9,
        });

        let hits = idx.search_with_options(
            &emb(vec![1.0, 0.0]),
            "guard decision",
            ActantSearchOptions {
                mode: ActantSearchMode::Graph,
                top_k: 1,
                dense_weight: 0.1,
                sparse_weight: 0.0,
                graph_weight: 0.9,
                rerank: false,
            },
        );

        assert_eq!(hits[0].item.id, "policy");
        assert!(hits[0].graph_score >= 0.9);
    }
}
