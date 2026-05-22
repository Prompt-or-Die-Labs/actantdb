//! `actantdb export` and `actantdb import` — bulk-dump and reload events.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use actant_storage::{Storage, StorageConfig};
use clap::ValueEnum;
use serde_json::json;
use sqlx::Row;

use crate::cli_errors;

/// Output format for `actantdb export`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum ExportFormat {
    /// One big JSON array.
    Json,
    /// One JSON object per line.
    Ndjson,
    /// Comma-separated values (no payload escaping beyond CSV rules).
    Csv,
}

/// Run the export command.
pub async fn run_export(
    db_path: &Path,
    format: ExportFormat,
    session: Option<String>,
    out: Option<PathBuf>,
) -> anyhow::Result<()> {
    let s = Storage::open(StorageConfig::file(db_path)).await?;
    let rows = if let Some(sid) = session.as_ref() {
        sqlx::query(
            "SELECT id, workspace_id, actor_id, session_id, parent_event_id, \
                    event_type, causality_kind, sensitivity, \
                    payload_inline, payload_ref, payload_hash, event_hash, created_at \
             FROM agent_event WHERE session_id = ? ORDER BY id ASC",
        )
        .bind(sid)
        .fetch_all(s.pool())
        .await?
    } else {
        sqlx::query(
            "SELECT id, workspace_id, actor_id, session_id, parent_event_id, \
                    event_type, causality_kind, sensitivity, \
                    payload_inline, payload_ref, payload_hash, event_hash, created_at \
             FROM agent_event ORDER BY id ASC",
        )
        .fetch_all(s.pool())
        .await?
    };

    let mut sink: Box<dyn Write> = match &out {
        Some(p) => Box::new(BufWriter::new(
            File::create(p).map_err(|e| cli_errors::storage("create export file", e))?,
        )),
        None => Box::new(std::io::stdout()),
    };

    match format {
        ExportFormat::Ndjson => {
            for r in &rows {
                let obj = row_to_json(r);
                writeln!(
                    sink,
                    "{}",
                    serde_json::to_string(&obj)
                        .map_err(|e| cli_errors::internal("encode exported row", e))?
                )?;
            }
        }
        ExportFormat::Json => {
            let objs: Vec<_> = rows.iter().map(row_to_json).collect();
            writeln!(
                sink,
                "{}",
                serde_json::to_string_pretty(&objs)
                    .map_err(|e| cli_errors::internal("encode export", e))?
            )?;
        }
        ExportFormat::Csv => {
            writeln!(
                sink,
                "id,created_at,event_type,actor_id,session_id,sensitivity,payload"
            )?;
            for r in &rows {
                let obj = row_to_json(r);
                let payload = obj
                    .get("payload_inline")
                    .and_then(|p| p.as_str())
                    .unwrap_or("");
                writeln!(
                    sink,
                    "{},{},{},{},{},{},{}",
                    csv(obj.get("id").and_then(|v| v.as_str()).unwrap_or_default()),
                    csv(obj
                        .get("created_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()),
                    csv(obj
                        .get("event_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()),
                    csv(obj
                        .get("actor_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()),
                    csv(obj
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()),
                    csv(obj
                        .get("sensitivity")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()),
                    csv(payload),
                )?;
            }
        }
    }
    sink.flush()
        .map_err(|e| cli_errors::storage("flush export output", e))?;
    eprintln!("exported {} rows", rows.len());
    Ok(())
}

fn csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn row_to_json(r: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    let sensitivity: String = r.try_get("sensitivity").unwrap_or_default();
    // Honor capsule sensitivity ceiling — redact secrets.
    let payload_inline: Option<String> = r.try_get("payload_inline").ok();
    let payload_ref: Option<String> = r.try_get("payload_ref").ok();
    let (payload_inline, payload_ref) = if sensitivity == "secret" {
        (Some("<redacted: secret>".to_string()), None)
    } else {
        (payload_inline, payload_ref)
    };
    json!({
        "id": r.try_get::<String, _>("id").unwrap_or_default(),
        "workspace_id": r.try_get::<String, _>("workspace_id").unwrap_or_default(),
        "actor_id": r.try_get::<String, _>("actor_id").unwrap_or_default(),
        "session_id": r.try_get::<Option<String>, _>("session_id").unwrap_or(None),
        "parent_event_id": r.try_get::<Option<String>, _>("parent_event_id").unwrap_or(None),
        "event_type": r.try_get::<String, _>("event_type").unwrap_or_default(),
        "causality_kind": r.try_get::<String, _>("causality_kind").unwrap_or_default(),
        "sensitivity": sensitivity,
        "payload_inline": payload_inline,
        "payload_ref": payload_ref,
        "payload_hash": r.try_get::<String, _>("payload_hash").unwrap_or_default(),
        "event_hash": r.try_get::<String, _>("event_hash").unwrap_or_default(),
        "created_at": r.try_get::<String, _>("created_at").unwrap_or_default(),
    })
}

// ---------------------------------------------------------------------------
// import
// ---------------------------------------------------------------------------

/// Run the import command — NDJSON input only.
pub async fn run_import(db_path: &Path, from: &Path) -> anyhow::Result<()> {
    let s = Storage::open(StorageConfig::file(db_path)).await?;
    let file =
        File::open(from).map_err(|e| cli_errors::not_found(format!("open import file: {e}")))?;
    let reader = BufReader::new(file);

    // Phase 1: read all records and collect run/session ids referenced.
    let mut records: Vec<serde_json::Value> = Vec::new();
    let mut run_ids: HashSet<String> = HashSet::new();
    for (lineno, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| cli_errors::storage("read import file", e))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
            cli_errors::invalid_input(format!("line {}: bad JSON: {e}", lineno + 1))
        })?;
        if let Some(id) = v
            .get("workflow_run_id")
            .and_then(|x| x.as_str())
            .or_else(|| v.get("session_id").and_then(|x| x.as_str()))
        {
            run_ids.insert(id.to_string());
        }
        records.push(v);
    }

    // Phase 2: idempotency — refuse if the target DB already has events
    // for any session_id we're importing.
    for id in &run_ids {
        let row = sqlx::query("SELECT 1 AS exists_ FROM agent_event WHERE session_id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(s.pool())
            .await?;
        if row.is_some() {
            return Err(cli_errors::conflict(format!(
                "refusing to import: events already exist for session/run `{id}`. \
                 Migrate to a fresh DB or remove the existing rows first."
            ))
            .into());
        }
    }

    // Phase 3: insert. We use INSERT OR IGNORE — the PK clash for any
    // duplicate `id` becomes a silent no-op rather than aborting the
    // whole batch.
    let mut inserted = 0;
    for v in &records {
        let res = sqlx::query(
            "INSERT OR IGNORE INTO agent_event \
             (id, workspace_id, actor_id, session_id, parent_event_id, \
              event_type, causality_kind, sensitivity, \
              payload_inline, payload_ref, payload_hash, event_hash, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(v.get("id").and_then(|x| x.as_str()).unwrap_or(""))
        .bind(v.get("workspace_id").and_then(|x| x.as_str()).unwrap_or(""))
        .bind(v.get("actor_id").and_then(|x| x.as_str()).unwrap_or(""))
        .bind(v.get("session_id").and_then(|x| x.as_str()))
        .bind(v.get("parent_event_id").and_then(|x| x.as_str()))
        .bind(v.get("event_type").and_then(|x| x.as_str()).unwrap_or(""))
        .bind(
            v.get("causality_kind")
                .and_then(|x| x.as_str())
                .unwrap_or("audit"),
        )
        .bind(
            v.get("sensitivity")
                .and_then(|x| x.as_str())
                .unwrap_or("low"),
        )
        .bind(v.get("payload_inline").and_then(|x| x.as_str()))
        .bind(v.get("payload_ref").and_then(|x| x.as_str()))
        .bind(v.get("payload_hash").and_then(|x| x.as_str()).unwrap_or(""))
        .bind(v.get("event_hash").and_then(|x| x.as_str()).unwrap_or(""))
        .bind(v.get("created_at").and_then(|x| x.as_str()).unwrap_or(""))
        .execute(s.pool())
        .await?;
        inserted += res.rows_affected();
    }
    println!(
        "imported {inserted} rows (out of {} candidates)",
        records.len()
    );
    Ok(())
}
