//! actant-audit-export — JSONL Chronicle dump with retention windows and
//! chunked nightly export.
//!
//! Phase 6 surface:
//!
//! - `export_workspace(...)` — full dump (Phase 1 behavior).
//! - `export_window(...)` — slice by `[from, to)` time window.
//! - `export_since(...)` — convenience: events after a checkpoint timestamp.
//! - `RetentionPolicy::should_retain(...)` — predicate the caller uses to
//!   decide whether to keep an event in long-term cold storage.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::io::Write;

use actant_core::*;
use actant_storage::Storage;
use serde::{Deserialize, Serialize};
use sqlx::Row;

/// Retention policy applied at export time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Keep events younger than this many days (after this, drop).
    pub keep_days: u32,
    /// Always retain events whose sensitivity is at or above this level.
    pub always_keep_at_least: Sensitivity,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_days: 365,
            always_keep_at_least: Sensitivity::High,
        }
    }
}

impl RetentionPolicy {
    /// True if an event should be retained.
    pub fn should_retain(&self, created_at_rfc3339: &str, sens: Sensitivity) -> bool {
        if sens_rank(sens) >= sens_rank(self.always_keep_at_least) {
            return true;
        }
        let Ok(t) = time::OffsetDateTime::parse(
            created_at_rfc3339,
            &time::format_description::well_known::Rfc3339,
        ) else {
            // If we can't parse, default to retain.
            return true;
        };
        let cutoff = time::OffsetDateTime::now_utc() - time::Duration::days(self.keep_days as i64);
        t >= cutoff
    }
}

fn sens_rank(s: Sensitivity) -> u8 {
    match s {
        Sensitivity::Public => 0,
        Sensitivity::Low => 1,
        Sensitivity::Medium => 2,
        Sensitivity::High => 3,
        Sensitivity::Secret => 4,
        Sensitivity::Regulated => 5,
    }
}

/// Export every event in a workspace as JSONL.
pub async fn export_workspace<W: Write>(
    storage: &Storage,
    workspace: &WorkspaceId,
    out: &mut W,
) -> Result<usize, ActantError> {
    export_window(storage, workspace, None, None, out).await
}

