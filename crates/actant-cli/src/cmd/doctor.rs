//! `actantdb doctor` — diagnose the local dev environment.

use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
enum Mark {
    Ok,
    Warn,
    Fail,
}

fn glyph(m: &Mark) -> &'static str {
    match m {
        Mark::Ok => "[ok]",
        Mark::Warn => "[warn]",
        Mark::Fail => "[fail]",
    }
}

fn report(m: Mark, label: &str, detail: &str, fix: Option<&str>) {
    println!("{:<6} {:<32}  {}", glyph(&m), label, detail);
    if let Some(f) = fix {
        println!("       fix:   {f}");
    }
}

/// Run the doctor checks.
pub fn run(db_path: &Path) -> anyhow::Result<()> {
    println!("ActantDB environment check");
    println!("==========================");

    check_rust();
    check_node();
    check_disk(db_path);
    check_port(4555);
    check_port(54323);
    check_optional_cli("claude");
    check_optional_cli("codex");
    check_optional_cli("opencode");
    check_pg_env();
    check_studio_dist();

    println!();
    Ok(())
}

fn check_rust() {
    let out = Command::new("rustc").arg("--version").output();
    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            // Extract semver like "1.88.0" out of "rustc 1.88.0 (xxxx ...)".
            let version = s.split_whitespace().nth(1).unwrap_or_default().to_string();
            if semver_ge(&version, "1.88.0") {
                report(Mark::Ok, "rustc >= 1.88", &s, None);
            } else {
                report(
                    Mark::Warn,
                    "rustc >= 1.88",
                    &s,
                    Some("rustup update stable"),
                );
            }
        }
        _ => report(
            Mark::Warn,
            "rustc >= 1.88",
            "not found",
            Some("install Rust via https://rustup.rs"),
        ),
    }
}

fn check_node() {
    let out = Command::new("node").arg("--version").output();
    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let trimmed = s.trim_start_matches('v').to_string();
            if semver_ge(&trimmed, "22.5.0") {
                report(Mark::Ok, "node >= 22.5", &s, None);
            } else {
                report(
                    Mark::Warn,
                    "node >= 22.5",
                    &s,
                    Some("install via `nvm install 22` or `volta install node@22`"),
                );
            }
        }
        _ => report(
            Mark::Warn,
            "node >= 22.5",
            "not found",
            Some("install Node.js >= 22.5 from https://nodejs.org"),
        ),
    }
}

fn check_disk(db_path: &Path) {
    let parent = db_path.parent().unwrap_or_else(|| Path::new("."));
    let target = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    // Best-effort: `df -k <path>` works on macOS and Linux.
    let out = Command::new("df").arg("-Pk").arg(target).output();
    match out {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            // Header + one data line; the available column is the 4th.
            if let Some(line) = text.lines().nth(1) {
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() >= 4 {
                    let avail_kb: u64 = cols[3].parse().unwrap_or(0);
                    let gb = (avail_kb as f64) / 1024.0 / 1024.0;
                    let mark = if gb >= 5.0 { Mark::Ok } else { Mark::Warn };
                    report(
                        mark,
                        "disk space (db dir)",
                        &format!("{gb:.1} GB free on {}", target.display()),
                        if gb >= 5.0 {
                            None
                        } else {
                            Some("free disk space below 5 GB threshold")
                        },
                    );
                    return;
                }
            }
            report(
                Mark::Warn,
                "disk space (db dir)",
                "could not parse `df`",
                None,
            );
        }
        _ => report(Mark::Warn, "disk space (db dir)", "`df` unavailable", None),
    }
}

fn check_port(port: u16) {
    let in_use = std::net::TcpListener::bind(("127.0.0.1", port)).is_err();
    if in_use {
        let pid = pid_on_port(port);
        let detail = match pid {
            Some(p) => format!("in use by pid {p}"),
            None => "in use (pid unknown)".to_string(),
        };
        report(
            Mark::Warn,
            &format!("port {port} free"),
            &detail,
            Some("kill the process or run on a different port"),
        );
    } else {
        report(Mark::Ok, &format!("port {port} free"), "available", None);
    }
}

fn pid_on_port(port: u16) -> Option<u32> {
    // macOS / Linux: lsof -nP -iTCP:<port> -sTCP:LISTEN -t
    let out = Command::new("lsof")
        .args(["-nP", "-sTCP:LISTEN", "-t", &format!("-iTCP:{port}")])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.lines().next().and_then(|line| line.trim().parse().ok())
}

fn check_optional_cli(name: &str) {
    let found = which(name).is_some();
    if found {
        report(Mark::Ok, &format!("`{name}` on PATH"), "found", None);
    } else {
        report(
            Mark::Warn,
            &format!("`{name}` on PATH"),
            "not found",
            Some("optional; see `@actantdb/box` docs if you need it"),
        );
    }
}

fn check_pg_env() {
    let val = std::env::var("ACTANTDB_DATABASE_URL").ok();
    match val {
        Some(url) if url.starts_with("postgres://") || url.starts_with("postgresql://") => {
            // We don't try to ping Postgres here — just acknowledge the wiring.
            report(
                Mark::Ok,
                "ACTANTDB_DATABASE_URL",
                "set (postgres backend selected)",
                None,
            );
        }
        Some(other) => report(
            Mark::Warn,
            "ACTANTDB_DATABASE_URL",
            &format!("set but not postgres: {other}"),
            Some("unset, or set to a postgres:// URL"),
        ),
        None => report(
            Mark::Ok,
            "ACTANTDB_DATABASE_URL",
            "unset (sqlite backend)",
            None,
        ),
    }
}

fn check_studio_dist() {
    let candidates = [
        PathBuf::from("packages/actant-studio/dist/ui"),
        PathBuf::from("packages/actant-studio/dist"),
    ];
    let found = candidates.iter().find(|p| p.exists()).cloned();
    if let Some(p) = found {
        report(Mark::Ok, "studio dist", &p.display().to_string(), None);
    } else {
        report(
            Mark::Warn,
            "studio dist",
            "not built",
            Some("pnpm --filter @actantdb/studio build"),
        );
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path_env = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_env) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
        // On Windows we'd also try `.exe` etc., but the CLI is unix-first.
    }
    None
}

fn semver_ge(have: &str, want: &str) -> bool {
    fn parts(s: &str) -> [u64; 3] {
        let mut it = s.split('.').filter_map(|p| {
            p.split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|q| q.parse().ok())
        });
        [
            it.next().unwrap_or(0),
            it.next().unwrap_or(0),
            it.next().unwrap_or(0),
        ]
    }
    let h = parts(have);
    let w = parts(want);
    h >= w
}
