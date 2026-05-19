//! End-to-end round-trip tests for the FFI surface.
//!
//! These run against the Rust crate directly (no XCFramework / Swift glue
//! involved) and exercise the same `ActantHandle` methods that the Swift
//! consumer reaches through uniffi-generated bindings.
//!
//! Every test uses an in-memory SQLite database (`store_dir == ""`), so
//! they're safe under the "do NOT run cargo test --workspace; disk crash"
//! directive — nothing touches the filesystem.

use actant_ffi::{ActantHandle, EventRow, FfiError};

#[tokio::test]
async fn open_dispatch_events_round_trip() {
    let handle = ActantHandle::open(
        String::new(), // empty store_dir → in-memory backend
        "ws_default".to_string(),
        "actor_ffi_test".to_string(),
    )
    .await
    .expect("open");

    let outcome = handle
        .dispatch(
            "create_session".to_string(),
            serde_json::json!({ "title": "ffi round trip" }).to_string(),
            None,
        )
        .await
        .expect("dispatch create_session");

    assert!(!outcome.command_id.is_empty());
    assert!(outcome.event_id.is_some());
    let result: serde_json::Value =
        serde_json::from_str(&outcome.result_json).expect("result_json parses");
    let session_id = result
        .get("session_id")
        .and_then(|v| v.as_str())
        .expect("session_id present");

    let events = handle
        .events_since(None, None, 100)
        .await
        .expect("events_since");

    assert_eq!(events.len(), 1, "exactly one event from create_session");
    let ev = &events[0];
    assert_eq!(ev.event_type, "session_created");
    assert_eq!(ev.session_id.as_deref(), Some(session_id));
    // Pending GAPS #42 — these defaults are the documented temporary shape.
    assert_eq!(ev.device_id, "_legacy_");
    assert_eq!(ev.hlc_physical_ms, 0);
    assert_eq!(ev.hlc_logical, 0);
}

#[tokio::test]
async fn ingest_same_event_twice_reports_stub_until_gaps_43() {
    let handle = ActantHandle::open(
        String::new(),
        "ws_default".to_string(),
        "actor_ffi_ingest".to_string(),
    )
    .await
    .expect("open");

    let row = EventRow {
        id: "evt_test_dup".to_string(),
        session_id: None,
        event_type: "test_event".to_string(),
        payload_json: "{}".to_string(),
        payload_hash: "0".repeat(64),
        created_at: "1970-01-01T00:00:00Z".to_string(),
        device_id: "device_ffi_test".to_string(),
        hlc_physical_ms: 1,
        hlc_logical: 0,
    };

    // Both ingest calls return the documented stub error until row #43
    // wires up the real Storage::ingest_events implementation.  The test
    // pins the contract so a silent change to "succeeds and writes" is
    // caught by CI rather than by a confused mobile consumer.
    let first = handle.ingest(vec![row.clone()]).await;
    let second = handle.ingest(vec![row]).await;

    match (first, second) {
        (Err(FfiError::Storage(a)), Err(FfiError::Storage(b))) => {
            assert!(
                a.contains("GAPS #43"),
                "stub message must reference GAPS #43: {a}"
            );
            assert!(
                b.contains("GAPS #43"),
                "stub message must reference GAPS #43: {b}"
            );
        }
        other => panic!("expected stub Storage error twice, got {other:?}"),
    }
}

#[tokio::test]
async fn close_releases_storage_cleanly() {
    let handle = ActantHandle::open(
        String::new(),
        "ws_default".to_string(),
        "actor_ffi_close".to_string(),
    )
    .await
    .expect("open");

    // A single dispatch to prove the handle was actually usable.
    let _ = handle
        .dispatch("create_session".to_string(), "{}".to_string(), None)
        .await
        .expect("dispatch");

    handle.close().await;

    // After close(), the pool is shut; dispatch must surface a Storage
    // error rather than panicking or hanging.
    let post_close = handle
        .dispatch("create_session".to_string(), "{}".to_string(), None)
        .await;

    assert!(
        matches!(post_close, Err(FfiError::Storage(_))),
        "post-close dispatch should yield Storage error, got {post_close:?}",
    );
}
