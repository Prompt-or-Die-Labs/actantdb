//! `actantdb` — the local CLI front-end.

use std::path::PathBuf;

use actant_command::Engine;
use actant_core::{ActorId, WorkspaceId};
use actant_storage::{Storage, StorageConfig};
use clap::{Parser, Subcommand, ValueEnum};

/// Backup mode flag for `actantdb backup --mode`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum BackupMode {
    /// Consistent single-file copy. Default.
    Full,
    /// Manifest-based incremental: full + WAL increments into a directory.
    Incremental,
}

/// Minimal RFC-3339-ish timestamp (UTC, second precision). The CLI doesn't
/// depend on `chrono` so we hand-roll it from `time` types.
fn chrono_rfc3339() -> String {
    use time::OffsetDateTime;
    let n = OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        n.year(),
        u8::from(n.month()),
        n.day(),
        n.hour(),
        n.minute(),
        n.second(),
    )
}

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
    /// Back up the database.
    ///
    /// `--mode=full` (default): consistent file copy after WAL checkpoint.
    /// `--mode=incremental`: write a full snapshot (only when needed) plus
    /// a WAL increment to `<to>` and update `<to>/manifest.json`. The
    /// dir-based incremental mode is the path used by automated backup
    /// pipelines; `--mode=full` to a single file is the path used by
    /// interactive snapshots.
    Backup {
        /// Destination path. Single file for `--mode=full`, directory for
        /// `--mode=incremental`.
        #[arg(long)]
        to: PathBuf,
        /// Backup mode. Default is `full`.
        #[arg(long, default_value = "full")]
        mode: BackupMode,
    },
    /// Restore the database.
    ///
    /// `--from <file>` restores a full snapshot (the `--mode=full` output).
    /// `--from <dir>` reads `<dir>/manifest.json`, copies in the most
    /// recent full snapshot, and re-applies WAL increments in order. Stop
    /// early with `--at-lsn N`.
    Restore {
        /// Source path. File OR directory; the CLI auto-detects.
        #[arg(long)]
        from: PathBuf,
        /// For directory-mode restore, stop replaying increments at this LSN.
        #[arg(long)]
        at_lsn: Option<u64>,
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
        Command::Backup { to, mode } => match mode {
            BackupMode::Full => {
                // Open the database, run WAL checkpoint, then copy the file.
                let s = Storage::open(StorageConfig::file(&db_path)).await?;
                sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                    .execute(s.pool())
                    .await
                    .map_err(|e| anyhow::anyhow!("wal_checkpoint: {e}"))?;
                drop(s);
                std::fs::copy(&db_path, &to)?;
                println!("backed up {} → {} (full)", db_path.display(), to.display());
            }
            BackupMode::Incremental => {
                // Incremental: write `<to>/full-<ts>.sqlite` (only when
                // the manifest has no full yet — every subsequent run
                // appends a WAL increment), update `<to>/manifest.json`.
                use actant_storage::{
                    backup_sha256_hex, EntryKind, Manifest, ManifestEntry,
                };
                std::fs::create_dir_all(&to)?;
                let manifest_path = to.join("manifest.json");
                let mut manifest = Manifest::read_or_default(&manifest_path)?;
                let s = Storage::open(StorageConfig::file(&db_path)).await?;
                let now_ts = chrono_rfc3339();

                let last_full_lsn = manifest.last_full_lsn();
                let from_lsn = manifest.last_lsn();
                if last_full_lsn.is_none() {
                    // First call: take a full snapshot.
                    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                        .execute(s.pool())
                        .await
                        .map_err(|e| anyhow::anyhow!("wal_checkpoint: {e}"))?;
                    let lsn = s.last_lsn().await?;
                    let file_name = format!("full-{lsn:020}.sqlite");
                    let snapshot_path = to.join(&file_name);
                    drop(s);
                    let bytes = std::fs::read(&db_path)?;
                    let sha = backup_sha256_hex(&bytes);
                    std::fs::write(&snapshot_path, &bytes)?;
                    manifest.entries.push(ManifestEntry {
                        kind: EntryKind::Full,
                        file: file_name,
                        lsn,
                        previous_lsn: lsn,
                        sha256: sha,
                        size_bytes: bytes.len() as u64,
                        taken_at: now_ts.clone(),
                    });
                    manifest.write(&manifest_path)?;
                    println!("backed up {} → {} (full @ lsn {})", db_path.display(), to.display(), lsn);
                } else {
                    // Subsequent call: capture WAL since the last entry.
                    let inc = s.wal_frames_since(from_lsn).await?;
                    let lsn = inc.lsn;
                    drop(s);
                    let bytes = serde_json::to_vec(&inc)
                        .map_err(|e| anyhow::anyhow!("encode wal increment: {e}"))?;
                    let sha = backup_sha256_hex(&bytes);
                    let file_name = format!("wal-{lsn:020}.json");
                    std::fs::write(to.join(&file_name), &bytes)?;
                    manifest.entries.push(ManifestEntry {
                        kind: EntryKind::Incremental,
                        file: file_name,
                        lsn,
                        previous_lsn: from_lsn,
                        sha256: sha,
                        size_bytes: bytes.len() as u64,
                        taken_at: now_ts.clone(),
                    });
                    manifest.write(&manifest_path)?;
                    println!("backed up {} → {} (incremental: {} → {})", db_path.display(), to.display(), from_lsn, lsn);
                }
            }
        },
        Command::Restore { from, at_lsn } => {
            if db_path.exists() {
                eprintln!(
                    "warning: overwriting existing database at {}",
                    db_path.display()
                );
            }
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            if from.is_dir() {
                // Directory mode: read manifest + apply full + WAL increments.
                use actant_storage::{EntryKind, Manifest, WalIncrement};
                let manifest = Manifest::read_or_default(&from.join("manifest.json"))?;
                if manifest.entries.is_empty() {
                    anyhow::bail!("no entries in {}/manifest.json", from.display());
                }
                let last_full = manifest
                    .entries
                    .iter()
                    .rposition(|e| matches!(e.kind, EntryKind::Full))
                    .ok_or_else(|| anyhow::anyhow!("no full snapshot in manifest"))?;
                let full_entry = &manifest.entries[last_full];
                std::fs::copy(from.join(&full_entry.file), &db_path)?;
                let mut current_lsn = full_entry.lsn;
                let stop = at_lsn.unwrap_or(u64::MAX);
                if current_lsn > stop {
                    anyhow::bail!("requested at_lsn={} is before the latest full snapshot lsn={}", stop, current_lsn);
                }
                let s = Storage::open(StorageConfig::file(&db_path)).await?;
                for entry in &manifest.entries[last_full + 1..] {
                    if !matches!(entry.kind, EntryKind::Incremental) {
                        continue;
                    }
                    if entry.lsn > stop {
                        break;
                    }
                    let bytes = std::fs::read(from.join(&entry.file))?;
                    let inc: WalIncrement = serde_json::from_slice(&bytes)
                        .map_err(|e| anyhow::anyhow!("decode {}: {}", entry.file, e))?;
                    s.apply_wal_frames(&inc).await?;
                    current_lsn = entry.lsn;
                }
                drop(s);
                println!("restored {} ← {} (lsn {})", db_path.display(), from.display(), current_lsn);
            } else {
                std::fs::copy(&from, &db_path)?;
                let _s = Storage::open(StorageConfig::file(&db_path)).await?;
                println!("restored {} ← {}", db_path.display(), from.display());
            }
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
