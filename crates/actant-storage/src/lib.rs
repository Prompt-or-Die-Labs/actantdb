//! actant-storage — SQLite-backed persistence layer for the ActantDB
//! substrate.
//!
//! Loads and applies the migrations under `/migrations/` to an actantdb
//! instance, exposes a thin connection-pool wrapper, and offers convenience
//! repositories for the canonical tables.
//!
//! Source of truth for the schema: `/specs/02-data-model.sql`. The migrations
//! are kept in lockstep with that file.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod backend;
mod backup;
mod postgres;
mod repo;

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use actant_core::ActantError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

pub use backend::{StorageBackend, PG_NOT_IMPLEMENTED_HINT};
pub use backup::{
    sha256_hex as backup_sha256_hex, EntryKind, Manifest, ManifestEntry, WalIncrement,
    MANIFEST_VERSION,
};
pub use postgres::PgStorage;

// Re-export the object-store abstraction so downstream crates have a single
// `use actant_storage::*` import point. The actant-objectstore default-feature
// build is lean (filesystem + memory only); S3 / IPFS remain opt-in.
pub use actant_objectstore::{
    is_safe_key as is_safe_blob_key, sha256_hex as blob_sha256_hex, BlobError, BlobRef, BlobResult,
    BlobStore, FilesystemStore, Layered, MemoryStore,
};

// `repo` extends `Storage` with inherent impls; the module itself doesn't
// export new names.
#[allow(unused_imports)]
use repo as _repo;

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_initial",
        include_str!("../../../migrations/0001_initial.sql"),
    ),
    (
        "0002_extended_primitives",
        include_str!("../../../migrations/0002_extended_primitives.sql"),
    ),
    (
        "0003_ai_native_and_reliability",
        include_str!("../../../migrations/0003_ai_native_and_reliability.sql"),
    ),
    (
        "0004_auth",
        include_str!("../../../migrations/0004_auth.sql"),
    ),
];

/// Storage configuration.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// SQLite file path; `:memory:` for in-memory.
    pub db_path: PathBuf,
    /// Apply migrations at open.
    pub apply_migrations: bool,
    /// Optional connection-pool size.
    pub max_connections: u32,
}

impl StorageConfig {
    /// New in-memory configuration suitable for tests.
    pub fn in_memory() -> Self {
        Self {
            db_path: PathBuf::from(":memory:"),
            apply_migrations: true,
            max_connections: 1,
        }
    }

    /// Filesystem-backed configuration.
    pub fn file(path: impl AsRef<Path>) -> Self {
        Self {
            db_path: path.as_ref().to_path_buf(),
            apply_migrations: true,
            max_connections: 8,
        }
    }
}

/// Opened storage handle. Cheap to clone (wraps an `Arc`d pool).
#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
    // Blob store for artifact payloads. Defaults to a filesystem store rooted
    // at `<db parent>/blobs` for file-backed DBs, and a `MemoryStore` for
    // `:memory:`. Injected via `with_blob_store` for production setups.
    pub(crate) blob_store: Arc<dyn BlobStore>,
}

impl std::fmt::Debug for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Storage").field("pool", &self.pool).finish()
    }
}

impl Storage {
    /// Open the SQLite database at `config.db_path`, optionally applying
    /// the bundled migrations.
    pub async fn open(config: StorageConfig) -> Result<Self, ActantError> {
        let path = config.db_path.to_string_lossy().to_string();
        let opts = if path == ":memory:" {
            SqliteConnectOptions::from_str("sqlite::memory:")
                .map_err(|e| ActantError::Storage(e.to_string()))?
                .create_if_missing(true)
        } else {
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
                .synchronous(SqliteSynchronous::Normal)
                .foreign_keys(true)
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_connections)
            .connect_with(opts)
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;

        let blob_store: Arc<dyn BlobStore> = default_blob_store(&config.db_path)
            .map_err(|e| ActantError::Storage(format!("blob store: {e}")))?;

        let storage = Storage { pool, blob_store };
        if config.apply_migrations {
            storage.run_migrations().await?;
        }
        Ok(storage)
    }

    /// Swap in a custom [`BlobStore`] for artifact payloads. Returns `self`
    /// for builder-style use:
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use actant_storage::{Storage, StorageConfig, MemoryStore};
    /// # async fn ex() -> anyhow::Result<()> {
    /// let storage = Storage::open(StorageConfig::in_memory())
    ///     .await?
    ///     .with_blob_store(Arc::new(MemoryStore::new()));
    /// # let _ = storage;
    /// # Ok(()) }
    /// ```
    pub fn with_blob_store(mut self, store: Arc<dyn BlobStore>) -> Self {
        self.blob_store = store;
        self
    }

    /// Access the configured blob store.
    pub fn blob_store(&self) -> Arc<dyn BlobStore> {
        self.blob_store.clone()
    }

    /// Underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Apply every embedded migration that hasn't been recorded yet.
    pub async fn run_migrations(&self) -> Result<(), ActantError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_migration (
                name TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;

        for (name, sql) in MIGRATIONS {
            let already: Option<(String,)> =
                sqlx::query_as("SELECT name FROM schema_migration WHERE name = ?")
                    .bind(name)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| ActantError::Storage(e.to_string()))?;
            if already.is_some() {
                continue;
            }
            apply_sql_script(&self.pool, sql).await?;
            sqlx::query("INSERT INTO schema_migration (name, applied_at) VALUES (?, ?)")
                .bind(name)
                .bind(actant_core::now_rfc3339())
                .execute(&self.pool)
                .await
                .map_err(|e| ActantError::Storage(e.to_string()))?;
        }
        Ok(())
    }

    /// List the names of every applied migration in order.
    pub async fn applied_migrations(&self) -> Result<Vec<String>, ActantError> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM schema_migration ORDER BY name")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(|(n,)| n).collect())
    }
}

/// Apply a multi-statement SQL script. Comments are stripped first so
/// semicolons inside `-- foo` lines don't break the split.
async fn apply_sql_script(pool: &SqlitePool, sql: &str) -> Result<(), ActantError> {
    let cleaned = strip_comments(sql);
    for raw in cleaned.split(';') {
        let stmt = raw.trim();
        if stmt.is_empty() {
            continue;
        }
        sqlx::query(stmt)
            .execute(pool)
            .await
            .map_err(|e| ActantError::Storage(format!("apply `{}`: {}", first_line(stmt), e)))?;
    }
    Ok(())
}

fn strip_comments(s: &str) -> String {
    let mut out = String::new();
    for line in s.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("--") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("").trim()
}

/// Build the default blob store for a given database path. For file-backed
/// databases this is a [`FilesystemStore`] rooted at `<db parent>/blobs`; for
/// `:memory:` it is a [`MemoryStore`].
fn default_blob_store(db_path: &Path) -> std::io::Result<Arc<dyn BlobStore>> {
    if db_path.as_os_str() == ":memory:" {
        return Ok(Arc::new(MemoryStore::new()));
    }
    let blobs_dir = db_path
        .parent()
        .map(|p| p.join("blobs"))
        .unwrap_or_else(|| PathBuf::from("blobs"));
    Ok(Arc::new(FilesystemStore::new(blobs_dir)?))
}
