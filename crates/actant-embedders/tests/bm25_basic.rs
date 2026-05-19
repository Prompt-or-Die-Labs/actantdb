//! BM25 sanity: index a tiny corpus, query, assert the highest-scoring doc
//! is the one with the most query-token overlap.

use actant_embedders::{Bm25Encoder, SparseEncoder};

fn score(
    query_vec: &actant_embedders::SparseVector,
    doc_vec: &actant_embedders::SparseVector,
) -> f32 {
    query_vec.dot(doc_vec)
}

#[test]
fn most_overlapping_doc_scores_highest() {
    let docs = [
        ("d1", "the quick brown fox jumps over the lazy dog"),
        ("d2", "a fox in socks runs through the box"),
        ("d3", "the dog naps in the sun all day"),
        ("d4", "rust async traits are fun"),
        ("d5", "the brown box of foxes is empty"),
    ];

    let mut enc = Bm25Encoder::new();
    for (id, text) in &docs {
        enc.index_document(id, text);
    }
    enc.finalize();

    let q = enc.encode("quick brown fox");
    let scores: Vec<(usize, f32)> = docs
        .iter()
        .enumerate()
        .map(|(i, (_, text))| (i, score(&q, &enc.encode(text))))
        .collect();

    let best = scores
        .iter()
        .cloned()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap();

    assert_eq!(
        best.0, 0,
        "doc d1 should score highest for query 'quick brown fox'; scores={scores:?}"
    );

    // And the rust-async doc has zero overlap — it must score 0.
    assert!(
        scores[3].1.abs() < f32::EPSILON,
        "doc with no overlap must score ~0, got {}",
        scores[3].1
    );
}

#[test]
fn provider_id_is_stable() {
    let enc = Bm25Encoder::new();
    assert_eq!(enc.provider(), "bm25");
}

#[test]
fn encoding_is_deterministic_across_instances() {
    let mut a = Bm25Encoder::new();
    let mut b = Bm25Encoder::new();
    for (id, t) in [("1", "hello world"), ("2", "hello there friend")] {
        a.index_document(id, t);
        b.index_document(id, t);
    }
    a.finalize();
    b.finalize();
    assert_eq!(a.encode("hello world"), b.encode("hello world"));
}
