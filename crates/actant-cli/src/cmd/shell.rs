//! `actantdb shell` — interactive read-only prompt.

use std::path::Path;

use actant_storage::{Storage, StorageConfig};
use comfy_table::Table;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use sqlx::Row;

use crate::cli_errors;

/// Run the interactive shell.
pub async fn run(db_path: &Path) -> anyhow::Result<()> {
    let s = Storage::open(StorageConfig::file(db_path)).await?;
    let mut rl = DefaultEditor::new()?;
    println!("actantdb shell (read-only). Type `help` for commands. `exit` to quit.");
    loop {
        let line = match rl.readline("actantdb> ") {
            Ok(l) => l,
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("readline error: {e}");
                break;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let _ = rl.add_history_entry(trimmed);
        match dispatch(&s, trimmed).await {
            Ok(should_exit) if should_exit => break,
            Ok(_) => {}
            Err(e) => cli_errors::print_public_error(&e),
        }
    }
    Ok(())
}

async fn dispatch(s: &Storage, line: &str) -> anyhow::Result<bool> {
    let mut parts = line.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();
    match cmd {
        "exit" | "quit" => Ok(true),
        "help" | "?" => {
            print_help();
            Ok(false)
        }
        "events" => {
            let limit: i64 = parse_kv(&rest, "--limit").unwrap_or(20);
            list_events(s, limit).await?;
            Ok(false)
        }
        "sessions" => {
            list_sessions(s).await?;
            Ok(false)
        }
        "get" => {
            let id = rest
                .first()
                .ok_or_else(|| cli_errors::invalid_input("usage: get <event_id>"))?;
            get_event(s, id).await?;
            Ok(false)
        }
        other => {
            eprintln!("unknown command: `{other}`. Type `help`.");
            Ok(false)
        }
    }
}

fn parse_kv<T: std::str::FromStr>(args: &[&str], key: &str) -> Option<T> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if *a == key {
            if let Some(v) = it.next() {
                if let Ok(parsed) = v.parse::<T>() {
                    return Some(parsed);
                }
            }
        }
    }
    None
}

fn print_help() {
    println!("Commands:");
    println!("  events [--limit N]   list last N events (default 20)");
    println!("  sessions             list sessions");
    println!("  get <event_id>       fetch one event row");
    println!("  help                 show this help");
    println!("  exit                 quit");
}

async fn list_events(s: &Storage, limit: i64) -> anyhow::Result<()> {
    let rows = sqlx::query(
        "SELECT id, created_at, event_type, actor_id \
         FROM agent_event ORDER BY id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(s.pool())
    .await?;
    let mut table = Table::new();
    table.set_header(vec!["id", "created_at", "type", "actor"]);
    for r in rows.iter().rev() {
        table.add_row(vec![
            r.try_get::<String, _>("id").unwrap_or_default(),
            r.try_get::<String, _>("created_at").unwrap_or_default(),
            r.try_get::<String, _>("event_type").unwrap_or_default(),
            r.try_get::<String, _>("actor_id").unwrap_or_default(),
        ]);
    }
    println!("{table}");
    Ok(())
}

async fn list_sessions(s: &Storage) -> anyhow::Result<()> {
    let rows = sqlx::query(
        "SELECT id, workspace_id, actor_id, started_at \
         FROM session ORDER BY started_at DESC LIMIT 50",
    )
    .fetch_all(s.pool())
    .await?;
    let mut table = Table::new();
    table.set_header(vec!["id", "workspace", "actor", "started_at"]);
    for r in &rows {
        table.add_row(vec![
            r.try_get::<String, _>("id").unwrap_or_default(),
            r.try_get::<String, _>("workspace_id").unwrap_or_default(),
            r.try_get::<String, _>("actor_id").unwrap_or_default(),
            r.try_get::<String, _>("started_at").unwrap_or_default(),
        ]);
    }
    println!("{table}");
    Ok(())
}

async fn get_event(s: &Storage, id: &str) -> anyhow::Result<()> {
    let row = sqlx::query(
        "SELECT id, created_at, event_type, actor_id, session_id, payload_inline \
         FROM agent_event WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(s.pool())
    .await?;
    match row {
        None => println!("(not found)"),
        Some(r) => {
            println!(
                "id          {}",
                r.try_get::<String, _>("id").unwrap_or_default()
            );
            println!(
                "created_at  {}",
                r.try_get::<String, _>("created_at").unwrap_or_default()
            );
            println!(
                "event_type  {}",
                r.try_get::<String, _>("event_type").unwrap_or_default()
            );
            println!(
                "actor_id    {}",
                r.try_get::<String, _>("actor_id").unwrap_or_default()
            );
            println!(
                "session_id  {}",
                r.try_get::<Option<String>, _>("session_id")
                    .unwrap_or(None)
                    .unwrap_or_default()
            );
            println!(
                "payload     {}",
                r.try_get::<Option<String>, _>("payload_inline")
                    .unwrap_or(None)
                    .unwrap_or_default()
            );
        }
    }
    Ok(())
}