/// Export events in `[from, to)` (both bounds optional) as JSONL.
pub async fn export_window<W: Write>(
    storage: &Storage,
    workspace: &WorkspaceId,
    from: Option<&str>,
    to: Option<&str>,
    out: &mut W,
) -> Result<usize, ActantError> {
    let mut where_clauses = vec!["workspace_id = ?".to_string()];
    if from.is_some() {
        where_clauses.push("created_at >= ?".into());
    }
    if to.is_some() {
        where_clauses.push("created_at < ?".into());
    }
    let sql = format!(
        "SELECT id, actor_id, event_type, payload_inline, sensitivity, created_at
         FROM agent_event WHERE {} ORDER BY created_at ASC, id ASC",
        where_clauses.join(" AND ")
    );
    let mut q = sqlx::query(&sql).bind(workspace.as_str());
    if let Some(t) = from {
        q = q.bind(t);
    }
    if let Some(t) = to {
        q = q.bind(t);
    }
    let rows = q
        .fetch_all(storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
    let mut n = 0;
    for r in rows {
        let id: String = r.get("id");
        let actor: String = r.get("actor_id");
        let typ: String = r.get("event_type");
        let inline: Option<String> = r.get("payload_inline");
        let sens_s: String = r.get("sensitivity");
        let created: String = r.get("created_at");
        let line = serde_json::json!({
            "id": id,
            "actor": actor,
            "event_type": typ,
            "payload": inline,
            "sensitivity": sens_s,
            "created_at": created,
        });
        writeln!(out, "{}", serde_json::to_string(&line).unwrap())
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        n += 1;
    }
    Ok(n)
}

/// Purge events that the policy says should not be retained. Returns the
/// number of rows deleted. Run periodically (daily/nightly).
pub async fn purge_by_policy(
    storage: &Storage,
    workspace: &WorkspaceId,
    policy: &RetentionPolicy,
) -> Result<usize, ActantError> {
    // Iterate every event for the workspace. Delete the ones that fail
    // should_retain. We delete one-by-one to keep SQLite happy and to
    // produce a deterministic count.
    let rows =
        sqlx::query("SELECT id, sensitivity, created_at FROM agent_event WHERE workspace_id = ?")
            .bind(workspace.as_str())
            .fetch_all(storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
    let mut purged = 0usize;
    for r in rows {
        let id: String = r.get("id");
        let sens_s: String = r.get("sensitivity");
        let created: String = r.get("created_at");
        let sens = match sens_s.as_str() {
            "public" => Sensitivity::Public,
            "low" => Sensitivity::Low,
            "medium" => Sensitivity::Medium,
            "high" => Sensitivity::High,
            "secret" => Sensitivity::Secret,
            "regulated" => Sensitivity::Regulated,
            _ => Sensitivity::Low,
        };
        if policy.should_retain(&created, sens) {
            continue;
        }
        sqlx::query("DELETE FROM agent_event WHERE id = ?")
            .bind(&id)
            .execute(storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        purged += 1;
    }
    Ok(purged)
}

/// Export every workspace into its own JSONL chunk. Returns a map of
/// `workspace_id → count`.
pub async fn nightly_export<F>(
    storage: &Storage,
    chunk_factory: F,
) -> Result<Vec<(String, usize)>, ActantError>
where
    F: Fn(&str) -> Result<Box<dyn Write + Send>, ActantError>,
{
    let workspaces: Vec<(String,)> =
        sqlx::query_as("SELECT id FROM workspace WHERE archived_at IS NULL")
            .fetch_all(storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
    let mut totals = Vec::with_capacity(workspaces.len());
    for (id,) in workspaces {
        let ws = WorkspaceId::from_string(id.clone());
        let mut w = chunk_factory(&id)?;
        let n = export_workspace(storage, &ws, &mut w).await?;
        totals.push((id, n));
    }
    Ok(totals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actant_storage::StorageConfig;
    use std::io::Cursor;

    #[tokio::test]
    async fn exports_jsonl() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let ws = Workspace {
            id: WorkspaceId::new(),
            name: "t".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        s.insert_workspace(&ws).await.unwrap();
        let mut buf = Vec::new();
        let n = export_workspace(&s, &ws.id, &mut buf).await.unwrap();
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn purge_drops_old_low_sens_keeps_high() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let ws = Workspace {
            id: WorkspaceId::new(),
            name: "t".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        s.insert_workspace(&ws).await.unwrap();
        let actor = Actor {
            id: ActorId::new(),
            workspace_id: ws.id.clone(),
            kind: ActorKind::Human,
            display_name: "x".into(),
            created_at: now_rfc3339(),
            disabled_at: None,
        };
        s.insert_actor(&actor).await.unwrap();
        // Insert two events: one old + low (should be purged) and one old + high.
        let old_iso = (time::OffsetDateTime::now_utc() - time::Duration::days(365))
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        for (id, sens) in [("evt_old_low", "low"), ("evt_old_high", "high")] {
            sqlx::query(
                "INSERT INTO agent_event
                    (id, workspace_id, actor_id, event_type, causality_kind,
                     sensitivity, payload_hash, event_hash, created_at)
                 VALUES (?,?,?,?,?,?,?,?,?)",
            )
            .bind(id)
            .bind(ws.id.as_str())
            .bind(actor.id.as_str())
            .bind("t")
            .bind("audit")
            .bind(sens)
            .bind("h")
            .bind("h")
            .bind(&old_iso)
            .execute(s.pool())
            .await
            .unwrap();
        }
        let policy = RetentionPolicy {
            keep_days: 30,
            always_keep_at_least: Sensitivity::High,
        };
        let purged = purge_by_policy(&s, &ws.id, &policy).await.unwrap();
        assert_eq!(purged, 1);
    }

    #[test]
    fn retention_keeps_high_sens_forever() {
        let p = RetentionPolicy {
            keep_days: 1,
            always_keep_at_least: Sensitivity::High,
        };
        let old = (time::OffsetDateTime::now_utc() - time::Duration::days(365))
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        assert!(p.should_retain(&old, Sensitivity::High));
        assert!(!p.should_retain(&old, Sensitivity::Low));
    }

    #[tokio::test]
    async fn nightly_export_writes_one_chunk_per_workspace() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        for n in ["a", "b"] {
            s.insert_workspace(&Workspace {
                id: WorkspaceId::new(),
                name: n.into(),
                created_at: now_rfc3339(),
                archived_at: None,
            })
            .await
            .unwrap();
        }
        let chunks: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
        let chunks_for_closure = std::sync::Arc::clone(&chunks);
        let totals = nightly_export(&s, move |ws_id| {
            let map = std::sync::Arc::clone(&chunks_for_closure);
            let key = ws_id.to_string();
            // Each call gets its own Vec<u8> behind a Mutex.
            map.lock().unwrap().insert(key.clone(), Vec::new());
            // Return a writer that pushes into the map.
            struct W {
                map: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>>,
                key: String,
            }
            impl Write for W {
                fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                    let mut g = self.map.lock().unwrap();
                    g.entry(self.key.clone())
                        .or_default()
                        .extend_from_slice(buf);
                    Ok(buf.len())
                }
                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }
            Ok(Box::new(W { map, key }) as Box<dyn Write + Send>)
        })
        .await
        .unwrap();
        assert_eq!(totals.len(), 2);
        let _ = Cursor::new(Vec::<u8>::new()); // silence unused import
    }
}
