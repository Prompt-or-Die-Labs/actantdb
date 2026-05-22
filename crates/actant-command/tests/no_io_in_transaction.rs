//! Spec 19 §14 / ADR-0002 — architectural grep test.
//!
//! `actant-command` is the only crate that owns a `Transaction<'_>` in the
//! synchronous command path. The hot kernel discipline forbids any of:
//! - process spawn (`tokio::process`, `std::process::Command`)
//! - HTTP client invocation (`reqwest::`, `hyper::`)
//!
//! inside that crate's source tree. Anything that needs an external system
//! call must be re-shaped into an effect handled by an async lane worker.
//!
//! This test shells out to system `grep -rn` over `crates/actant-command/src/`
//! and asserts zero matches. The existing
//! `spec_19_verification::no_external_io_inside_transactions` only substring-
//! checks `lib.rs`; this test broadens to the whole subtree and the wider
//! forbidden-pattern list.

use std::path::PathBuf;
use std::process::Command;

const FORBIDDEN_PATTERNS: &[&str] = &[
    "tokio::process",
    "std::process::Command",
    "reqwest::",
    "hyper::",
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn actant_command_src_has_no_external_io_calls() {
    // Skip gracefully if grep is unavailable (don't fail CI on a missing tool).
    if Command::new("grep").arg("--version").output().is_err() {
        eprintln!("grep not on PATH; skipping architectural grep test");
        return;
    }

    let target = workspace_root().join("crates/actant-command/src");
    assert!(
        target.is_dir(),
        "expected {} to exist; workspace layout changed?",
        target.display()
    );

    for needle in FORBIDDEN_PATTERNS {
        let out = Command::new("grep")
            .arg("-rn")
            .arg("--include=*.rs")
            .arg(needle)
            .arg(&target)
            .output()
            .expect("failed to invoke grep");

        // grep exit codes:
        //   0 => match found       (FAIL — forbidden pattern in source)
        //   1 => no match          (PASS)
        //   2 => error              (FAIL — unable to verify)
        match out.status.code() {
            Some(0) => {
                let hits = String::from_utf8_lossy(&out.stdout);
                panic!(
                    "Spec 19 §14 / ADR-0002 violation: pattern `{needle}` found inside \
                     actant-command::src/* — external I/O must be moved to an effect handler.\n\
                     Hits:\n{hits}"
                );
            }
            Some(1) => { /* no match → pass */ }
            // grep returns 2 on error and >=3 on signal — treat any other
            // status as a scanner-level failure.
            _ => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                panic!("grep returned an error scanning for `{needle}`: {stderr}");
            }
        }
    }
}
