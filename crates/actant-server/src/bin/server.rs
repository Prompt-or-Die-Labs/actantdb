//! `actantdb-server` — the local HTTP/WS server.

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "actantdb-server", version)]
struct Args {
    /// Bind address (host:port).
    #[arg(long, default_value = "127.0.0.1:4555", env = "ACTANTDB_BIND")]
    bind: String,

    /// SQLite database path. Defaults to in-memory.
    #[arg(long, env = "ACTANTDB_DB")]
    db: Option<PathBuf>,

    /// Path to a PEM-encoded TLS certificate chain. When set together with
    /// `--tls-key`, the server serves HTTPS instead of plain HTTP.
    #[arg(long, env = "ACTANTDB_TLS_CERT", requires = "tls_key")]
    tls_cert: Option<PathBuf>,

    /// Path to a PEM-encoded private key for the TLS certificate.
    #[arg(long, env = "ACTANTDB_TLS_KEY", requires = "tls_cert")]
    tls_key: Option<PathBuf>,
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
    let (router, _state) = actant_server::bootstrap(args.db).await?;
    actant_server::serve(router, &args.bind, args.tls_cert, args.tls_key).await?;
    tracing::info!("actantdb-server shutdown complete");
    Ok(())
}
