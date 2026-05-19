//! S3 smoke test. Compiled only with `--features s3`. Runs against the
//! endpoint named in `ACTANTDB_TEST_S3_BUCKET` / `ACTANTDB_TEST_S3_ENDPOINT`,
//! skipping if either env var is missing (same skip pattern as the Ollama
//! provider test).

#![cfg(feature = "s3")]

use actant_objectstore::{BlobStore, S3Config, S3Store};
use bytes::Bytes;

#[tokio::test]
async fn s3_roundtrip_when_endpoint_is_configured() {
    let Ok(bucket) = std::env::var("ACTANTDB_TEST_S3_BUCKET") else {
        eprintln!("skipping s3_roundtrip: ACTANTDB_TEST_S3_BUCKET unset");
        return;
    };
    let mut cfg = S3Config::new(&bucket);
    cfg.endpoint = std::env::var("ACTANTDB_TEST_S3_ENDPOINT").ok();
    cfg.region = std::env::var("ACTANTDB_TEST_S3_REGION").ok();
    cfg.access_key_id = std::env::var("ACTANTDB_TEST_S3_ACCESS_KEY").ok();
    cfg.secret_access_key = std::env::var("ACTANTDB_TEST_S3_SECRET_KEY").ok();
    cfg.allow_http = std::env::var("ACTANTDB_TEST_S3_ALLOW_HTTP")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let store = S3Store::from_config(cfg).expect("S3Store::from_config");
    let key = format!("actantdb-test/{}", ulid::Ulid::new());
    let body = Bytes::from_static(b"hello s3");

    let r = store.put(&key, body.clone()).await.expect("put");
    assert!(r.uri.starts_with("s3://"));
    let got = store.get(&r.uri).await.expect("get");
    assert_eq!(got, body);
    assert!(store.exists(&r.uri).await.expect("exists"));
    store.delete(&r.uri).await.expect("delete");
    assert!(!store.exists(&r.uri).await.expect("exists after delete"));
}
