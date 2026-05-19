//! `actantdb dev` — watch source dirs and react to changes.
//!
//! Re-validates policy JSON files against [`actant_policy::PolicyDoc`] and
//! re-runs the contracts codegen when files change.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

/// Run the dev watch command.
pub async fn run(dirs: Vec<PathBuf>) -> anyhow::Result<()> {
    let watch_dirs = if dirs.is_empty() {
        vec![
            PathBuf::from("commands"),
            PathBuf::from("policies"),
            PathBuf::from("templates"),
            PathBuf::from("crates/actant-contracts/src"),
        ]
    } else {
        dirs
    };

    // Bridge notify's sync callback to tokio via an unbounded channel.
    let (tx, mut rx) = mpsc::unbounded_channel::<notify::Result<notify::Event>>();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )?;

    let mut watched_any = false;
    for d in &watch_dirs {
        if d.exists() {
            watcher.watch(d, RecursiveMode::Recursive)?;
            println!("watching {}", d.display());
            watched_any = true;
        } else {
            eprintln!("(skipping {} — does not exist)", d.display());
        }
    }
    if !watched_any {
        anyhow::bail!("no watchable directories found");
    }

    println!("press Ctrl-C to stop\n");

    while let Some(res) = rx.recv().await {
        match res {
            Ok(e) => handle_event(&e),
            Err(err) => eprintln!("watch error: {err}"),
        }
    }
    Ok(())
}

fn handle_event(e: &notify::Event) {
    if !matches!(
        e.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return;
    }
    for path in &e.paths {
        if path.is_dir() {
            continue;
        }
        let display = path.display();
        if path
            .file_name()
            .and_then(|f| f.to_str())
            .map(|n| n.ends_with(".actant.json") || n == "policy.json")
            .unwrap_or(false)
        {
            match validate_policy(path) {
                Ok(()) => println!("[policy ok]  {display}"),
                Err(err) => println!("[policy err] {display}: {err}"),
            }
        } else if path
            .components()
            .any(|c| c.as_os_str() == "actant-contracts")
        {
            println!("[contracts]  {display} changed — running codegen-ts");
            run_codegen_ts();
        } else {
            println!("[change]     {display}");
        }
    }
}

fn validate_policy(path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let _: actant_policy::PolicyDoc = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(())
}

fn run_codegen_ts() {
    let out = Command::new("cargo")
        .args(["run", "-p", "actant-contracts", "--", "codegen-ts"])
        .output();
    match out {
        Ok(o) if o.status.success() => println!("[contracts]  codegen-ts ok"),
        Ok(o) => eprintln!(
            "[contracts]  codegen-ts failed:\n{}",
            String::from_utf8_lossy(&o.stderr)
        ),
        Err(e) => eprintln!("[contracts]  codegen-ts not run: {e}"),
    }
}
