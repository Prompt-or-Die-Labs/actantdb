//! ADR verification harness. Each ADR with structural implications gets
//! one test that asserts the decision is enforced in code.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn adr_0001_commands_are_mutation() {
    // Every state change goes through actant-command::Engine::dispatch.
    let cmd = read_repo("crates/actant-command/src/lib.rs");
    // The engine has exactly one dispatch entry point.
    let dispatch_count = cmd.matches("pub async fn dispatch(").count();
    assert_eq!(
        dispatch_count, 1,
        "expected exactly one dispatch entry point"
    );
}

#[test]
fn adr_0002_effects_outside_transaction() {
    // Effects are scheduled, not executed inside command transactions.
    let cmd = read_repo("crates/actant-command/src/lib.rs");
    for needle in ["tokio::process::", "std::process::"] {
        assert!(
            !cmd.contains(needle),
            "actant-command performs direct execution — ADR 0002 violation: {needle}"
        );
    }
}

#[test]
fn adr_0003_context_as_manifest() {
    // Context items pass through a manifest, not raw concatenation.
    let ctx = read_repo("crates/actant-context/src/lib.rs");
    assert!(ctx.contains("Manifest"), "Manifest type missing");
    assert!(ctx.contains("manifest_hash"));
    assert!(ctx.contains("blocked"));
}

#[test]
fn adr_0005_data_capsules_have_table_and_type() {
    let schema = read_repo("specs/02-data-model.sql");
    assert!(schema.contains("CREATE TABLE capsule"));
    let lib = read_repo("crates/actant-capsule/src/lib.rs");
    assert!(lib.contains("struct Capsule"));
}

#[test]
fn adr_0007_behavioral_trust_has_score_confidence_samples() {
    let lib = read_repo("crates/actant-trust/src/lib.rs");
    assert!(lib.contains("score"));
    assert!(lib.contains("confidence"));
    assert!(lib.contains("sample_size"));
}

#[test]
fn adr_0008_cli_is_first_class() {
    // There's a Rust binary `actantdb` AND a TS CLI `actantdb` in studio.
    let cli = read_repo("crates/actant-cli/src/main.rs");
    assert!(cli.contains("Subcommand"), "CLI must use clap subcommands");
    let ts_cli = read_repo("packages/actant-studio/src/cli.ts");
    assert!(ts_cli.contains("usage()"));
}

#[test]
fn adr_0014_local_first_embedders() {
    // Default embedder runs locally; cloud is opt-in.
    let lib = read_repo("crates/actant-embedders/src/lib.rs");
    assert!(
        lib.contains("HashEmbedder"),
        "default local embedder missing"
    );
}

#[test]
fn adr_0015_otel_genai_columns_present() {
    let schema = read_repo("specs/02-data-model.sql");
    assert!(schema.contains("otel_trace_id"));
    assert!(schema.contains("otel_span_id"));
}

#[test]
fn adr_0016_reliability_primitives_all_present() {
    let mig = read_repo("migrations/0003_ai_native_and_reliability.sql");
    for primitive in [
        "rate_limit_policy",
        "circuit_state",
        "lock",
        "ingress_event",
    ] {
        assert!(
            mig.contains(primitive),
            "ADR 0016 missing primitive {primitive}"
        );
    }
}

#[test]
fn adr_0017_universal_idempotency() {
    let mig = read_repo("migrations/0003_ai_native_and_reliability.sql");
    assert!(mig.contains("idempotency_record"));
    let cmd = read_repo("crates/actant-command/src/lib.rs");
    assert!(cmd.contains("idempotency_lookup"));
    assert!(cmd.contains("idempotency_record"));
}

#[test]
fn adr_0018_hot_kernel_exists() {
    let kernel = read_repo("crates/actant-kernel/src/lib.rs");
    assert!(kernel.contains("dispatch_tool_call"));
    assert!(kernel.contains("HotToolCall"));
}

#[test]
fn adr_0020_deployment_modes_have_helm_chart() {
    let chart = read_repo("deploy/helm/actantdb/Chart.yaml");
    assert!(chart.contains("name: actantdb"));
    let docker = read_repo("deploy/docker/Dockerfile");
    assert!(docker.contains("actantdb-server"));
}
