//! Registry exposes the local-first defaults and produces working
//! embeddings end-to-end.

use actant_embedders::Registry;

#[tokio::test]
async fn defaults_embedder_returns_32_dim_vector() {
    let r = Registry::with_defaults();
    let e = r.embedder("hash").expect("hash embedder is wired by default");
    assert_eq!(e.dimension(), 32);
    let emb = e.embed("the quick brown fox").await.unwrap();
    assert_eq!(emb.vector.len(), 32);
    assert_eq!(emb.provider, "hash");
}

#[test]
fn defaults_expose_bm25_and_identity_reranker() {
    let r = Registry::with_defaults();
    assert!(r.sparse("bm25").is_some(), "bm25 sparse encoder default");
    assert!(
        r.reranker("identity").is_some(),
        "identity reranker default"
    );
}

#[test]
fn unknown_lookup_returns_none() {
    let r = Registry::with_defaults();
    assert!(r.embedder("voyage").is_none());
    assert!(r.sparse("splade-v3").is_none());
    assert!(r.reranker("bge-reranker-v2").is_none());
}

#[test]
fn fastembed_returns_none_without_explicit_registration() {
    // Even when the `fastembed` feature compiles, the host must register the
    // model. The bare `with_defaults()` registry never instantiates one.
    let r = Registry::with_defaults();
    assert!(r.embedder("fastembed:bge-small-en-v1.5").is_none());
}
