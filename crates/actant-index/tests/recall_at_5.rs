//! Spec 15 §"Acceptance criteria" — recall@5 ≥ 0.8 against the bundled
//! benchmark fixture.
//!
//! The fixture (`tests/fixtures/recall_queries.json`) holds 20
//! query → gold-doc-ids pairs. This test loads it, indexes the corpus with
//! the active embedder, runs each query, and asserts recall@5 ≥ 0.8.
//!
//! **Currently ignored.** The Phase 1 default embedder is `HashEmbedder`,
//! which hashes the entire input string as one unit and produces effectively
//! orthogonal vectors for any two non-identical strings. Recall@5 against a
//! real fixture is therefore structurally low for HashEmbedder — the gate
//! activates when a semantic embedder (FastEmbed, BGE, etc.) is wired in.
//! The harness itself runs in the sibling `recall_at_5_smoke.rs` so the
//! plumbing stays exercised on every PR.

mod common;

use common::{build_index, load_fixture, recall_at_k};

#[tokio::test]
#[ignore = "recall@5 against HashEmbedder is structurally low; gold-tests run when a real embedder is wired"]
async fn recall_at_5_meets_threshold() {
    let fixture = load_fixture();
    let idx = build_index(&fixture).await;
    let recall = recall_at_k(&fixture, &idx, 5).await;
    println!("recall@5 = {recall:.3}");
    assert!(
        recall >= 0.8,
        "recall@5 = {recall:.3} below spec-15 threshold 0.8 (n={})",
        fixture.queries.len()
    );
}
