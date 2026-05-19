//! Two distinct [`MemoryStore`]s must not share storage. The test inserts the
//! same key into both and verifies the values are independent.

use actant_objectstore::{BlobStore, MemoryStore};
use bytes::Bytes;

#[tokio::test]
async fn distinct_instances_have_disjoint_state() {
    let a = MemoryStore::with_id("a");
    let b = MemoryStore::with_id("b");

    a.put("k", Bytes::from_static(b"from-a")).await.unwrap();
    b.put("k", Bytes::from_static(b"from-b")).await.unwrap();

    assert_eq!(&a.get("k").await.unwrap()[..], b"from-a");
    assert_eq!(&b.get("k").await.unwrap()[..], b"from-b");

    a.delete("k").await.unwrap();
    assert!(!a.exists("k").await.unwrap());
    // Delete on `a` must not affect `b`.
    assert!(b.exists("k").await.unwrap());
}

#[tokio::test]
async fn clones_share_state_within_one_instance() {
    let store = MemoryStore::with_id("shared");
    let clone = store.clone();

    store.put("k", Bytes::from_static(b"v")).await.unwrap();
    assert!(clone.exists("k").await.unwrap());
    assert_eq!(&clone.get("k").await.unwrap()[..], b"v");
}

#[tokio::test]
async fn cross_store_uri_is_rejected() {
    let a = MemoryStore::with_id("a");
    let b = MemoryStore::with_id("b");
    let r = a.put("k", Bytes::from_static(b"v")).await.unwrap();
    // `a`'s URI is `mem://a/k`; `b` must refuse to serve it.
    let err = b.get(&r.uri).await.unwrap_err();
    assert!(
        matches!(err, actant_objectstore::BlobError::InvalidKey(_)),
        "{err:?}"
    );
}
