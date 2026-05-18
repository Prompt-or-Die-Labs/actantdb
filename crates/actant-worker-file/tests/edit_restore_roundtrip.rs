//! Edit / restore round-trip test.
//!
//! Per the work package AC: "A demo file edit + restore round-trip produces
//! a byte-identical file." The current `FileHandler` does not yet expose a
//! `restore` verb; restoration in Phase 1 is performed by re-issuing a
//! write with the original contents. This test exercises exactly that:
//!
//! 1. Write the original bytes via `FileHandler`.
//! 2. Snapshot the original bytes (the worker's job in the full
//!    implementation is to capture `pre_state_artifact_ref`; here we just
//!    grab the bytes out of band, which is equivalent for the round-trip).
//! 3. Issue an edit via `FileHandler` (write different bytes).
//! 4. Restore by re-writing the snapshot via `FileHandler`.
//! 5. Read back via `FileHandler` and assert byte-identical to the original.

use actant_worker_file::FileHandler;
use actant_worker_protocol::Handler;

#[tokio::test]
async fn edit_then_restore_yields_byte_identical_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("roundtrip.txt");

    // Use a payload that includes non-ASCII to make sure we're comparing
    // bytes and not just "looks the same".
    let original: &[u8] = "hello world\nline 2\n\u{1f600} emoji\n".as_bytes();
    let original_str = std::str::from_utf8(original).unwrap();

    // (1) Write original via the handler.
    FileHandler
        .handle(serde_json::json!({
            "path": path.display().to_string(),
            "contents": original_str,
        }))
        .await
        .expect("write original");

    let snapshot = std::fs::read(&path).expect("snapshot original");
    assert_eq!(snapshot, original, "snapshot must match what we wrote");

    // (3) Edit via the handler (different contents).
    FileHandler
        .handle(serde_json::json!({
            "path": path.display().to_string(),
            "contents": "completely different content",
        }))
        .await
        .expect("write edited");
    let edited = std::fs::read(&path).expect("read edited");
    assert_ne!(edited, original, "edit must actually change the bytes");

    // (4) Restore by re-writing the snapshot via the handler.
    let restore_str = std::str::from_utf8(&snapshot).unwrap();
    FileHandler
        .handle(serde_json::json!({
            "path": path.display().to_string(),
            "contents": restore_str,
        }))
        .await
        .expect("write restore");

    // (5) Read back via the handler and assert byte-identical to original.
    let read_back = FileHandler
        .handle(serde_json::json!({
            "path": path.display().to_string(),
        }))
        .await
        .expect("read after restore");
    let read_str = read_back["contents"].as_str().expect("contents field");
    assert_eq!(
        read_str.as_bytes(),
        original,
        "round-trip must be byte-identical"
    );

    // Belt-and-braces: also assert at the FS level (no encoding round-trip
    // through serde_json could have masked corruption).
    let restored_bytes = std::fs::read(&path).expect("read final");
    assert_eq!(
        restored_bytes, original,
        "FS bytes after restore must equal the original snapshot"
    );
}

#[tokio::test]
async fn multiple_edits_each_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("multi.txt");

    let versions: &[&str] = &[
        "v1",
        "v2 with more text",
        "v3",
        "v4 \u{00e9}\u{00e8}\u{00ea}",
        "",
    ];

    // Write each version, snapshot, edit to "garbage", restore, assert.
    for v in versions {
        FileHandler
            .handle(serde_json::json!({
                "path": path.display().to_string(),
                "contents": v,
            }))
            .await
            .unwrap();
        let snap = std::fs::read(&path).unwrap();
        FileHandler
            .handle(serde_json::json!({
                "path": path.display().to_string(),
                "contents": "GARBAGE",
            }))
            .await
            .unwrap();
        FileHandler
            .handle(serde_json::json!({
                "path": path.display().to_string(),
                "contents": std::str::from_utf8(&snap).unwrap(),
            }))
            .await
            .unwrap();
        let after = std::fs::read(&path).unwrap();
        assert_eq!(after, snap, "round-trip mismatch for version `{v}`");
    }
}
