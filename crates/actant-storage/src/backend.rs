//! Backend abstraction over the SQLite and Postgres storage handles.

use actant_core::*;
use bytes::Bytes;
use sqlx::SqlitePool;

use crate::{PgStorage, Storage};

/// Human-readable pointer surfaced when a legacy SQLite-only pool accessor is
/// called against a Postgres backend.
pub const PG_NOT_IMPLEMENTED_HINT: &str =
    "this API exposes the SQLite connection pool; use StorageBackend methods for backend-neutral storage";

/// Unified storage handle. Cheap to clone (each variant wraps an `Arc`d pool).
#[derive(Debug, Clone)]
pub enum StorageBackend {
    /// SQLite storage.
    Sqlite(Storage),
    /// Postgres storage.
    Postgres(PgStorage),
}

impl StorageBackend {
    /// Borrow the SQLite pool, or surface a descriptive
    /// [`ActantError::NotImplemented`] when the active backend is Postgres.
    ///
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

    /// Verify the storage connection.
    pub async fn ping(&self) -> Result<(), ActantError> {
        match self {
            Self::Sqlite(s) => {
                sqlx::query("SELECT 1")
                    .execute(s.pool())
                    .await
                    .map_err(|e| ActantError::Storage(e.to_string()))?;
            }
            Self::Postgres(p) => {
                sqlx::query("SELECT 1")
                    .execute(p.pool())
                    .await
                    .map_err(|e| ActantError::Storage(e.to_string()))?;
            }
        }
        Ok(())
    }

    /// Insert a workspace.
    pub async fn insert_workspace(&self, ws: &Workspace) -> Result<(), ActantError> {
        match self {
            Self::Sqlite(s) => s.insert_workspace(ws).await,
            Self::Postgres(p) => p.insert_workspace(ws).await,
        }
    }

    /// Fetch a workspace by id.
    pub async fn get_workspace(&self, id: &WorkspaceId) -> Result<Option<Workspace>, ActantError> {
        match self {
            Self::Sqlite(s) => s.get_workspace(id).await,
            Self::Postgres(p) => p.get_workspace(id).await,
        }
    }

    /// Insert an actor.
    pub async fn insert_actor(&self, actor: &Actor) -> Result<(), ActantError> {
        match self {
            Self::Sqlite(s) => s.insert_actor(actor).await,
            Self::Postgres(p) => p.insert_actor(actor).await,
        }
    }

    /// Fetch an actor by id.
    pub async fn get_actor(&self, id: &ActorId) -> Result<Option<Actor>, ActantError> {
        match self {
            Self::Sqlite(s) => s.get_actor(id).await,
            Self::Postgres(p) => p.get_actor(id).await,
        }
    }

    /// Insert a session.
    pub async fn insert_session(&self, session: &Session) -> Result<(), ActantError> {
        match self {
            Self::Sqlite(s) => s.insert_session(session).await,
            Self::Postgres(p) => p.insert_session(session).await,
        }
    }

    /// Append an event.
    pub async fn append_event(&self, event: &AgentEvent) -> Result<(), ActantError> {
        match self {
            Self::Sqlite(s) => s.append_event(event).await,
            Self::Postgres(p) => p.append_event(event).await,
        }
    }

    /// Last event hash for a workspace or session.
    pub async fn last_event_hash(
        &self,
        workspace_id: &WorkspaceId,
        session_id: Option<&SessionId>,
    ) -> Result<Option<String>, ActantError> {
        match self {
            Self::Sqlite(s) => s.last_event_hash(workspace_id, session_id).await,
            Self::Postgres(p) => p.last_event_hash(workspace_id, session_id).await,
        }
    }

    /// Insert a command record.
    pub async fn insert_command(&self, command: &CommandRecord) -> Result<(), ActantError> {
        match self {
            Self::Sqlite(s) => s.insert_command(command).await,
            Self::Postgres(p) => p.insert_command(command).await,
        }
    }

    /// Look up an idempotency key.
    pub async fn idempotency_lookup(
        &self,
        workspace_id: &WorkspaceId,
        key: &str,
    ) -> Result<Option<String>, ActantError> {
        match self {
            Self::Sqlite(s) => s.idempotency_lookup(workspace_id, key).await,
            Self::Postgres(p) => p.idempotency_lookup(workspace_id, key).await,
        }
    }

    /// Record an idempotency key.
    pub async fn idempotency_record(
        &self,
        workspace_id: &WorkspaceId,
        actor_id: &ActorId,
        key: &str,
        command_type: &str,
        input_hash: &str,
        result_ref: Option<&str>,
    ) -> Result<(), ActantError> {
        match self {
            Self::Sqlite(s) => {
                s.idempotency_record(
                    workspace_id,
                    actor_id,
                    key,
                    command_type,
                    input_hash,
                    result_ref,
                )
                .await
            }
            Self::Postgres(p) => {
                p.idempotency_record(
                    workspace_id,
                    actor_id,
                    key,
                    command_type,
                    input_hash,
                    result_ref,
                )
                .await
            }
        }
    }

    /// Store artifact bytes and metadata.
    pub async fn put_artifact(
        &self,
        workspace_id: &WorkspaceId,
        actor_id: &ActorId,
        kind: &str,
        body: Bytes,
        sensitivity: Sensitivity,
    ) -> Result<ArtifactId, ActantError> {
        match self {
            Self::Sqlite(s) => {
                s.put_artifact(workspace_id, actor_id, kind, body, sensitivity)
                    .await
            }
            Self::Postgres(p) => {
                p.put_artifact(workspace_id, actor_id, kind, body, sensitivity)
                    .await
            }
        }
    }

    /// Fetch an artifact URI by id.
    pub async fn get_artifact_uri(&self, id: &ArtifactId) -> Result<Option<String>, ActantError> {
        match self {
            Self::Sqlite(s) => s.get_artifact_uri(id).await,
            Self::Postgres(p) => p.get_artifact_uri(id).await,
        }
    }

    /// Query events for a session.
    pub async fn events_in_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<AgentEvent>, ActantError> {
        match self {
            Self::Sqlite(s) => s.events_in_session(session_id).await,
            Self::Postgres(p) => p.events_in_session(session_id).await,
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
