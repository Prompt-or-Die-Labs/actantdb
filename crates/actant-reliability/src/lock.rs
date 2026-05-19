//! Lease-based distributed locks backed by the `lock` table.

use actant_core::*;
use actant_storage::Storage;

/// Active lease.
#[derive(Debug, Clone)]
pub struct Lease {
    /// Lock id.
    pub id: String,
    /// Owner.
    pub owner: ActorId,
    /// Expiry RFC3339.
    pub expires_at: String,
}

/// Acquire a lock for `resource_key`.
pub async fn acquire(
    storage: &Storage,
    workspace: &WorkspaceId,
    owner: &ActorId,
    resource_key: &str,
    lease_seconds: i64,
) -> Result<Option<Lease>, ActantError> {
    use time::format_description::well_known::Rfc3339;
    let now = time::OffsetDateTime::now_utc();
    let expires_at = (now + time::Duration::seconds(lease_seconds))
        .format(&Rfc3339)
        .expect("rfc3339");
    let id = format!("lk_{}", ulid::Ulid::new());
    sqlx::query("DELETE FROM lock WHERE workspace_id = ? AND expires_at < ?")
        .bind(workspace.as_str())
        .bind(now.format(&Rfc3339).expect("rfc3339"))
        .execute(storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
    let res = sqlx::query(
        "INSERT INTO lock (id, workspace_id, resource_key, owner_actor_id, expires_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace.as_str())
    .bind(resource_key)
    .bind(owner.as_str())
    .bind(&expires_at)
    .bind(now.format(&Rfc3339).expect("rfc3339"))
    .execute(storage.pool())
    .await;
    match res {
        Ok(_) => Ok(Some(Lease {
            id,
            owner: owner.clone(),
            expires_at,
        })),
        Err(_) => Ok(None),
    }
}

/// Release a lock by id.
pub async fn release(storage: &Storage, lock_id: &str) -> Result<(), ActantError> {
    sqlx::query("DELETE FROM lock WHERE id = ?")
        .bind(lock_id)
        .execute(storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actant_storage::StorageConfig;

    #[tokio::test]
    async fn acquire_and_contention() {
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
            display_name: "w".into(),
            created_at: now_rfc3339(),
            disabled_at: None,
        };
        s.insert_actor(&actor).await.unwrap();
        let l = acquire(&s, &ws.id, &actor.id, "file:/foo", 30)
            .await
            .unwrap()
            .unwrap();
        assert!(acquire(&s, &ws.id, &actor.id, "file:/foo", 30)
            .await
            .unwrap()
            .is_none());
        release(&s, &l.id).await.unwrap();
        assert!(acquire(&s, &ws.id, &actor.id, "file:/foo", 30)
            .await
            .unwrap()
            .is_some());
    }
}
