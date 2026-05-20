//! Incremental backup primitives (GAPS.md #21).
//!
//! ActantDB ships a simple "full snapshot + WAL-bytes increments" backup
//! model. The granularity is the WAL between two checkpoint calls; not
//! individual frames (SQLite's frame layout is an internal binary format
//! that varies by version + page size, and there's no portable parser in
//! the dependency surface here).
//!
//! ## Vocabulary
//!
//! - **Full snapshot.** A consistent copy of the SQLite main file produced
//!   after `wal_checkpoint(TRUNCATE)`.
//! - **WAL frames since `lsn`.** The raw bytes of the `<db>-wal` file
//!   captured after a `wal_checkpoint(PASSIVE)` and immediately preceding
//!   a `wal_checkpoint(TRUNCATE)`. The `lsn` argument is informational —
//!   we record it inside the manifest and the storage's
//!   `actant_backup_state` table so the caller can detect an out-of-order
//!   apply.
//! - **LSN.** Monotonic 64-bit counter persisted in `actant_backup_state`.
//!   Bumped each time a WAL increment is captured. Not the same thing as
//!   SQLite's internal salts.
//!
//! ## Restore contract
//!
//! Restore = copy the full snapshot to the target, then `apply_wal_frames`
//! for each captured increment in LSN order. Same SQLite version + page
//! size between capture and restore is required (call this out in the
//! README). Single-machine RPO is the only thing the v1 design promises.

use std::path::{Path, PathBuf};

use actant_core::ActantError;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::Storage;

/// Captured WAL increment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalIncrement {
    /// LSN assigned to this increment (the value `last_lsn()` returns
    /// after the capture).
    pub lsn: u64,
    /// Previous LSN, for chain-consistency checks at apply time.
    pub previous_lsn: u64,
    /// Captured WAL bytes. Empty `Vec` when the WAL was empty at capture
    /// time (no writes since the previous capture); applying it is a
    /// no-op.
    pub bytes: Vec<u8>,
}

