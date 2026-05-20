//! Storage-level tests for the incremental-backup helpers.
//!
//! Closes the storage portion of GAPS.md row #21.

use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use tempfile::tempdir;

async fn open_file(path: &std::path::Path) -> Storage {
    let mut config = StorageConfig::file(path);
    config.max_connections = 1;
    Storage::open(config).await.expect("open")
}

async fn open_file_no_migrate(path: &std::path::Path) -> Storage {
    let mut config = StorageConfig::file(path);
    config.apply_migrations = false;
    config.max_connections = 1;
    Storage::open(config).await.expect("open")
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

    // Checkpoint migrations to LSN 1 on src.
    let _ = src.wal_frames_since(0).await.unwrap();
    // Flush the backup state update (which happened after truncate) to the main database file.
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(src.pool())
        .await
        .unwrap();

    // Copy the database file at LSN 1 to the destination, so that the
    // destination database file has the EXACT same binary header/salts.
    let dst_dir = tempdir().unwrap();
    let dst_path = dst_dir.path().join("dst.sqlite");
    std::fs::copy(&src_path, &dst_path).unwrap();

    let ws_id = make_workspace(&src, "rolled-forward").await;

    // Capture WAL after that single insert (LSN 1 -> 2).
    let inc = src.wal_frames_since(1).await.unwrap();
    println!("inc.bytes len: {}", inc.bytes.len());
    assert!(!inc.bytes.is_empty(), "WAL should have bytes after writes");

    // Open dst (which is a clone of src at LSN 1).
    let dst = open_file(&dst_path).await;
    println!(
        "dst last_lsn before apply: {}",
        dst.last_lsn().await.unwrap()
    );
    assert_eq!(dst.last_lsn().await.unwrap(), 1);
    assert!(dst.get_workspace(&ws_id).await.unwrap().is_none());

    // Drop and reopen dst to close all active connections/locks before applying
    let pool = dst.pool().clone();
    drop(dst);
    pool.close().await;
    let dst = open_file_no_migrate(&dst_path).await;

    // Apply (LSN 1 -> 2).
    dst.apply_wal_frames(&inc).await.unwrap();
    println!(
        "dst last_lsn after apply: {}",
        dst.last_lsn().await.unwrap()
    );
    assert_eq!(dst.last_lsn().await.unwrap(), 2);

    // Reopen dst to verify that recovery and checkpoint persisted to the database file.
    drop(dst);
    let dst = open_file(&dst_path).await;

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
