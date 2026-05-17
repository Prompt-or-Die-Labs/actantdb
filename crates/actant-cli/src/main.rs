//! `actantdb` — the local CLI front-end.

use std::path::PathBuf;

use actant_command::Engine;
use actant_core::{ActorId, WorkspaceId};
use actant_storage::{Storage, StorageConfig};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "actantdb", version, about = "ActantDB local CLI")]
struct Cli {
    /// SQLite database path. Defaults to ~/.actantdb/actant.db.
    #[arg(long, env = "ACTANTDB_DB", global = true)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::enum_variant_names)]
enum Command {
    /// Apply migrations to the configured database (creates it if missing).
    Migrate {
        /// Print the pending migrations without applying them.
        #[arg(long)]
        dry_run: bool,
    },
    /// Back up the database to a file (consistent snapshot via WAL checkpoint).
    Backup {
        /// Destination path.
        #[arg(long)]
        to: PathBuf,
    },
    /// Restore the database from a file.
    Restore {
        /// Source path.
        #[arg(long)]
        from: PathBuf,
    },
    /// Start the HTTP/WS server.
    Serve {
        /// Bind address.
        #[arg(long, default_value = "127.0.0.1:4555")]
        bind: String,
        /// PEM cert path. When set together with --tls-key, serves HTTPS.
        #[arg(long, requires = "tls_key")]
        tls_cert: Option<PathBuf>,
        /// PEM private-key path.
        #[arg(long, requires = "tls_cert")]
        tls_key: Option<PathBuf>,
    },
    /// Dispatch a single command and print the result.
    Command {
        /// Workspace id.
        #[arg(long)]
        workspace: String,
        /// Actor id.
        #[arg(long)]
        actor: String,
        /// Command type (one of the alpha commands).
        #[arg(long, value_name = "TYPE")]
        kind: String,
        /// JSON input (use '-' for stdin).
        #[arg(long, default_value = "{}")]
        input: String,
    },
    /// Print Chronicle events for a session.
    Events {
        /// Session id.
        #[arg(long)]
        session: String,
    },
    /// List pending approvals in a workspace.
    Approvals {
        /// Workspace id.
        #[arg(long)]
        workspace: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();
    let cli = Cli::parse();
    let db_path = cli.db.unwrap_or_else(default_db_path);

    match cli.command {
        Command::Migrate { dry_run } => {
            if dry_run {
                // Open *read-only enough* to list applied; report what would
                // be applied. Embedded migration set is fixed-list, so the
                // pending set is `MIGRATIONS - applied`.
                let mut cfg = StorageConfig::file(&db_path);
                cfg.apply_migrations = false;
                let s = Storage::open(cfg).await?;
                let applied = s.applied_migrations().await.unwrap_or_default();
                let all = [
                    "0001_initial",
                    "0002_extended_primitives",
                    "0003_ai_native_and_reliability",
                ];
                let pending: Vec<&str> = all
                    .iter()
                    .filter(|m| !applied.iter().any(|a| a == *m))
                    .copied()
                    .collect();
                println!("dry-run: applied={applied:?} pending={pending:?}");
                if pending.is_empty() {
                    println!("nothing to apply");
                }
            } else {
                let s = Storage::open(StorageConfig::file(&db_path)).await?;
                let applied = s.applied_migrations().await?;
                println!("migrated {}; applied = {:?}", db_path.display(), applied);
            }
        }
        Command::Backup { to } => {
            // Open the database, run WAL checkpoint, then copy the file.
            let s = Storage::open(StorageConfig::file(&db_path)).await?;
            sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(s.pool())
                .await
                .map_err(|e| anyhow::anyhow!("wal_checkpoint: {e}"))?;
            // Drop the connection so file is in a consistent state.
            drop(s);
            std::fs::copy(&db_path, &to)?;
            println!("backed up {} → {}", db_path.display(), to.display());
        }
        Command::Restore { from } => {
            // Refuse to overwrite a live database without explicit force in
            // a future iteration. For Phase 6 we do the simplest correct
            // thing: copy the file in.
            if db_path.exists() {
                eprintln!(
                    "warning: overwriting existing database at {}",
                    db_path.display()
                );
            }
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&from, &db_path)?;
            // Sanity check: re-open succeeds.
            let _s = Storage::open(StorageConfig::file(&db_path)).await?;
            println!("restored {} ← {}", db_path.display(), from.display());
        }
        Command::Serve {
            bind,
            tls_cert,
            tls_key,
        } => {
            let (router, _state) = actant_server::bootstrap(Some(db_path.clone())).await?;
            actant_server::serve(router, &bind, tls_cert, tls_key).await?;
        }
        Command::Command {
            workspace,
            actor,
            kind,
            input,
        } => {
            let s = Storage::open(StorageConfig::file(&db_path)).await?;
            let engine = Engine::new(s);
            let input_value: serde_json::Value = if input == "-" {
                let mut buf = String::new();
                use std::io::Read;
                std::io::stdin().read_to_string(&mut buf)?;
                serde_json::from_str(&buf)?
            } else {
                serde_json::from_str(&input)?
            };
            let ws = WorkspaceId::from_string(workspace);
            let actor = ActorId::from_string(actor);
            let out = engine
                .dispatch(&ws, &actor, &kind, input_value, None)
                .await?;
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Command::Events { session } => {
            let s = Storage::open(StorageConfig::file(&db_path)).await?;
            let sid = actant_core::SessionId::from_string(session);
            let events = s.events_in_session(&sid).await?;
            for e in events {
                println!(
                    "{}  {:<24}  {}",
                    &e.created_at,
                    e.event_type,
                    e.payload_inline.as_deref().unwrap_or("")
                );
            }
        }
        Command::Approvals { workspace } => {
            let s = Storage::open(StorageConfig::file(&db_path)).await?;
            let rows = sqlx::query_as::<_, (String, Option<String>, String, String)>(
                "SELECT id, tool_call_id, summary, status
                 FROM approval_request
                 WHERE workspace_id = ? AND status = 'pending'
                 ORDER BY created_at ASC",
            )
            .bind(&workspace)
            .fetch_all(s.pool())
            .await?;
            if rows.is_empty() {
                eprintln!("(no pending approvals)");
            }
            for (id, tool_call, summary, status) in rows {
                println!(
                    "{}  tool_call={}  status={}  summary={}",
                    id,
                    tool_call.unwrap_or_default(),
                    status,
                    summary
                );
            }
        }
    }
    Ok(())
}

fn default_db_path() -> PathBuf {
    let mut p = dirs_local();
    p.push(".actantdb");
    p.push("actant.db");
    p
}

fn dirs_local() -> PathBuf {
    if let Some(h) = std::env::var_os("HOME") {
        PathBuf::from(h)
    } else {
        PathBuf::from(".")
    }
}
