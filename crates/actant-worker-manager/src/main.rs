//! `actant-workers` — host every configured worker in one process.

use std::path::PathBuf;

use actant_core::{ActorId, WorkspaceId};
use actant_effects::EffectQueue;
use actant_storage::{Storage, StorageConfig};
use actant_worker_browser::{BrowserHandler, EmulatorDriver};
use actant_worker_email::{EmailHandler, RecordingMailer};
use actant_worker_file::FileHandler;
use actant_worker_manager::ManagerConfig;
use actant_worker_mcp::McpHandler;
use actant_worker_model::ModelHandler;
use actant_worker_protocol::{Handler, WorkerDescriptor, WorkerRunner};
use actant_worker_shell::ShellHandler;
use actant_worker_slack::{HttpPoster, SlackHandler};
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
    if config.shell {
        handlers.push(Box::new(ShellHandler));
    }
    if config.file {
        handlers.push(Box::new(FileHandler));
    }
    if config.model {
        handlers.push(Box::new(ModelHandler::mock()));
    }
    if config.email {
        handlers.push(Box::new(EmailHandler::new(RecordingMailer::new())));
    }
    if config.slack {
        handlers.push(Box::new(SlackHandler::new(HttpPoster::default())));
    }
    if config.browser {
        handlers.push(Box::new(BrowserHandler::new(EmulatorDriver::new(
            "actantdb browser",
        ))));
    }
    if config.mcp {
        handlers.push(Box::new(McpHandler));
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
