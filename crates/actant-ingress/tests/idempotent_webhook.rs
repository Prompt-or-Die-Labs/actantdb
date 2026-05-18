//! Property: submitting 100 duplicate webhook payloads with the same
//! idempotency key produces exactly one `ingress_event` row.
//!
//! Covers AC: "100 duplicate webhook submissions produce exactly 1 ingress_event."

use actant_core::{now_rfc3339, Workspace, WorkspaceId};
use actant_ingress::{ingest, IngressEvent};
use actant_storage::{Storage, StorageConfig};

#[tokio::test]
async fn one_hundred_duplicates_produce_exactly_one_row() {
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "duplicate-test".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();

    let event = IngressEvent {
        source: "stripe".into(),
        event_type: "charge.created".into(),
        dedupe_key: "evt_idempotent_42".into(),
        payload: serde_json::json!({"amount": 4200, "currency": "usd"}),
    };

    let mut new_count = 0u32;
    let mut dedup_count = 0u32;
    for _ in 0..100 {
        match ingest(&s, &ws.id, &event).await.unwrap() {
            Some(_id) => new_count += 1,
            None => dedup_count += 1,
        }
    }
    assert_eq!(
        new_count, 1,
        "exactly one ingest call should produce a new row"
    );
    assert_eq!(dedup_count, 99, "remaining 99 calls should dedupe");

    let (row_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM ingress_event WHERE workspace_id = ? AND source = ? AND dedupe_key = ?",
    )
    .bind(ws.id.as_str())
    .bind(&event.source)
    .bind(&event.dedupe_key)
    .fetch_one(s.pool())
    .await
    .unwrap();
    assert_eq!(row_count, 1, "exactly one ingress_event row in storage");
}

#[tokio::test]
async fn distinct_keys_still_produce_distinct_rows() {
    // Sanity check: dedup is keyed by (source, dedupe_key), so 100 distinct
    // keys should yield 100 rows.
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "distinct-test".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();

    for i in 0..100 {
        let event = IngressEvent {
            source: "stripe".into(),
            event_type: "charge.created".into(),
            dedupe_key: format!("evt_{i}"),
            payload: serde_json::json!({"i": i}),
        };
        assert!(ingest(&s, &ws.id, &event).await.unwrap().is_some());
    }
    let (row_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM ingress_event WHERE workspace_id = ?")
            .bind(ws.id.as_str())
            .fetch_one(s.pool())
            .await
            .unwrap();
    assert_eq!(row_count, 100);
}
