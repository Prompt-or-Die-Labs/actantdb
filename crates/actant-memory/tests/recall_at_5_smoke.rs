//! Smoke counterpart to `recall_at_5.rs` — proves the recall harness wires
//! end-to-end (fixture load → index build → query → score) on every CI run,
//! independent of the active embedder's semantic quality. Asserts recall ≥ 0.0
//! (the no-op floor) so it never blocks a build.
//!
//! With HashEmbedder + identical-text fixture rows, this measurably yields
//! recall = 1.0 — useful as a debug signal but the contract is just "the
//! pipeline ran without panicking".

mod common;

use common::{build_index, load_fixture, recall_at_k};

#[tokio::test]
async fn recall_harness_runs() {
    let fixture = load_fixture();
    assert!(!fixture.queries.is_empty(), "fixture must have queries");
    let idx = build_index(&fixture).await;
    let recall = recall_at_k(&fixture, &idx, 5).await;
    println!("recall@5 (smoke) = {recall:.3}");
    assert!(
        (0.0..=1.0).contains(&recall),
        "recall must be in [0.0, 1.0]; got {recall}"
    );
}
