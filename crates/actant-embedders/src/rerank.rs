//! Rerankers.
//!
//! [`Reranker`] is the trait surface used by the retrieval planner after the
//! hybrid candidate set has been assembled. [`IdentityReranker`] is the
//! always-available no-op shim; it preserves the candidate order, assigns
//! `1.0` to every document, and emits a `reason` string that explains how
//! to enable a real semantic reranker.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// One reranker output row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RerankResult {
    /// Index into the original `docs` slice passed to `rerank`.
    pub idx: usize,
    /// Score in `[0, 1]` by convention; higher is better.
    pub score: f32,
    /// Human-readable explanation. Spec §13 requires this field always be
    /// populated so rerank decisions are auditable.
    pub reason: String,
}

/// Reranker trait. Returns one result per input doc.
#[async_trait]
pub trait Reranker: Send + Sync + 'static {
    /// Stable provider id (e.g. `"identity"`, `"bge-reranker-v2"`).
    fn provider(&self) -> &'static str;
    /// Rerank `docs` against `query`. Implementations must return one row
    /// per input doc and must populate `reason`.
    async fn rerank(&self, query: &str, docs: &[String]) -> Vec<RerankResult>;
}

/// No-op reranker. Preserves input order, assigns score `1.0`, and emits a
/// reason pointing at the real reranker.
#[derive(Debug, Clone, Default)]
pub struct IdentityReranker;

impl IdentityReranker {
    /// New identity reranker.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Reranker for IdentityReranker {
    fn provider(&self) -> &'static str {
        "identity"
    }

    async fn rerank(&self, _query: &str, docs: &[String]) -> Vec<RerankResult> {
        let reason = "no-op identity reranker; install rerank-bge for semantic rerank.";
        docs.iter()
            .enumerate()
            .map(|(idx, _)| RerankResult {
                idx,
                score: 1.0,
                reason: reason.into(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn identity_preserves_order_and_emits_reason() {
        let r = IdentityReranker::new();
        let docs = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let out = r.rerank("anything", &docs).await;
        assert_eq!(out.len(), 3);
        for (i, row) in out.iter().enumerate() {
            assert_eq!(row.idx, i);
            assert!((row.score - 1.0).abs() < f32::EPSILON);
            assert!(!row.reason.is_empty());
        }
    }
}
