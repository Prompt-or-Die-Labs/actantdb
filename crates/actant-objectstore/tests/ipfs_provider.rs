//! IPFS smoke test. Compiled only with `--features ipfs`. Runs against the
//! Kubo HTTP API named in `ACTANTDB_TEST_KUBO_URL`, skipping when unset.

#![cfg(feature = "ipfs")]

use actant_objectstore::{BlobStore, IpfsConfig, IpfsStore};
use bytes::Bytes;

#[tokio::test]
async fn ipfs_roundtrip_when_kubo_is_running() {
    let Ok(url) = std::env::var("ACTANTDB_TEST_KUBO_URL") else {
        eprintln!("skipping ipfs_roundtrip: ACTANTDB_TEST_KUBO_URL unset");
        return;
    };
    let store = IpfsStore::new(IpfsConfig {
        base_url: url,
        gateway: None,
    })
    .expect("IpfsStore::new");

    let body = Bytes::from(format!("hello ipfs from actantdb {}", ulid::Ulid::new()));
    let r = store.put("greeting.txt", body.clone()).await.expect("put");
    assert!(r.uri.starts_with("ipfs://"), "uri = {}", r.uri);

    let got = store.get(&r.uri).await.expect("get");
    assert_eq!(got, body);

    // Existence check via block/stat.
    assert!(store.exists(&r.uri).await.expect("exists"));
}
