//! End-to-end check of [`FilesystemStore`]: put → exists → get → delete →
//! exists. Uses a `tempfile::TempDir` so each test run is isolated.

use actant_objectstore::{is_safe_key, BlobError, BlobStore, FilesystemStore};
use bytes::Bytes;
use std::time::Duration;

#[tokio::test]
async fn put_then_get_roundtrips_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemStore::new(dir.path()).unwrap();
    let body = Bytes::from_static(b"hello world");
    let r = store.put("ab1234", body.clone()).await.unwrap();
    assert!(r.uri.starts_with("file://"), "uri = {}", r.uri);
    assert_eq!(r.size, body.len() as u64);
    assert_eq!(
        r.content_hash,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
    let got = store.get(&r.uri).await.unwrap();
    assert_eq!(got, body);
}

#[tokio::test]
async fn put_shards_by_two_char_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemStore::new(dir.path()).unwrap();
    let r = store
        .put("deadbeef", Bytes::from_static(b"x"))
        .await
        .unwrap();
    // The on-disk layout must be <root>/de/deadbeef — confirm by reading the
    // file directly through std::fs.
    let expected = store.root().join("de").join("deadbeef");
    assert!(
        expected.is_file(),
        "expected {} to exist",
        expected.display()
    );
    // Sanity-check the URI references the same path.
    assert!(r.uri.contains("/de/deadbeef"), "uri = {}", r.uri);
}

#[tokio::test]
async fn exists_returns_true_after_put_and_false_after_delete() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemStore::new(dir.path()).unwrap();
    assert!(!store.exists("aabbcc").await.unwrap());
    store.put("aabbcc", Bytes::from_static(b"x")).await.unwrap();
    assert!(store.exists("aabbcc").await.unwrap());
    store.delete("aabbcc").await.unwrap();
    assert!(!store.exists("aabbcc").await.unwrap());
}

#[tokio::test]
async fn delete_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemStore::new(dir.path()).unwrap();
    // Deleting something that never existed is a no-op, not an error.
    store.delete("notthere").await.unwrap();
}

#[tokio::test]
async fn get_missing_returns_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemStore::new(dir.path()).unwrap();
    let err = store.get("missing").await.unwrap_err();
    assert!(matches!(err, BlobError::NotFound(_)), "{err:?}");
}

#[tokio::test]
async fn unsafe_keys_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemStore::new(dir.path()).unwrap();
    for bad in [
        "",
        "..",
        "../etc/passwd",
        "a/b",
        "/abs",
        ".hidden",
        "with space",
    ] {
        assert!(!is_safe_key(bad), "{bad:?} should be unsafe");
        let err = store.put(bad, Bytes::from_static(b"x")).await.unwrap_err();
        assert!(matches!(err, BlobError::InvalidKey(_)), "{err:?}");
    }
}

#[tokio::test]
async fn presign_get_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let store = FilesystemStore::new(dir.path()).unwrap();
    store.put("aakey", Bytes::from_static(b"x")).await.unwrap();
    let url = store
        .presign_get("aakey", Duration::from_secs(10))
        .await
        .unwrap();
    assert!(url.is_none(), "filesystem URIs are not presignable");
}
