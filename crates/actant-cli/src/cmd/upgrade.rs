//! `actantdb upgrade [--check]` — check or guide the upgrade flow.

use std::process::Command;
use std::time::Duration;

/// Run the upgrade command.
pub async fn run(check: bool) -> anyhow::Result<()> {
    if !check {
        println!("To upgrade: npm install -g @actantdb/studio@latest");
        println!("(the studio package bundles the actantdb binary entrypoint)");
        return Ok(());
    }

    let current = env!("CARGO_PKG_VERSION");
    // Try `npm view` first (fast, no http client needed if user has npm).
    // Fall back to the npm registry HTTP API.
    let latest = npm_view_version("@actantdb/all")
        .or_else(|| npm_view_version("@actantdb/studio"))
        .ok_or_else(|| anyhow::anyhow!("could not resolve latest version from npm"));

    match latest {
        Ok(v) => {
            println!("you're on {current}, latest is {v}");
            if v.trim() != current {
                println!("upgrade: npm install -g @actantdb/studio@latest");
            } else {
                println!("(up to date)");
            }
        }
        Err(_) => match registry_latest_version("@actantdb/all").await {
            Ok(v) => {
                println!("you're on {current}, latest is {v}");
                if v.trim() != current {
                    println!("upgrade: npm install -g @actantdb/studio@latest");
                } else {
                    println!("(up to date)");
                }
            }
            Err(e) => {
                eprintln!("could not check latest version: {e}");
                println!("you're on {current}");
            }
        },
    }
    Ok(())
}

fn npm_view_version(pkg: &str) -> Option<String> {
    let out = Command::new("npm")
        .args(["view", pkg, "version"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

async fn registry_latest_version(pkg: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let url = format!("https://registry.npmjs.org/{pkg}/latest");
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("registry {url} returned {}", resp.status());
    }
    let json: serde_json::Value = resp.json().await?;
    json.get("version")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("no `version` field in registry response"))
}
