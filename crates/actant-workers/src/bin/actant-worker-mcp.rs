//! `actant-worker-mcp` — bridge worker. Wraps a configured stdio MCP
//! server program and runs the standard worker loop, claiming `mcp.call`
//! effects and dispatching them through `McpStdioClient`.

#![cfg(feature = "mcp")]

use std::path::PathBuf;

use actant_core::{ActorId, WorkspaceId};
use actant_effects::EffectQueue;
use actant_storage::{Storage, StorageConfig};
use actant_worker_protocol::{WorkerDescriptor, WorkerRunner};
use actant_workers::mcp::McpHandler;
use clap::Parser;
use tokio::sync::watch;

#[derive(Debug, Parser)]
#[command(name = "actant-worker-mcp", version)]
struct Args {
    /// SQLite database path.
    #[arg(long, env = "ACTANTDB_DB", default_value = "./actant.db")]
    db: PathBuf,
    /// Workspace id this worker registers under.
    #[arg(long, env = "ACTANTDB_WORKSPACE", default_value = "ws_default")]
    workspace: String,
    /// Actor id assigned to this worker.
    #[arg(long, env = "ACTANTDB_ACTOR", default_value = "act_worker_mcp")]
    actor: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let args = Args::parse();
    let storage = Storage::open(StorageConfig::file(&args.db)).await?;
    let queue = EffectQueue::new(storage);
    let (_tx, rx) = watch::channel(false);
    let descriptor = WorkerDescriptor {
        workspace_id: WorkspaceId::from_string(args.workspace),
        actor_id: ActorId::from_string(args.actor),
        name: "actant-worker-mcp".into(),
        capabilities: vec!["mcp.call".into()],
    };
    let mut runner = WorkerRunner::new(queue, descriptor, vec![Box::new(McpHandler)], rx);
    tracing::info!("actant-worker-mcp starting");
    runner.run().await?;
    Ok(())
}
