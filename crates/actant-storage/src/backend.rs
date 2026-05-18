//! Backend abstraction over the SQLite and Postgres storage handles.
//!
//! Closes the long-standing coupling where higher-level crates (notably
//! `actant-command`) reached for `sqlx::SqlitePool` directly. The substrate
//! now passes a `StorageBackend` instead, which can wrap either the
//! SQLite-backed [`Storage`](crate::Storage) or the Postgres-backed
//! [`PgStorage`](crate::PgStorage).
//!
//! Today only the SQLite variant exposes a working pool accessor; the
//! Postgres variant intentionally returns
//! [`ActantError::NotImplemented`](actant_core::ActantError::NotImplemented)
//! with a pointer to `/specs/11-roadmap.md` Phase 6. The trait shape is the
//! contract; per-backend SQL dialect translation (`?` -> `$N`,
//! `INSERT OR IGNORE` -> `ON CONFLICT DO NOTHING`, `TIMESTAMPTZ` vs
//! ISO-8601 `TEXT`) plus the missing `tool`, `tool_call`,
//! `approval_request`, `memory_candidate`, `memory` tables in
//! `/migrations/pg/` are a follow-up tracked in GAPS.md row #5 (Phase 6
//! cloud / team).

use actant_core::ActantError;
use sqlx::SqlitePool;

use crate::{PgStorage, Storage};

/// Human-readable pointer surfaced inside every `NotImplemented` error
/// raised by the Postgres path. Keeps the deferral discoverable from logs.
pub const PG_NOT_IMPLEMENTED_HINT: &str =
    "postgres command-engine path: schema parity + dialect translation tracked in \
     /specs/11-roadmap.md Phase 6 (and GAPS.md row #5)";

/// Unified storage handle. Cheap to clone (each variant wraps an `Arc`d pool).
#[derive(Debug, Clone)]
pub enum StorageBackend {
    /// SQLite — production path today.
    Sqlite(Storage),
    /// Postgres — wired through the substrate but per-command dialect work
    /// is deferred to Phase 6. Calls that need a SQLite pool surface a
    /// well-named [`ActantError::NotImplemented`].
    Postgres(PgStorage),
}

impl StorageBackend {
    /// Borrow the SQLite pool, or surface a descriptive
    /// [`ActantError::NotImplemented`] when the active backend is Postgres.
    ///
    /// Higher-level crates use this when they need to run hand-rolled SQL
    /// against the SQLite schema. The error variant carries a pointer to
    /// the roadmap entry tracking full PG support.
    pub fn sqlite_pool(&self) -> Result<&SqlitePool, ActantError> {
        match self {
            Self::Sqlite(s) => Ok(s.pool()),
            Self::Postgres(_) => Err(ActantError::NotImplemented(
                PG_NOT_IMPLEMENTED_HINT.to_string(),
            )),
        }
    }

    /// Borrow the SQLite handle, or `None` when the active backend is Postgres.
    pub fn as_sqlite(&self) -> Option<&Storage> {
        match self {
            Self::Sqlite(s) => Some(s),
            Self::Postgres(_) => None,
        }
    }

    /// Borrow the Postgres handle, or `None` when the active backend is SQLite.
    pub fn as_postgres(&self) -> Option<&PgStorage> {
        match self {
            Self::Postgres(p) => Some(p),
            Self::Sqlite(_) => None,
        }
    }

    /// Stable label for diagnostics / metrics.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Sqlite(_) => "sqlite",
            Self::Postgres(_) => "postgres",
        }
    }
}

impl From<Storage> for StorageBackend {
    fn from(s: Storage) -> Self {
        Self::Sqlite(s)
    }
}

impl From<PgStorage> for StorageBackend {
    fn from(p: PgStorage) -> Self {
        Self::Postgres(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StorageConfig;

    #[tokio::test]
    async fn sqlite_backend_exposes_pool() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let backend: StorageBackend = s.into();
        assert_eq!(backend.label(), "sqlite");
        assert!(backend.sqlite_pool().is_ok());
        assert!(backend.as_sqlite().is_some());
        assert!(backend.as_postgres().is_none());
    }
}
