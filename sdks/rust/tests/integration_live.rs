//! Optional live integration test. Skipped unless `ACTANTDB_TEST_URL` is set.
//!
//! Mirrors the Python SDK's `test_client.py` integration mode — useful in CI
//! against a real `actantdb serve` binary, but not part of the default
//! `cargo test` run.
//!
//! Example:
//!
//! ```bash
//! ACTANTDB_TEST_URL=http://127.0.0.1:4555 cargo test \
//!     -p actantdb-client --test integration_live -- --nocapture
//! ```

use actantdb_client::ActantClient;
use url::Url;

fn live_url() -> Option<Url> {
    std::env::var("ACTANTDB_TEST_URL")
        .ok()
        .and_then(|s| Url::parse(&s).ok())
}

#[tokio::test]
async fn live_healthz_round_trip() {
    let Some(url) = live_url() else {
        eprintln!("skip: ACTANTDB_TEST_URL not set");
        return;
    };
    let client = ActantClient::new(url);
    let h = client.healthz().await.expect("live healthz");
    assert!(h.is_healthy(), "server reported unhealthy: {h:?}");
}

#[tokio::test]
async fn live_metadata_commands_nonempty() {
    let Some(url) = live_url() else {
        eprintln!("skip: ACTANTDB_TEST_URL not set");
        return;
    };
    let client = ActantClient::new(url);
    let names = client.metadata_commands().await.expect("metadata");
    assert!(!names.is_empty(), "server returned no commands");
}