impl Storage {
    /// Highest LSN ever assigned by `wal_frames_since`. Returns `0` on a
    /// fresh database.
    pub async fn last_lsn(&self) -> Result<u64, ActantError> {
        self.ensure_backup_state().await?;
        let row = sqlx::query("SELECT last_lsn FROM actant_backup_state WHERE id = 1")
            .fetch_one(self.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(row.try_get::<i64, _>("last_lsn").unwrap_or(0) as u64)
    }

    /// Capture the WAL since the previous capture (or since DB creation if
    /// `from_lsn == 0`).
    ///
    /// The `from_lsn` argument is the LSN the caller expects to be the
    /// most recent one already on disk; if it doesn't match the persisted
    /// `last_lsn` we surface an `InvalidInput` error so chains can't be
    /// silently rewound.
    pub async fn wal_frames_since(&self, from_lsn: u64) -> Result<WalIncrement, ActantError> {
        self.ensure_backup_state().await?;
        let current = self.last_lsn().await?;
        if from_lsn != current {
            return Err(ActantError::InvalidInput(format!(
                "wal_frames_since: caller expected LSN {from_lsn} but storage is at {current}"
            )));
        }

        // PASSIVE checkpoint flushes committed pages from the WAL into the
        // main file without truncating, leaving the WAL bytes on disk for
        // us to capture.
        sqlx::query("PRAGMA wal_checkpoint(PASSIVE)")
            .execute(self.pool())
            .await
            .map_err(|e| ActantError::Storage(format!("wal_checkpoint(PASSIVE): {e}")))?;

        let bytes = match self.wal_path() {
            Some(path) => std::fs::read(&path).unwrap_or_default(),
            None => Vec::new(),
        };

        // TRUNCATE the WAL now that we've captured its contents. Any new
        // writes will start a fresh WAL that becomes the next increment.
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(self.pool())
            .await
            .map_err(|e| ActantError::Storage(format!("wal_checkpoint(TRUNCATE): {e}")))?;

        let next = current + 1;
        sqlx::query(
            "UPDATE actant_backup_state SET last_lsn = ?, last_captured_at = ? WHERE id = 1",
        )
        .bind(next as i64)
        .bind(actant_core::now_rfc3339())
        .execute(self.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;

        Ok(WalIncrement {
            lsn: next,
            previous_lsn: current,
            bytes,
        })
    }

    /// Apply a previously captured [`WalIncrement`] to the storage. The
    /// caller is responsible for ordering — applying out of order is
    /// rejected.
    ///
    /// Restore flow: open the freshly copied full snapshot via
    /// [`Storage::open`], then call this for each increment in order.
    /// SQLite picks up the staged `<db>-wal` automatically on the next
    /// transaction; we trigger that with an explicit
    /// `wal_checkpoint(TRUNCATE)` so the snapshot file is self-contained
    /// after each step.
    pub async fn apply_wal_frames(&self, frames: &WalIncrement) -> Result<(), ActantError> {
        // Write the WAL bytes first, before any connection is opened/queried.
        // This ensures SQLite will run WAL recovery automatically on the very
        // first query/connection to the database.
        if !frames.bytes.is_empty() {
            let Some(wal_path) = self.wal_path() else {
                return Err(ActantError::Storage(
                    "apply_wal_frames: in-memory storage cannot accept WAL frames".into(),
                ));
            };
            println!("Writing WAL to: {}", wal_path.display());
            std::fs::write(&wal_path, &frames.bytes).map_err(|e| {
                ActantError::Storage(format!("write {}: {}", wal_path.display(), e))
            })?;
            println!(
                "Written WAL size: {}",
                std::fs::metadata(&wal_path).unwrap().len()
            );
        }

        self.ensure_backup_state().await?;
        let current = self.last_lsn().await?;
        if frames.previous_lsn != current {
            // Clean up the WAL file we just wrote to avoid leaving it in an invalid state.
            if !frames.bytes.is_empty() {
                if let Some(wal_path) = self.wal_path() {
                    let _ = std::fs::remove_file(&wal_path);
                }
            }
            let previous_lsn = frames.previous_lsn;
            return Err(ActantError::InvalidInput(format!(
                "apply_wal_frames: increment chains from LSN {previous_lsn} but storage is at {current}"
            )));
        }

        if !frames.bytes.is_empty() {
            // Force SQLite to replay the WAL into the main file and then
            // truncate it. Since this is run after the first connection has
            // performed recovery, the pages are already merged.
            sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(self.pool())
                .await
                .map_err(|e| ActantError::Storage(format!("wal_checkpoint(TRUNCATE): {e}")))?;
        }

        sqlx::query(
            "UPDATE actant_backup_state SET last_lsn = ?, last_captured_at = ? WHERE id = 1",
        )
        .bind(frames.lsn as i64)
        .bind(actant_core::now_rfc3339())
        .execute(self.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Ensure the `actant_backup_state` sidecar table exists. Idempotent.
    async fn ensure_backup_state(&self) -> Result<(), ActantError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS actant_backup_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                last_lsn INTEGER NOT NULL DEFAULT 0,
                last_captured_at TEXT
            )",
        )
        .execute(self.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        sqlx::query("INSERT OR IGNORE INTO actant_backup_state (id, last_lsn) VALUES (1, 0)")
            .execute(self.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Path of the SQLite WAL sidecar file, if this storage is file-backed.
    fn wal_path(&self) -> Option<PathBuf> {
        let main = sqlite_main_path(self.pool())?;
        let mut wal = main.into_os_string();
        wal.push("-wal");
        Some(PathBuf::from(wal))
    }
}

/// Best-effort extraction of the main DB file path from a pool. Returns
/// `None` for `:memory:` connections.
fn sqlite_main_path(pool: &sqlx::SqlitePool) -> Option<PathBuf> {
    let opts = pool.connect_options();
    let filename: &Path = opts.get_filename();
    if filename.as_os_str().is_empty() || filename.as_os_str() == ":memory:" {
        return None;
    }
    Some(filename.to_path_buf())
}

// ---------------------------------------------------------------------------
// Manifest format (the wire format on disk).
// ---------------------------------------------------------------------------

/// Manifest schema version. Bump when the format changes.
pub const MANIFEST_VERSION: u32 = 1;

/// Top-level manifest emitted alongside the backup files. Append-only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Schema version.
    pub version: u32,
    /// Ordered list of entries. Index 0 is the first full snapshot; later
    /// entries are mixed full snapshots + WAL increments.
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    /// New, empty manifest.
    pub fn new() -> Self {
        Self {
            version: MANIFEST_VERSION,
            entries: Vec::new(),
        }
    }

    /// Read a manifest from disk; create an empty one when the file is
    /// absent.
    pub fn read_or_default(path: &Path) -> Result<Self, ActantError> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let bytes = std::fs::read(path)
            .map_err(|e| ActantError::Storage(format!("read {}: {}", path.display(), e)))?;
        let m: Self = serde_json::from_slice(&bytes)
            .map_err(|e| ActantError::Storage(format!("parse manifest: {e}")))?;
        if m.version != MANIFEST_VERSION {
            return Err(ActantError::Storage(format!(
                "unsupported manifest version {} (this build understands {})",
                m.version, MANIFEST_VERSION
            )));
        }
        Ok(m)
    }

    /// Write the manifest atomically (write-then-rename).
    pub fn write(&self, path: &Path) -> Result<(), ActantError> {
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| ActantError::Storage(format!("encode manifest: {e}")))?;
        std::fs::write(&tmp, &bytes)
            .map_err(|e| ActantError::Storage(format!("write {}: {}", tmp.display(), e)))?;
        std::fs::rename(&tmp, path)
            .map_err(|e| ActantError::Storage(format!("rename {}: {}", path.display(), e)))?;
        Ok(())
    }

    /// LSN of the last entry, or `0` when the manifest is empty.
    pub fn last_lsn(&self) -> u64 {
        self.entries.last().map(|e| e.lsn).unwrap_or(0)
    }

    /// LSN of the most recent full snapshot, or `None` when none has been
    /// taken yet.
    pub fn last_full_lsn(&self) -> Option<u64> {
        self.entries
            .iter()
            .rev()
            .find(|e| matches!(e.kind, EntryKind::Full))
            .map(|e| e.lsn)
    }

    /// Timestamp of the most recent full snapshot, or `None`.
    pub fn last_full_taken_at(&self) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|e| matches!(e.kind, EntryKind::Full))
            .map(|e| e.taken_at.as_str())
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

/// One entry in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// `full` or `incremental`.
    pub kind: EntryKind,
    /// Filename (relative to the backup directory).
    pub file: String,
    /// LSN this entry advances the chain to.
    pub lsn: u64,
    /// LSN this entry expects the chain to be at before applying. Equal
    /// to `lsn` for `Full` entries (a full snapshot is self-contained).
    pub previous_lsn: u64,
    /// Hex-encoded sha256 of the file content.
    pub sha256: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// RFC3339 timestamp at which the entry was captured.
    pub taken_at: String,
}

/// Kind of manifest entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// Full snapshot — the SQLite main file at a checkpointed instant.
    Full,
    /// Incremental — a captured WAL bytestream.
    Incremental,
}

/// Hex-encoded sha256 of an arbitrary byte slice. Re-exported so the CLI
/// doesn't have to depend on `actant-core::sha256_hex` directly.
pub fn sha256_hex(bytes: &[u8]) -> String {
    actant_core::sha256_hex(bytes)
}
