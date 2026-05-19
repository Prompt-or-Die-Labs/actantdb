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
    // Fail loud on Postgres bootstrap until the substrate actually supports it.
    // Storage audit gap #1 was: `ACTANTDB_DATABASE_URL` was silently ignored
    // and the server opened SQLite anyway — Helm `storage.backend=postgres`
    // looked healthy while writing to a local file. Refuse to start so the
    // mismatch surfaces immediately. Remove this guard once
    // `crates/actant-storage::PgStorage` ships the full repo surface (see
    // STORAGE_AUDIT.md gap #2 + GAPS.md row #5).
    if let Ok(url) = std::env::var("ACTANTDB_DATABASE_URL") {
        if !url.is_empty() {
            anyhow::bail!(
                "ACTANTDB_DATABASE_URL is set ({}) but the Postgres backend \
                 is not yet usable end-to-end: `crates/actant-storage::PgStorage` \
                 ships 7 of the 87 tables and 0 of the repo methods needed by \
                 the command engine. The server refuses to start so this \
                 doesn't silently downgrade to SQLite. Track:\n  \
                 - STORAGE_AUDIT.md gap #2\n  \
                 - GAPS.md row #5\n\
                 To run on SQLite anyway: unset ACTANTDB_DATABASE_URL and \
                 pass --db <path>.",
                redact_db_url(&url)
            );
        }
    }
    let (router, _state) = actant_server::bootstrap(args.db).await?;
    actant_server::serve(router, &args.bind, args.tls_cert, args.tls_key).await?;
    tracing::info!("actantdb-server shutdown complete");
    Ok(())
}

/// Redact the password from a `postgres://user:pass@host/db` URL for safe
/// inclusion in error messages and logs.
fn redact_db_url(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        if let Some(at) = after_scheme.find('@') {
            let userinfo = &after_scheme[..at];
            let rest = &after_scheme[at..];
            if let Some(colon) = userinfo.find(':') {
                let user = &userinfo[..colon];
                return format!("{}://{}:***{}", &url[..scheme_end], user, rest);
            }
        }
    }
    url.to_string()
}
