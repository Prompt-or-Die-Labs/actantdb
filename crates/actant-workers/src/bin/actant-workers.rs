//! `actant-workers` — host every configured worker in one process.
//!
//! The set of workers actually available is determined at compile time
//! by the cargo features enabled on `actant-workers`. The runtime `--*`
//! flags only flip whichever of those compiled-in workers are active.

#![cfg(feature = "manager")]

use std::path::PathBuf;

use actant_core::{ActorId, WorkspaceId};
use actant_effects::EffectQueue;
use actant_storage::{Storage, StorageConfig};
use actant_worker_protocol::{Handler, WorkerDescriptor, WorkerRunner};
use actant_workers::manager::ManagerConfig;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "actant-workers", version)]
struct Args {
    #[arg(long, env = "ACTANTDB_DB")]
    db: PathBuf,
    #[arg(long, env = "ACTANTDB_WORKSPACE")]
    workspace: String,
    #[arg(long, default_value = "act_workers")]
    actor: String,
    #[arg(long, default_value = "worker-manager-01")]
    name: String,
    /// Enable every worker.
    #[arg(long)]
    all: bool,
    #[arg(long)]
    shell: bool,
    #[arg(long)]
    file: bool,
    #[arg(long)]
    model: bool,
    #[arg(long)]
    email: bool,
    #[arg(long)]
    slack: bool,
    #[arg(long)]
    browser: bool,
    #[arg(long)]
    mcp: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let config = if args.all {
        ManagerConfig::all()
    } else {
        ManagerConfig {
            shell: args.shell,
            file: args.file,
            model: args.model,
            email: args.email,
            slack: args.slack,
            browser: args.browser,
            mcp: args.mcp,
        }
    };
    if !config.any_enabled() {
        anyhow::bail!("no workers selected; pass --all or one of --shell/--file/...");
    }

    let storage = Storage::open(StorageConfig::file(&args.db)).await?;
    let queue = EffectQueue::new(storage);

    let mut handlers: Vec<Box<dyn Handler>> = Vec::new();

    #[cfg(feature = "shell")]
    if config.shell {
        handlers.push(Box::new(actant_workers::shell::ShellHandler));
    }
    #[cfg(not(feature = "shell"))]
    if config.shell {
        anyhow::bail!("--shell requested but actant-workers was built without the `shell` feature");
    }

    #[cfg(feature = "file")]
    if config.file {
        handlers.push(Box::new(actant_workers::file::FileHandler));
    }
    #[cfg(not(feature = "file"))]
    if config.file {
        anyhow::bail!("--file requested but actant-workers was built without the `file` feature");
    }

    #[cfg(feature = "model")]
    if config.model {
        handlers.push(Box::new(actant_workers::model::ModelHandler::mock()));
    }
    #[cfg(not(feature = "model"))]
    if config.model {
        anyhow::bail!("--model requested but actant-workers was built without the `model` feature");
    }

    #[cfg(feature = "email")]
    if config.email {
        handlers.push(Box::new(actant_workers::email::EmailHandler::new(
            actant_workers::email::RecordingMailer::new(),
        )));
    }
    #[cfg(not(feature = "email"))]
    if config.email {
        anyhow::bail!("--email requested but actant-workers was built without the `email` feature");
    }

    #[cfg(feature = "slack")]
    if config.slack {
        handlers.push(Box::new(actant_workers::slack::SlackHandler::new(
            actant_workers::slack::HttpPoster::default(),
        )));
    }
    #[cfg(not(feature = "slack"))]
    if config.slack {
        anyhow::bail!("--slack requested but actant-workers was built without the `slack` feature");
    }

    #[cfg(feature = "browser")]
    if config.browser {
        handlers.push(Box::new(actant_workers::browser::BrowserHandler::new(
            actant_workers::browser::EmulatorDriver::new("actantdb browser"),
        )));
    }
    #[cfg(not(feature = "browser"))]
    if config.browser {
        anyhow::bail!(
            "--browser requested but actant-workers was built without the `browser` feature"
        );
    }

    #[cfg(feature = "mcp")]
    if config.mcp {
        handlers.push(Box::new(actant_workers::mcp::McpHandler));
    }
    #[cfg(not(feature = "mcp"))]
    if config.mcp {
        anyhow::bail!("--mcp requested but actant-workers was built without the `mcp` feature");
    }

    let caps: Vec<String> = config
        .capabilities()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let desc = WorkerDescriptor {
        workspace_id: WorkspaceId::from_string(args.workspace),
        actor_id: ActorId::from_string(args.actor),
        name: args.name,
        capabilities: caps,
    };
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let mut runner = WorkerRunner::new(queue, desc, handlers, rx);
    runner.run().await?;
    Ok(())
}
