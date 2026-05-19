//! `actantdb tail` — `tail -f`-style live log of the event ledger.

use std::path::Path;
use std::time::Duration;

use actant_storage::{Storage, StorageConfig};
use sqlx::Row;

/// Run the tail command.
pub async fn run(
    db_path: &Path,
    session: Option<String>,
    kind: Option<String>,
    actor: Option<String>,
    follow: bool,
) -> anyhow::Result<()> {
    let s = Storage::open(StorageConfig::file(db_path)).await?;
    let mut last_id: Option<String> = None;

    // First batch: last 20 events (DESC by id, then reversed for chrono order).
    let mut rows = fetch_recent(&s, &session, &kind, &actor, 20).await?;
    rows.reverse();
    for r in &rows {
        print_event(r);
        last_id = Some(r.id.clone());
    }

    if !follow {
        return Ok(());
    }

    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let batch = fetch_since(&s, &session, &kind, &actor, last_id.as_deref(), 100).await?;
        for r in &batch {
            print_event(r);
            last_id = Some(r.id.clone());
        }
    }
}

async fn fetch_recent(
    s: &Storage,
    session: &Option<String>,
    kind: &Option<String>,
    actor: &Option<String>,
    limit: i64,
) -> anyhow::Result<Vec<EvtRow>> {
    let mut sql = String::from(
        "SELECT id, created_at, event_type, actor_id, session_id, payload_inline \
         FROM agent_event WHERE 1=1",
    );
    if session.is_some() {
        sql.push_str(" AND session_id = ?");
    }
    if kind.is_some() {
        sql.push_str(" AND event_type = ?");
    }
    if actor.is_some() {
        sql.push_str(" AND actor_id = ?");
    }
    sql.push_str(" ORDER BY id DESC LIMIT ?");
    let mut q = sqlx::query(&sql);
    if let Some(v) = session {
        q = q.bind(v);
    }
    if let Some(v) = kind {
        q = q.bind(v);
    }
    if let Some(v) = actor {
        q = q.bind(v);
    }
    q = q.bind(limit);
    let rows = q.fetch_all(s.pool()).await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(EvtRow {
            id: r.try_get("id")?,
            created_at: r.try_get("created_at")?,
            event_type: r.try_get("event_type")?,
            actor_id: r.try_get("actor_id")?,
            session_id: r.try_get("session_id").ok(),
            payload_inline: r.try_get("payload_inline").ok(),
        });
    }
    Ok(out)
}

async fn fetch_since(
    s: &Storage,
    session: &Option<String>,
    kind: &Option<String>,
    actor: &Option<String>,
    after: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<EvtRow>> {
    let mut sql = String::from(
        "SELECT id, created_at, event_type, actor_id, session_id, payload_inline \
         FROM agent_event WHERE 1=1",
    );
    if session.is_some() {
        sql.push_str(" AND session_id = ?");
    }
    if kind.is_some() {
        sql.push_str(" AND event_type = ?");
    }
    if actor.is_some() {
        sql.push_str(" AND actor_id = ?");
    }
    if after.is_some() {
        sql.push_str(" AND id > ?");
    }
    sql.push_str(" ORDER BY id ASC LIMIT ?");
    let mut q = sqlx::query(&sql);
    if let Some(v) = session {
        q = q.bind(v);
    }
    if let Some(v) = kind {
        q = q.bind(v);
    }
    if let Some(v) = actor {
        q = q.bind(v);
    }
    if let Some(v) = after {
        q = q.bind(v);
    }
    q = q.bind(limit);
    let rows = q.fetch_all(s.pool()).await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(EvtRow {
            id: r.try_get("id")?,
            created_at: r.try_get("created_at")?,
            event_type: r.try_get("event_type")?,
            actor_id: r.try_get("actor_id")?,
            session_id: r.try_get("session_id").ok(),
            payload_inline: r.try_get("payload_inline").ok(),
        });
    }
    Ok(out)
}

struct EvtRow {
    id: String,
    created_at: String,
    event_type: String,
    actor_id: String,
    session_id: Option<String>,
    payload_inline: Option<String>,
}

fn print_event(r: &EvtRow) {
    let summary = summarize(&r.event_type, r.payload_inline.as_deref());
    println!(
        "{}  {}  {:<28}  actor={}  {}",
        r.created_at, r.id, r.event_type, r.actor_id, summary
    );
    let _ = &r.session_id;
}

fn summarize(kind: &str, payload: Option<&str>) -> String {
    let Some(p) = payload else {
        return String::new();
    };
    let v: serde_json::Value = match serde_json::from_str(p) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    match kind {
        k if k.starts_with("tool_call_") => {
            let tool = v
                .get("tool_name")
                .or_else(|| v.get("tool"))
                .and_then(|x| x.as_str())
                .unwrap_or("?");
            let status = v.get("status").and_then(|x| x.as_str()).unwrap_or("");
            format!(
                "tool={tool} {}",
                if status.is_empty() {
                    "".into()
                } else {
                    format!("status={status}")
                }
            )
        }
        "model_call" | "model_call_completed" | "model_call_started" => {
            let model = v.get("model").and_then(|x| x.as_str()).unwrap_or("?");
            let toks = v.get("tokens").and_then(|x| x.as_u64()).unwrap_or(0);
            format!("model={model} tokens={toks}")
        }
        _ => {
            // Truncate payload to keep the line readable.
            let s = serde_json::to_string(&v).unwrap_or_default();
            if s.len() > 120 {
                format!("{}…", &s[..120])
            } else {
                s
            }
        }
    }
}
