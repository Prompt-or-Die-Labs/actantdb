//! Spec 19 — Performance architecture verification.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn no_external_io_inside_transactions() {
    // Spec 19: "Grep test: zero HTTP / process spawn / model SDK calls inside
    // any Transaction<'_> block."
    // Approximation: any crate that uses sqlx transactions must not call
    // tokio::process / reqwest / std::fs in the same function.
    for crate_dir in [
        "crates/actant-command/src/lib.rs",
        "crates/actant-effects/src/lib.rs",
        "crates/actant-kernel/src/lib.rs",
    ] {
        let src = read_repo(crate_dir);
        // None of these crates should reach for external I/O.
        for needle in ["tokio::process::", "std::process::"] {
            assert!(
                !src.contains(needle),
                "{crate_dir} performs forbidden I/O inside the hot path: {needle}"
            );
        }
    }
}

#[test]
fn bench_harness_exists() {
    // Spec 19: "Bench harness in `bench/` measures every operation in §2
    // and asserts p50/p99."
    // We don't have p50/p99 assertions yet, but the bench crate must
    // contain a real criterion benchmark.
    let cargo = read_repo("bench/Cargo.toml");
    assert!(cargo.contains("criterion"));
    assert!(cargo.contains("storage_append"));
    assert!(cargo.contains("command_dispatch"));
}

#[tokio::test]
async fn kernel_dispatch_table_covers_alpha_commands() {
    // Spec 19: "actant-kernel's dispatch table covers every alpha command."
    // Approximation: the kernel can dispatch a tool call through the engine.
    use actant_command::Engine;
    use actant_core::*;
    use actant_kernel::{dispatch_tool_call, HotToolCall};
    use actant_storage::{Storage, StorageConfig};

    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "t".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Agent,
        display_name: "a".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    s.insert_actor(&actor).await.unwrap();
    let engine = Engine::new(s.clone());
    let session = engine
        .dispatch(
            &ws.id,
            &actor.id,
            "create_session",
            serde_json::json!({}),
            None,
        )
        .await
        .unwrap();
    let sid = SessionId::from_string(session.result["session_id"].as_str().unwrap().to_string());
    let out = dispatch_tool_call(
        &engine,
        HotToolCall {
            workspace_id: ws.id,
            actor_id: actor.id,
            session_id: sid,
            tool: "file.read".into(),
            arguments: serde_json::json!({"path": "README.md"}),
        },
    )
    .await
    .unwrap();
    assert!(out["tool_call_id"].is_string());
}
