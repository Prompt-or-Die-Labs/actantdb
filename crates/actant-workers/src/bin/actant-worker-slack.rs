//! `actant-worker-slack` binary.

#![cfg(feature = "slack")]

use std::path::PathBuf;

use actant_core::{ActorId, WorkspaceId};
use actant_effects::EffectQueue;
use actant_storage::{Storage, StorageConfig};
use actant_worker_protocol::{WorkerDescriptor, WorkerRunner};
use actant_workers::slack::{HttpPoster, SlackHandler};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "actant-worker-slack", version)]
struct Args {
    #[arg(long, env = "ACTANTDB_DB")]
    db: PathBuf,
    #[arg(long, env = "ACTANTDB_WORKSPACE")]
    workspace: String,
    #[arg(long, default_value = "act_worker_slack")]
    actor: String,
    #[arg(long, default_value = "slack-worker-01")]
    name: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let storage = Storage::open(StorageConfig::file(&args.db)).await?;
    let queue = EffectQueue::new(storage);
    let handler = SlackHandler::new(HttpPoster::default());
    let desc = WorkerDescriptor {
        workspace_id: WorkspaceId::from_string(args.workspace),
        actor_id: ActorId::from_string(args.actor),
        name: args.name,
        capabilities: vec!["slack.post".into()],
    };
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let mut runner = WorkerRunner::new(queue, desc, vec![Box::new(handler)], rx);
    runner.run().await?;
    Ok(())
}
