//! Cross-space rejection: two embedders with different `provider()` strings
//! cannot be mixed without an explicit cross-space adapter.

use std::sync::Arc;

use actant_embed::{Embedder, Embedding};
use actant_embedders::{cross_space_check, Registry, SpaceError};
use async_trait::async_trait;

/// A second embedder with a *different* provider id but the same dimension
/// as `HashEmbedder`. Identical dimensions are intentionally not enough —
/// the cross-space check is strict on provider identity.
#[derive(Debug, Default)]
struct FakeOther;

#[async_trait]
impl Embedder for FakeOther {
    fn provider(&self) -> &'static str {
        "fake-other"
    }
    fn dimension(&self) -> usize {
        32
    }
    async fn embed(&self, _text: &str) -> anyhow::Result<Embedding> {
        Ok(Embedding {
            provider: "fake-other".into(),
            model: "fake".into(),
            vector: vec![0.0; 32],
        })
    }
}

#[test]
fn cross_space_helper_rejects_distinct_providers() {
    let err = cross_space_check("hash", "fake-other").unwrap_err();
    match err {
        SpaceError::ProviderMismatch { lhs, rhs } => {
            assert_eq!(lhs, "hash");
            assert_eq!(rhs, "fake-other");
        }
    }
}

#[test]
fn cross_space_helper_accepts_matching_providers() {
    assert!(cross_space_check("hash", "hash").is_ok());
}

#[test]
fn registry_rejects_mixing_distinct_providers() {
    let mut r = Registry::with_defaults();
    r.register_embedder("fake-other", Arc::new(FakeOther));

    let err = r
        .check_cross_space("hash", "fake-other")
        .expect_err("mixing hash and fake-other must be rejected");
    let s = format!("{err}");
    assert!(s.contains("cross-space mismatch"), "msg was: {s}");
}

#[test]
fn registry_accepts_same_provider() {
    let r = Registry::with_defaults();
    r.check_cross_space("hash", "hash")
        .expect("same provider must be accepted");
}

#[test]
fn registry_unknown_provider_is_error() {
    let r = Registry::with_defaults();
    let err = r.check_cross_space("hash", "ghost").unwrap_err();
    assert!(format!("{err}").contains("unknown provider"));
}
