//! actant-memory — candidate / approval / use lifecycle on top of
//! `actant-storage`. The high-level command surface lives in `actant-command`;
//! this crate holds reusable lifecycle helpers and queries.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::*;
use actant_storage::Storage;
use sqlx::Row;

/// Memory lifecycle façade over a `Storage`.
#[derive(Clone)]
pub struct MemoryStore {
    storage: Storage,
}

impl MemoryStore {
    /// Wrap a storage handle.
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// List active memories in a workspace.
    pub async fn list_active(&self, workspace: &WorkspaceId) -> Result<Vec<Memory>, ActantError> {
        let rows = sqlx::query(
            "SELECT id, workspace_id, text, category, sensitivity, confidence,
                    scope, source_candidate_id, source_event_ids, embedding_ref_id,
                    usage_count, last_used_at, expires_at, revoked_at, deleted_at, created_at
             FROM memory
             WHERE workspace_id = ? AND revoked_at IS NULL AND deleted_at IS NULL
             ORDER BY created_at DESC",
        )
        .bind(workspace.as_str())
        .fetch_all(self.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let sens_s: String = r.get("sensitivity");
            out.push(Memory {
                id: MemoryId::from_string(r.get::<String, _>("id")),
                workspace_id: WorkspaceId::from_string(r.get::<String, _>("workspace_id")),
                text: r.get("text"),
                category: r.get("category"),
                sensitivity: serde_json::from_value(serde_json::Value::String(sens_s))
                    .unwrap_or(Sensitivity::Low),
                confidence: r.get("confidence"),
                scope: r.get("scope"),
                source_candidate_id: r
                    .get::<Option<String>, _>("source_candidate_id")
                    .map(MemoryCandidateId::from_string),
                source_event_ids: r.get("source_event_ids"),
                embedding_ref_id: r
                    .get::<Option<String>, _>("embedding_ref_id")
                    .map(EmbeddingRefId::from_string),
                usage_count: r.get("usage_count"),
                last_used_at: r.get("last_used_at"),
                expires_at: r.get("expires_at"),
                revoked_at: r.get("revoked_at"),
                deleted_at: r.get("deleted_at"),
                created_at: r.get("created_at"),
            });
        }
        Ok(out)
    }

    /// Mark a memory as revoked (soft delete).
    pub async fn revoke(&self, id: &MemoryId) -> Result<(), ActantError> {
        sqlx::query("UPDATE memory SET revoked_at = ? WHERE id = ?")
            .bind(now_rfc3339())
            .bind(id.as_str())
            .execute(self.storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Hard-delete a memory: scrub text and embedding pointer.
    pub async fn delete(&self, id: &MemoryId) -> Result<(), ActantError> {
        sqlx::query(
            "UPDATE memory SET deleted_at = ?, text = '', embedding_ref_id = NULL
             WHERE id = ?",
        )
        .bind(now_rfc3339())
        .bind(id.as_str())
        .execute(self.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Detect contradictory pairs of active memories.
    /// Conflict = same lemma overlap above `threshold` (Jaccard on word
    /// tokens) AND opposing polarity markers ("is" vs "is not", "always" vs
    /// "never"). Writes a `memory_conflict` row for each pair detected and
    /// returns the inserted count.
    pub async fn detect_conflicts(
        &self,
        workspace: &WorkspaceId,
        threshold: f64,
    ) -> Result<usize, ActantError> {
        let mems = self.list_active(workspace).await?;
        let mut inserted = 0usize;
        for i in 0..mems.len() {
            for j in (i + 1)..mems.len() {
                let a = &mems[i];
                let b = &mems[j];
                if jaccard(&a.text, &b.text) < threshold {
                    continue;
                }
                if !opposing_polarity(&a.text, &b.text) {
                    continue;
                }
                let id = format!("mc_{}", ulid::Ulid::new());
                let row = sqlx::query(
                    "INSERT OR IGNORE INTO memory_conflict
                        (id, workspace_id, memory_a_id, memory_b_id, conflict_type, detected_at)
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(&id)
                .bind(workspace.as_str())
                .bind(a.id.as_str())
                .bind(b.id.as_str())
                .bind("polarity")
                .bind(now_rfc3339())
                .execute(self.storage.pool())
                .await
                .map_err(|e| ActantError::Storage(e.to_string()))?;
                if row.rows_affected() > 0 {
                    inserted += 1;
                }
            }
        }
        Ok(inserted)
    }
}

fn tokens(s: &str) -> std::collections::HashSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

fn jaccard(a: &str, b: &str) -> f64 {
    let ta = tokens(a);
    let tb = tokens(b);
    if ta.is_empty() && tb.is_empty() {
        return 1.0;
    }
    let i = ta.intersection(&tb).count() as f64;
    let u = ta.union(&tb).count() as f64;
    if u == 0.0 {
        0.0
    } else {
        i / u
    }
}

fn opposing_polarity(a: &str, b: &str) -> bool {
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let pairs: &[(&str, &str)] = &[
        ("is not", "is "),
        ("always", "never"),
        ("must", "must not"),
        ("can", "cannot"),
        ("does not", "does "),
    ];
    for (p, q) in pairs {
        if (a.contains(p) && b.contains(q) && !b.contains(p))
            || (b.contains(p) && a.contains(q) && !a.contains(p))
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use actant_storage::StorageConfig;

    async fn insert_mem(s: &Storage, ws: &WorkspaceId, text: &str) -> MemoryId {
        let id = MemoryId::new();
        sqlx::query(
            "INSERT INTO memory
                (id, workspace_id, text, category, sensitivity, scope,
                 source_event_ids, usage_count, created_at)
             VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind(id.as_str())
        .bind(ws.as_str())
        .bind(text)
        .bind("fact")
        .bind("low")
        .bind("global")
        .bind("[]")
        .bind(0i64)
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .unwrap();
        id
    }

    #[tokio::test]
    async fn detects_polarity_conflict() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let ws = Workspace {
            id: WorkspaceId::new(),
            name: "t".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        s.insert_workspace(&ws).await.unwrap();
        insert_mem(&s, &ws.id, "the project always uses pytest").await;
        insert_mem(&s, &ws.id, "the project never uses pytest").await;
        let store = MemoryStore::new(s.clone());
        let n = store.detect_conflicts(&ws.id, 0.3).await.unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn revoke_and_delete() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let ws = Workspace {
            id: WorkspaceId::new(),
            name: "t".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        s.insert_workspace(&ws).await.unwrap();
        let mem_id = MemoryId::new();
        sqlx::query(
            "INSERT INTO memory
                (id, workspace_id, text, category, sensitivity, scope,
                 source_event_ids, usage_count, created_at)
             VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind(mem_id.as_str())
        .bind(ws.id.as_str())
        .bind("hi")
        .bind("fact")
        .bind("low")
        .bind("global")
        .bind("[]")
        .bind(0i64)
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .unwrap();
        let store = MemoryStore::new(s.clone());
        assert_eq!(store.list_active(&ws.id).await.unwrap().len(), 1);
        store.revoke(&mem_id).await.unwrap();
        assert_eq!(store.list_active(&ws.id).await.unwrap().len(), 0);
        store.delete(&mem_id).await.unwrap();
    }
}
