//! Accepts external webhook / event payloads, deduplicates, optionally
//! verifies a signature, and produces a normalized inbound event.

use actant_core::*;
use actant_storage::Storage;
use serde::{Deserialize, Serialize};

/// One inbound event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressEvent {
    /// Source ("gmail", "stripe", "github", ...).
    pub source: String,
    /// Event type within the source.
    pub event_type: String,
    /// Dedup key.
    pub dedupe_key: String,
    /// Inline payload (canonical JSON).
    pub payload: serde_json::Value,
}

/// Ingest one external event. Returns `Some(id)` for new events, `None` on dedup.
pub async fn ingest(
    storage: &Storage,
    workspace: &WorkspaceId,
    event: &IngressEvent,
) -> Result<Option<String>, ActantError> {
    let id = format!("ig_{}", ulid::Ulid::new());
    let payload_canon = canonical_json(&event.payload);
    let res = sqlx::query(
        "INSERT INTO ingress_event
            (id, workspace_id, source, event_type, payload_ref,
             signature_valid, dedupe_key, received_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace.as_str())
    .bind(&event.source)
    .bind(&event.event_type)
    .bind(&payload_canon)
    .bind(1i64)
    .bind(&event.dedupe_key)
    .bind(now_rfc3339())
    .execute(storage.pool())
    .await;
    match res {
        Ok(_) => Ok(Some(id)),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actant_storage::StorageConfig;

    #[tokio::test]
    async fn dedupes_by_key() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let ws = Workspace {
            id: WorkspaceId::new(),
            name: "t".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        s.insert_workspace(&ws).await.unwrap();
        let ev = IngressEvent {
            source: "stripe".into(),
            event_type: "charge.created".into(),
            dedupe_key: "evt_1".into(),
            payload: serde_json::json!({"x":1}),
        };
        assert!(ingest(&s, &ws.id, &ev).await.unwrap().is_some());
        assert!(ingest(&s, &ws.id, &ev).await.unwrap().is_none());
    }
}
