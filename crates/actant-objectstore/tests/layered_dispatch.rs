//! Verify that [`Layered`] routes reads by URI scheme and writes to the
//! default backend.

use std::sync::Arc;

use actant_objectstore::{BlobStore, FilesystemStore, Layered, MemoryStore};
use bytes::Bytes;

#[tokio::test]
async fn put_goes_to_default_backend() {
    let dir = tempfile::tempdir().unwrap();
    let default = Arc::new(FilesystemStore::new(dir.path()).unwrap());
    let memstore = Arc::new(MemoryStore::with_id("aux"));
    let layered = Layered::new(default.clone()).with_scheme("mem", memstore.clone());

    let r = layered
        .put("aakey", Bytes::from_static(b"on-disk"))
        .await
        .unwrap();
    assert!(r.uri.starts_with("file://"), "uri = {}", r.uri);
    // The default (filesystem) holds the value; the auxiliary (memory) does not.
    assert!(default.exists("aakey").await.unwrap());
    assert!(!memstore.exists("aakey").await.unwrap());
}

#[tokio::test]
async fn get_routes_by_scheme_then_falls_back_to_default() {
    let dir = tempfile::tempdir().unwrap();
    let default = Arc::new(FilesystemStore::new(dir.path()).unwrap());
    let memstore = Arc::new(MemoryStore::with_id("aux"));
    let layered = Layered::new(default.clone()).with_scheme("mem", memstore.clone());

    // Seed the memory store directly so we can verify scheme dispatch.
    memstore
        .put("memkey", Bytes::from_static(b"in-memory"))
        .await
        .unwrap();

    // mem://aux/memkey must route to the memory store.
    let got = layered.get("mem://aux/memkey").await.unwrap();
    assert_eq!(&got[..], b"in-memory");

    // Bare keys fall back to the default backend.
    default
        .put("aakey", Bytes::from_static(b"on-disk"))
        .await
        .unwrap();
    let got = layered.get("aakey").await.unwrap();
    assert_eq!(&got[..], b"on-disk");
}

#[tokio::test]
async fn unregistered_scheme_falls_back_to_default() {
    let dir = tempfile::tempdir().unwrap();
    let default = Arc::new(FilesystemStore::new(dir.path()).unwrap());
    let layered = Layered::new(default.clone());

    // `s3://` is not registered; Layered must hand it to the default backend,
    // which won't recognise it — and will surface InvalidKey because `s3://…`
    // is not a `file://…` URI nor a safe bare key.
    let err = layered.get("s3://bucket/key").await.unwrap_err();
    assert!(
        matches!(err, actant_objectstore::BlobError::InvalidKey(_)),
        "{err:?}"
    );
}
