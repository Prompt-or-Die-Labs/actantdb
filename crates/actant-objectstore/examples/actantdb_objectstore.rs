//! `actantdb-objectstore` — a put/get/presign roundtrip against the in-memory
//! [`MemoryStore`]. Run with `cargo run -p actant-objectstore --example actantdb-objectstore`.

use std::time::Duration;

use actant_objectstore::{BlobStore, MemoryStore};
use bytes::Bytes;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = MemoryStore::with_id("demo");
    let body = Bytes::from_static(b"hello, actantdb object store");

    println!("put …");
    let r = store.put("greeting.txt", body.clone()).await?;
    println!("  uri          = {}", r.uri);
    println!("  size         = {}", r.size);
    println!("  content_hash = {}", r.content_hash);

    println!("get …");
    let fetched = store.get(&r.uri).await?;
    assert_eq!(fetched, body);
    println!(
        "  fetched {} bytes — matches written payload",
        fetched.len()
    );

    println!("presign …");
    let url = store.presign_get(&r.uri, Duration::from_secs(60)).await?;
    println!("  presigned = {url:?} (MemoryStore does not presign)");

    println!("delete …");
    store.delete(&r.uri).await?;
    let after = store.exists(&r.uri).await?;
    assert!(!after, "object should not exist after delete");
    println!("  exists after delete = {after}");

    println!("ok");
    Ok(())
}
