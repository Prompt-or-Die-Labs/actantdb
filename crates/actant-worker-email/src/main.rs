//! `actant-worker-email` binary.

use std::path::PathBuf;

use actant_core::{ActorId, WorkspaceId};
use actant_effects::EffectQueue;
use actant_storage::{Storage, StorageConfig};
use actant_worker_email::{EmailHandler, RecordingMailer};
use actant_worker_protocol::{WorkerDescriptor, WorkerRunner};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "actant-worker-email", version)]
struct Args {
    #[arg(long, env = "ACTANTDB_DB")]
    db: PathBuf,
    #[arg(long, env = "ACTANTDB_WORKSPACE")]
    workspace: String,
    #[arg(long, default_value = "act_worker_email")]
    actor: String,
    #[arg(long, default_value = "email-worker-01")]
    name: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let storage = Storage::open(StorageConfig::file(&args.db)).await?;
    let queue = EffectQueue::new(storage);
    let handler = EmailHandler::new(RecordingMailer::new());
    let desc = WorkerDescriptor {
        workspace_id: WorkspaceId::from_string(args.workspace),
        actor_id: ActorId::from_string(args.actor),
        name: args.name,
        capabilities: vec!["email.send".into()],
    };
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let mut runner = WorkerRunner::new(queue, desc, vec![Box::new(handler)], rx);
    runner.run().await?;
    Ok(())
}
