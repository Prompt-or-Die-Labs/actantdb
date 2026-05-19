//! Storage-level tests for the incremental-backup helpers.
//!
//! Closes the storage portion of GAPS.md row #21.

use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use tempfile::tempdir;

async fn open_file(path: &std::path::Path) -> Storage {
    Storage::open(StorageConfig::file(path))
        .await
        .expect("open")
}

async fn make_workspace(s: &Storage, name: &str) -> WorkspaceId {
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: name.into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.expect("insert ws");
    ws.id
}

#[tokio::test]
async fn last_lsn_starts_at_zero() {
    let dir = tempdir().unwrap();
    let s = open_file(&dir.path().join("a.sqlite")).await;
    assert_eq!(s.last_lsn().await.unwrap(), 0);
}

#[tokio::test]
async fn wal_frames_since_increments_lsn() {
    let dir = tempdir().unwrap();
    let s = open_file(&dir.path().join("a.sqlite")).await;
    make_workspace(&s, "one").await;

    let inc1 = s.wal_frames_since(0).await.unwrap();
    assert_eq!(inc1.previous_lsn, 0);
    assert_eq!(inc1.lsn, 1);
    assert_eq!(s.last_lsn().await.unwrap(), 1);

    make_workspace(&s, "two").await;
    let inc2 = s.wal_frames_since(1).await.unwrap();
    assert_eq!(inc2.previous_lsn, 1);
    assert_eq!(inc2.lsn, 2);
}

#[tokio::test]
async fn wal_frames_since_rejects_out_of_order_from_lsn() {
    let dir = tempdir().unwrap();
    let s = open_file(&dir.path().join("a.sqlite")).await;
    let err = s.wal_frames_since(7).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("expected LSN 7"), "got: {msg}");
}

#[tokio::test]
async fn apply_wal_frames_round_trips_writes() {
    let src_dir = tempdir().unwrap();
    let src_path = src_dir.path().join("src.sqlite");
    let src = open_file(&src_path).await;
    let ws_id = make_workspace(&src, "rolled-forward").await;

    // Capture WAL after that single insert.
    let inc = src.wal_frames_since(0).await.unwrap();
    assert!(!inc.bytes.is_empty(), "WAL should have bytes after writes");

    // Build a fresh target from scratch.
    let dst_dir = tempdir().unwrap();
    let dst_path = dst_dir.path().join("dst.sqlite");
    let dst = open_file(&dst_path).await;
    assert!(dst.get_workspace(&ws_id).await.unwrap().is_none());

    // Apply.
    dst.apply_wal_frames(&inc).await.unwrap();
    assert_eq!(dst.last_lsn().await.unwrap(), 1);
    let got = dst
        .get_workspace(&ws_id)
        .await
        .unwrap()
        .expect("workspace landed");
    assert_eq!(got.name, "rolled-forward");
}

#[tokio::test]
async fn apply_rejects_out_of_chain_increment() {
    let dir = tempdir().unwrap();
    let s = open_file(&dir.path().join("a.sqlite")).await;

    let bad = actant_storage::WalIncrement {
        lsn: 5,
        previous_lsn: 4,
        bytes: vec![],
    };
    let err = s.apply_wal_frames(&bad).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("chains from LSN 4"), "got: {msg}");
}
