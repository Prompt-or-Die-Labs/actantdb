//! `actantdb watch <expr>` — live filter using the row-level predicate language.

use std::path::Path;
use std::time::Duration;

use actant_storage::{Storage, StorageConfig};
use actant_subscribe::Predicate;
use serde_json::json;
use sqlx::Row;

use crate::{cli_errors, predicate_parse};

/// Run the watch command.
pub async fn run(db_path: &Path, expr: &str) -> anyhow::Result<()> {
    let predicate: Predicate = predicate_parse::parse(expr)
        .map_err(|e| cli_errors::invalid_input(format!("parse predicate: {e}")))?;
    eprintln!("watching: {predicate:?}");

    let s = Storage::open(StorageConfig::file(db_path)).await?;
    let mut last_id: Option<String> = None;
    loop {
        let mut sql = String::from(
            "SELECT id, created_at, event_type, actor_id, session_id, payload_inline \
             FROM agent_event WHERE 1=1",
        );
        if last_id.is_some() {
            sql.push_str(" AND id > ?");
        }
        sql.push_str(" ORDER BY id ASC LIMIT 200");
        let mut q = sqlx::query(&sql);
        if let Some(id) = &last_id {
            q = q.bind(id);
        }
        let rows = q.fetch_all(s.pool()).await?;
        for r in rows {
            let id: String = r.try_get("id")?;
            let created_at: String = r.try_get("created_at")?;
            let event_type: String = r.try_get("event_type")?;
            let actor_id: String = r.try_get("actor_id")?;
            let session_id: Option<String> = r.try_get("session_id").ok();
            let payload_inline: Option<String> = r.try_get("payload_inline").ok();

            let payload_json: serde_json::Value = payload_inline
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::Value::Null);

            // The shape we evaluate is similar to the WS broadcast payload:
            // top-level `kind`, `actor`, `session`, `payload`.
            let row = json!({
                "kind": event_type,
                "actor": actor_id,
                "session": session_id,
                "payload": payload_json,
                "id": id,
                "created_at": created_at,
            });

            if predicate.evaluate(&row) {
                println!(
                    "{}  {}  {}",
                    created_at,
                    id,
                    serde_json::to_string(&row).unwrap_or_default()
                );
            }
            last_id = Some(id);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
