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

    /// Allow binding to a non-loopback address without TLS. By default,
    /// `actantdb-server` refuses this combination. Setting this is a
    /// production footgun — see UI_AUTH_DESIGN.md §6.
    #[arg(long, env = "ACTANTDB_INSECURE_PUBLIC")]
    insecure_public: bool,

    /// Honor reverse-proxy headers (`X-Forwarded-For`, `Forwarded`,
    /// `X-Real-IP`) even when bound to loopback. Without this flag the
    /// server refuses forwarded requests in local-mode to prevent a
    /// trivial bypass of the "loopback = trusted" assumption.
    #[arg(long, env = "ACTANTDB_TRUST_PROXY")]
    trust_proxy: bool,
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
    let local_mode = actant_server::auth_routes::is_bind_loopback(&args.bind);
    let tls_enabled = args.tls_cert.is_some() && args.tls_key.is_some();

    // Refuse to start when bound non-loopback without TLS (the single biggest
    // statically-preventable footgun — see UI_AUTH_DESIGN.md §6).
    if !local_mode && !tls_enabled && !args.insecure_public {
        anyhow::bail!(
            "actantdb-server: refusing to bind non-loopback ({bind}) without TLS.\n\
             Pass --tls-cert + --tls-key, or pass --insecure-public (NOT \
             recommended — see UI_AUTH_DESIGN.md §6 / 'Defenses').",
            bind = args.bind
        );
    }

    let router = if let Some(url) = std::env::var("ACTANTDB_DATABASE_URL")
        .ok()
        .filter(|url| !url.is_empty())
    {
        let (router, _state) =
            actant_server::bootstrap_postgres_with_mode(&url, local_mode, args.trust_proxy).await?;
        eprintln!(
            "(postgres mode: {}; Studio/admin routes return 501 until their SQL is backend-neutral)",
            redact_db_url(&url)
        );
        router
    } else {
        let (router, _state, link_code) =
            actant_server::bootstrap_with_mode(args.db, local_mode, tls_enabled, args.trust_proxy)
                .await?;

        if let Some(code) = link_code.as_deref() {
            eprintln!();
            eprintln!("A one-time linking code is required to claim this workspace:");
            eprintln!();
            eprintln!("    Code:    {code}");
            eprintln!("    Expires: in 15 minutes");
            eprintln!();
            let proto = if tls_enabled { "https" } else { "http" };
            eprintln!("Open: {proto}://{bind}/link", bind = args.bind);
            eprintln!("Or:   {proto}://{bind}/link/{code}", bind = args.bind);
            eprintln!();
            eprintln!("(this code rotates on each restart until ownership is claimed)");
            eprintln!();
        } else if local_mode {
            eprintln!("(local mode: no password required)");
        }
        router
    };

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
