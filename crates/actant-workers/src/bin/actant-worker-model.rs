//! `actant-worker-model` binary.

#![cfg(feature = "model")]

use std::path::PathBuf;

use actant_core::{ActorId, WorkspaceId};
use actant_effects::EffectQueue;
use actant_storage::{Storage, StorageConfig};
use actant_worker_protocol::{WorkerDescriptor, WorkerRunner};
use actant_workers::model::{ModelHandler, Provider};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "actant-worker-model", version)]
struct Args {
    #[arg(long, env = "ACTANTDB_DB")]
    db: PathBuf,
    #[arg(long, env = "ACTANTDB_WORKSPACE")]
    workspace: String,
    #[arg(long, default_value = "act_worker_model")]
    actor: String,
    #[arg(long, default_value = "model-worker-01")]
    name: String,
    /// OpenAI-compatible base URL; defaults to mock if unset.
    #[arg(long, env = "ACTANTDB_MODEL_BASE_URL")]
    base_url: Option<String>,
    /// OpenAI-compatible API key.
    #[arg(long, env = "ACTANTDB_MODEL_API_KEY")]
    api_key: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let provider = match (args.base_url, args.api_key) {
        (Some(base_url), Some(api_key)) => Provider::OpenAi { base_url, api_key },
        _ => Provider::Mock,
    };
    let handler = ModelHandler { provider };

    let storage = Storage::open(StorageConfig::file(&args.db)).await?;
    let queue = EffectQueue::new(storage);
    let desc = WorkerDescriptor {
        workspace_id: WorkspaceId::from_string(args.workspace),
        actor_id: ActorId::from_string(args.actor),
        name: args.name,
        capabilities: vec!["model.call".into()],
    };
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let mut runner = WorkerRunner::new(queue, desc, vec![Box::new(handler)], rx);
    runner.run().await?;
    Ok(())
}
