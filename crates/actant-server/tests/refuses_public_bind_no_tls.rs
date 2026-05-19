//! The binary refuses to start when bound to a non-loopback address
//! without TLS. Without this guard, a fresh `actantdb-server --bind
//! 0.0.0.0:4555` would expose its plaintext command surface to the network.

use std::process::Command;

fn bin_path() -> std::path::PathBuf {
    // `cargo test` puts the binary next to the test binary under
    // `target/debug` (or `target/debug/deps`); CARGO_BIN_EXE_<name> is the
    // canonical lookup.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_actantdb-server"))
}

#[test]
fn refuses_to_bind_public_address_without_tls() {
    let out = Command::new(bin_path())
        .args(["--bind", "0.0.0.0:0"])
        // Strip the env that might come from the host's `.envrc`.
        .env_remove("ACTANTDB_BIND")
        .env_remove("ACTANTDB_TLS_CERT")
        .env_remove("ACTANTDB_TLS_KEY")
        .env_remove("ACTANTDB_INSECURE_PUBLIC")
        .env_remove("ACTANTDB_DB")
        .env_remove("ACTANTDB_DATABASE_URL")
        .output()
        .expect("spawn actantdb-server");
    assert!(
        !out.status.success(),
        "expected the binary to refuse public bind without TLS, got success"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("refusing to bind non-loopback") && stderr.contains("without TLS"),
        "expected refusal message in stderr; got: {stderr}"
    );
    assert!(
        stderr.contains("--tls-cert") && stderr.contains("--insecure-public"),
        "refusal must mention both escape hatches; got: {stderr}"
    );
}

#[test]
fn accepts_loopback_bind_without_tls() {
    // Sanity: the same binary, bound to localhost, starts fine. We send
    // SIGTERM after a few hundred ms to keep the test from hanging.

    #[cfg(unix)]
    {
        use std::time::Duration;
        let mut child = Command::new(bin_path())
            .args(["--bind", "127.0.0.1:0"])
            .env_remove("ACTANTDB_BIND")
            .env_remove("ACTANTDB_TLS_CERT")
            .env_remove("ACTANTDB_TLS_KEY")
            .env_remove("ACTANTDB_DB")
            .env_remove("ACTANTDB_DATABASE_URL")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn");
        std::thread::sleep(Duration::from_millis(400));
        // Still running? If it exited cleanly that means it parsed args
        // and proceeded to bind; an early bail would show up here.
        match child.try_wait().unwrap() {
            Some(status) => panic!("loopback bind exited prematurely with {status}"),
            None => {
                // Kill and reap.
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    // On non-unix CI we just assert the bin path resolves.
    #[cfg(not(unix))]
    assert!(bin_path().exists());
}
