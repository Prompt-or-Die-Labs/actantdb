//! `actantdb status` — print aggregated server / DB / backup state.

use std::path::Path;
use std::time::Duration;

use actant_storage::{Storage, StorageConfig};
use serde_json::json;
use sqlx::Row;

/// Run the status command.
pub async fn run(db_path: &Path, as_json: bool) -> anyhow::Result<()> {
    let server = probe_server().await;
    let (db_open_err, applied, sessions, events, last_full_lsn, last_inc_lsn) =
        match Storage::open(StorageConfig::file(db_path)).await {
            Ok(s) => {
                let applied = s.applied_migrations().await.unwrap_or_default();
                let sessions: i64 = sqlx::query("SELECT COUNT(*) AS n FROM session")
                    .fetch_one(s.pool())
                    .await
                    .map(|r| r.try_get::<i64, _>("n").unwrap_or(0))
                    .unwrap_or(0);
                let events: i64 = sqlx::query("SELECT COUNT(*) AS n FROM agent_event")
                    .fetch_one(s.pool())
                    .await
                    .map(|r| r.try_get::<i64, _>("n").unwrap_or(0))
                    .unwrap_or(0);
                let last_full: Option<i64> =
                    sqlx::query("SELECT last_lsn FROM actant_backup_state WHERE id = 1")
                        .fetch_optional(s.pool())
                        .await
                        .ok()
                        .flatten()
                        .and_then(|r| r.try_get::<i64, _>("last_lsn").ok());
                (None, applied, sessions, events, last_full, last_full)
            }
            Err(e) => (Some(e.to_string()), Vec::new(), 0, 0, None, None),
        };

    let db_size = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);

    if as_json {
        let obj = json!({
            "server": {
                "url": "http://127.0.0.1:4555",
                "ready": server.ready,
                "error": server.err,
            },
            "db": {
                "path": db_path.display().to_string(),
                "size_bytes": db_size,
                "open_error": db_open_err,
                "applied_migrations": applied,
                "sessions": sessions,
                "events": events,
            },
            "backup": {
                "last_full_lsn": last_full_lsn,
                "last_incremental_lsn": last_inc_lsn,
            },
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        println!("Server:");
        if server.ready {
            println!("  http://127.0.0.1:4555  (ready)");
        } else {
            println!(
                "  http://127.0.0.1:4555  (down: {})",
                server.err.as_deref().unwrap_or("unreachable")
            );
        }
        println!();
        println!("Database:");
        println!("  path:                {}", db_path.display());
        println!("  size:                {db_size} bytes");
        if let Some(e) = &db_open_err {
            println!("  open error:          {e}");
        } else {
            println!("  applied migrations:  {}", applied.join(", "));
            println!("  sessions:            {sessions}");
            println!("  events:              {events}");
        }
        println!();
        println!("Backup state:");
        match last_full_lsn {
            Some(n) => println!("  last LSN captured:   {n}"),
            None => println!("  last LSN captured:   (none yet)"),
        }
    }
    Ok(())
}

struct ServerProbe {
    ready: bool,
    err: Option<String>,
}

async fn probe_server() -> ServerProbe {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ServerProbe {
                ready: false,
                err: Some(format!("client build: {e}")),
            }
        }
    };
    match client
        .get("http://127.0.0.1:4555/v1/healthz/ready")
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => ServerProbe {
            ready: true,
            err: None,
        },
        Ok(r) => ServerProbe {
            ready: false,
            err: Some(format!("HTTP {}", r.status())),
        },
        Err(e) => ServerProbe {
            ready: false,
            err: Some(e.to_string()),
        },
    }
}
