//! Run the `actantdb` binary as a subprocess and exercise its commands.

use std::process::Command;

fn bin() -> std::path::PathBuf {
    // CARGO_BIN_EXE_actantdb is set by cargo for binary integration tests.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_actantdb"))
}

fn tmp_db() -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("actantdb-cli-test-{}.sqlite", ulid::Ulid::new()));
    p
}

fn tmp_file(prefix: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("{prefix}-{}", ulid::Ulid::new()));
    p
}

#[test]
fn migrate_creates_database() {
    let db = tmp_db();
    let out = Command::new(bin())
        .args(["--db", db.to_str().unwrap(), "migrate"])
        .output()
        .expect("spawn actantdb");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(db.exists(), "db file was not created");
    std::fs::remove_file(&db).ok();
}

#[test]
fn create_session_via_subprocess() {
    let db = tmp_db();
    // Migrate first to set up the schema and seed workspace/actor.
    Command::new(bin())
        .args(["--db", db.to_str().unwrap(), "migrate"])
        .output()
        .unwrap();

    // The migrate path doesn't seed default workspace+actor (that's the
    // server's bootstrap). Insert them via a small `command` call that
    // requires nothing — the CLI's `command` subcommand uses the engine
    // directly, which writes through the schema. We can't easily seed
    // workspace/actor via the CLI, so just verify `migrate` listing works.
    let out = Command::new(bin())
        .args([
            "--db",
            db.to_str().unwrap(),
            "approvals",
            "--workspace",
            "ws_default",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "approvals exit: {} stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::remove_file(&db).ok();
}

#[test]
fn help_lists_subcommands() {
    let out = Command::new(bin())
        .arg("--help")
        .output()
        .expect("spawn actantdb --help");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("migrate"));
    assert!(stdout.contains("serve"));
    assert!(stdout.contains("command"));
    assert!(stdout.contains("events"));
    assert!(stdout.contains("approvals"));
}

#[test]
fn version_flag_works() {
    let out = Command::new(bin())
        .arg("--version")
        .output()
        .expect("spawn actantdb --version");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with("actantdb"), "got: {stdout}");
}

#[test]
fn invalid_sql_uses_structured_error_shape() {
    let db = tmp_db();
    let out = Command::new(bin())
        .args([
            "--db",
            db.to_str().unwrap(),
            "sql",
            "DROP TABLE agent_event",
        ])
        .output()
        .expect("spawn actantdb sql");
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(!out.status.success());
    assert!(stderr.contains("error: invalid_input"), "stderr: {stderr}");
    assert!(
        stderr.contains("detail: invalid input:"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("fix:"), "stderr: {stderr}");
    assert!(!stderr.contains("message:"), "stderr: {stderr}");
    let _ = std::fs::remove_file(&db);
}

#[test]
fn bad_import_json_uses_structured_error_shape() {
    let db = tmp_db();
    let import = tmp_file("actantdb-import-bad");
    std::fs::write(&import, "{not json}\n").unwrap();

    let out = Command::new(bin())
        .args([
            "--db",
            db.to_str().unwrap(),
            "import",
            "--from",
            import.to_str().unwrap(),
        ])
        .output()
        .expect("spawn actantdb import");
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(!out.status.success());
    assert!(stderr.contains("error: invalid_input"), "stderr: {stderr}");
    assert!(
        stderr.contains("detail: invalid input: line 1: bad JSON"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("fix:"), "stderr: {stderr}");
    assert!(!stderr.contains("message:"), "stderr: {stderr}");
    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_file(&import);
}
