//! Identity reranker invariants: order is preserved, every score is 1.0,
//! `reason` is always populated.

use actant_embedders::{IdentityReranker, Reranker};

#[tokio::test]
async fn preserves_order_score_one_reason_non_empty() {
    let r = IdentityReranker::new();
    let docs: Vec<String> = (0..7).map(|i| format!("doc-{i}")).collect();

    let out = r.rerank("anything goes", &docs).await;

    assert_eq!(out.len(), docs.len());
    for (i, row) in out.iter().enumerate() {
        assert_eq!(row.idx, i, "identity must preserve input order");
        assert!(
            (row.score - 1.0).abs() < f32::EPSILON,
            "identity score must be 1.0, got {}",
            row.score
        );
        assert!(!row.reason.trim().is_empty(), "reason must be populated");
        assert!(
            row.reason.contains("identity") || row.reason.contains("rerank"),
            "reason should mention the shim: {}",
            row.reason
        );
    }
}

#[tokio::test]
async fn empty_input_yields_empty_output() {
    let r = IdentityReranker::new();
    let out = r.rerank("q", &[]).await;
    assert!(out.is_empty());
}
