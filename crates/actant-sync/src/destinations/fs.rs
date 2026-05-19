//! Filesystem destination — the default sync target.
//!
//! Lays Chronicle events out as one immutable JSON file per event under
//! `<root>/<workspace_id>/<YYYY-MM-DD>/<event_id>.json`. The cursor is a
//! single-line text file at `<root>/<workspace_id>/_cursor.txt`.
//!
//! Idempotency: the per-event key is content-addressed (event id is the
//! immutable hash-chained id). Re-pushing the same batch writes the same
//! bytes to the same paths — atomic rename means a torn read is impossible.
//! The cursor file is updated only after every event in the batch has
//! landed, so a crash mid-batch is bounded: the next run sees the unchanged
//! cursor and re-attempts the partial batch.

use std::path::{Path, PathBuf};

use actant_core::{AgentEvent, EventId, WorkspaceId};
use async_trait::async_trait;
use tokio::fs;

use crate::destination::{cursor_key, key_for, Destination};
use crate::SyncError;

/// Filesystem-backed destination. Cheap to clone (wraps a `PathBuf`).
#[derive(Debug, Clone)]
pub struct FilesystemDestination {
    root: PathBuf,
}

impl FilesystemDestination {
    /// Open a destination rooted at `root`. Creates the directory if missing.
    pub fn new(root: impl AsRef<Path>) -> std::io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            root: root.canonicalize()?,
        })
    }

    /// Absolute root path on disk.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn event_path(&self, ws: &WorkspaceId, e: &AgentEvent) -> PathBuf {
        self.root.join(key_for(ws, e))
    }

    fn cursor_path(&self, ws: &WorkspaceId) -> PathBuf {
        self.root.join(cursor_key(ws))
    }

    async fn write_event(&self, ws: &WorkspaceId, e: &AgentEvent) -> Result<(), SyncError> {
        let path = self.event_path(ws, e);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let bytes = serde_json::to_vec_pretty(e)?;
        // Atomic write: write to a `.tmp` sibling, then rename. Avoids torn
        // reads if another process is iterating the directory.
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, &bytes).await?;
        fs::rename(&tmp, &path).await?;
        Ok(())
    }

    async fn write_cursor(&self, ws: &WorkspaceId, id: &EventId) -> Result<(), SyncError> {
        let path = self.cursor_path(ws);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let tmp = path.with_extension("txt.tmp");
        fs::write(&tmp, id.as_str().as_bytes()).await?;
        fs::rename(&tmp, &path).await?;
        Ok(())
    }
}

#[async_trait]
impl Destination for FilesystemDestination {
    async fn push(
        &self,
        workspace_id: &WorkspaceId,
        _since_event_id: Option<&EventId>,
        batch: &[AgentEvent],
    ) -> Result<Option<EventId>, SyncError> {
        if batch.is_empty() {
            return Ok(self.cursor(workspace_id).await?);
        }
        // Write every event before touching the cursor so a crash mid-batch
        // leaves the cursor pointing at the prior boundary — the runner will
        // re-attempt and overwrite the same files (idempotent).
        for e in batch {
            self.write_event(workspace_id, e).await?;
        }
        let final_id = batch.last().unwrap().id.clone();
        self.write_cursor(workspace_id, &final_id).await?;
        Ok(Some(final_id))
    }

    async fn cursor(&self, workspace_id: &WorkspaceId) -> Result<Option<EventId>, SyncError> {
        let path = self.cursor_path(workspace_id);
        match fs::read_to_string(&path).await {
            Ok(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(EventId::from_string(trimmed.to_string())))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SyncError::Io(e.to_string())),
        }
    }

    fn name(&self) -> &str {
        "filesystem"
    }
}
