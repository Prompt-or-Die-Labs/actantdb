//! Shared helpers for the recall@5 integration tests.

#![allow(dead_code)] // Each test file uses a subset of these helpers.

use std::path::PathBuf;

use actant_embedders::HashEmbedder;
use actant_index::Index;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RecallFixture {
    pub queries: Vec<RecallQuery>,
}

#[derive(Debug, Deserialize)]
pub struct RecallQuery {
    pub query: String,
    pub gold_doc_ids: Vec<String>,
}

pub fn load_fixture() -> RecallFixture {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/recall_queries.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// Build a corpus that pairs every gold doc id with the query text that
/// retrieves it. With the HashEmbedder, identical text → identical embedding,
/// so the harness yields recall = 1.0 and proves the wiring works. When a
/// semantic embedder lands, the corpus body should be replaced with richer
/// per-doc text and the recall threshold re-validated.
pub async fn build_index(fixture: &RecallFixture) -> Index {
    let embedder = HashEmbedder::new();
    let mut idx = Index::new();
    for q in &fixture.queries {
        for gold_id in &q.gold_doc_ids {
            idx.index_text(gold_id.clone(), q.query.clone(), &embedder)
                .await
                .unwrap();
        }
    }
    idx
}

pub async fn recall_at_k(fixture: &RecallFixture, idx: &Index, k: usize) -> f64 {
    let embedder = HashEmbedder::new();
    let mut hits = 0usize;
    let total = fixture.queries.len();
    for q in &fixture.queries {
        let q_emb = embedder_embed(&embedder, &q.query).await;
        let results = idx.search(&q_emb, k);
        let returned_ids: Vec<&str> = results.iter().map(|(_, it)| it.id.as_str()).collect();
        if q.gold_doc_ids
            .iter()
            .any(|g| returned_ids.iter().any(|r| *r == g.as_str()))
        {
            hits += 1;
        }
    }
    hits as f64 / total as f64
}

async fn embedder_embed(e: &HashEmbedder, text: &str) -> actant_embed::Embedding {
    use actant_embed::Embedder;
    e.embed(text).await.unwrap()
}
