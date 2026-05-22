//! `actantdb explain <event_id>` — natural-language explanation of one event.

use std::path::Path;

use actant_storage::{Storage, StorageConfig};
use sqlx::Row;

use crate::cli_errors;

/// Run the explain command.
pub async fn run(db_path: &Path, event_id: &str) -> anyhow::Result<()> {
    let s = Storage::open(StorageConfig::file(db_path)).await?;
    let row = sqlx::query(
        "SELECT id, created_at, event_type, actor_id, session_id, parent_event_id, \
                causality_kind, sensitivity, causal_parent_ids, \
                tool_call_id, model_call_id, workflow_run_id, payload_inline \
         FROM agent_event WHERE id = ?",
    )
    .bind(event_id)
    .fetch_optional(s.pool())
    .await?;

    let Some(r) = row else {
        return Err(cli_errors::not_found(format!("event not found: {event_id}")).into());
    };

    let id: String = r.try_get("id")?;
    let created_at: String = r.try_get("created_at")?;
    let event_type: String = r.try_get("event_type")?;
    let actor_id: String = r.try_get("actor_id")?;
    let session_id: Option<String> = r.try_get("session_id").ok();
    let parent_event_id: Option<String> = r.try_get("parent_event_id").ok();
    let causality_kind: String = r.try_get("causality_kind").unwrap_or_default();
    let sensitivity: String = r.try_get("sensitivity").unwrap_or_default();
    let causal_parent_ids: Option<String> = r.try_get("causal_parent_ids").ok();
    let tool_call_id: Option<String> = r.try_get("tool_call_id").ok();
    let model_call_id: Option<String> = r.try_get("model_call_id").ok();
    let workflow_run_id: Option<String> = r.try_get("workflow_run_id").ok();
    let payload_inline: Option<String> = r.try_get("payload_inline").ok();

    println!("Event {id} ({event_type})");
    println!("  recorded at {created_at}");
    println!("  by actor {actor_id}");
    if let Some(sid) = &session_id {
        println!("  in session {sid}");
    }
    if let Some(wf) = &workflow_run_id {
        println!("  part of run {wf}");
    }
    println!("  causality_kind = {causality_kind}");
    println!("  sensitivity    = {sensitivity}");

    // Causal parents — first the immediate parent, then the broader DAG.
    if let Some(pid) = &parent_event_id {
        explain_parent(&s, "immediate parent", pid).await?;
    }

    if let Some(json_text) = causal_parent_ids.as_deref() {
        if let Ok(serde_json::Value::Array(arr)) =
            serde_json::from_str::<serde_json::Value>(json_text)
        {
            for entry in &arr {
                if let Some(pid) = entry.as_str() {
                    explain_parent(&s, "causal parent", pid).await?;
                }
            }
        }
    }

    // Forward edges — children whose parent_event_id points here.
    let children = sqlx::query(
        "SELECT id, event_type, created_at FROM agent_event \
         WHERE parent_event_id = ? ORDER BY created_at ASC LIMIT 20",
    )
    .bind(&id)
    .fetch_all(s.pool())
    .await?;
    if !children.is_empty() {
        println!("  triggered:");
        for c in children {
            let cid: String = c.try_get("id").unwrap_or_default();
            let ct: String = c.try_get("event_type").unwrap_or_default();
            let ca: String = c.try_get("created_at").unwrap_or_default();
            println!("    - {ct}  {cid}  at {ca}");
        }
    }

    if let Some(tc) = &tool_call_id {
        println!("  tool_call_id   = {tc}");
    }
    if let Some(mc) = &model_call_id {
        println!("  model_call_id  = {mc}");
    }

    if let Some(payload) = payload_inline.as_deref() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
            if let Some(status) = v.get("status").and_then(|s| s.as_str()) {
                println!("  status         = {status}");
            }
            if let Some(took) = v.get("took_ms").and_then(|s| s.as_u64()) {
                println!("  duration       = {took}ms");
            }
        }
    }

    Ok(())
}

async fn explain_parent(s: &Storage, label: &str, pid: &str) -> anyhow::Result<()> {
    let r = sqlx::query("SELECT event_type, created_at FROM agent_event WHERE id = ?")
        .bind(pid)
        .fetch_optional(s.pool())
        .await?;
    match r {
        Some(row) => {
            let et: String = row.try_get("event_type").unwrap_or_default();
            let at: String = row.try_get("created_at").unwrap_or_default();
            println!("  {label}: {et} {pid} at {at}");
        }
        None => println!("  {label}: {pid} (not found)"),
    }
    Ok(())
}
