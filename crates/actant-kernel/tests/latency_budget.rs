//! Spec 19 §2 — kernel dispatch latency budget gate.
//!
//! Runs `dispatch_tool_call` 1000 times against a freshly-opened in-memory
//! storage, collects per-iteration wall-clock timings, and asserts that the
//! p99 stays within a CI-margined budget derived from
//! `/specs/19-performance-architecture.md` §2.
//!
//! Spec 19 §2 says `request_tool_call` should hit **p50 < 5 ms / p99 < 30 ms**
//! on a developer laptop. `actant-kernel::dispatch_tool_call` is a thin
//! wrapper that issues exactly that command through the engine, so the budget
//! row applies directly.
//!
//! The spec numbers are for an **optimised release build** on a developer
//! laptop. `cargo test` runs in debug mode, where sqlx, tokio, and the policy
//! compiler are all unoptimised; a 5–10x tail-latency penalty over release
//! steady state is normal. We therefore use two budgets:
//! - **p50** stays tight (≤ 15 ms = 3x spec p50). p50 is dominated by the hot
//!   path itself, which suffers far less from `opt-level = 0`.
//! - **p99** absorbs the debug-build tail (≤ 300 ms = 10x spec p99). The job
//!   of this gate is to catch *regressions* — a 10x cushion still trips
//!   loudly if the kernel grows a sync I/O hop or a busy-wait.
//!
//! Warmup: the first dispatch on a fresh storage compiles policy, primes the
//! SQLite connection cache, and pays one-shot allocations. We discard the
//! first 100 iterations before recording timings; without this the tail would
//! be dominated by cold-start noise rather than the steady-state kernel path.

use std::time::{Duration, Instant};

use actant_command::Engine;
use actant_core::{
    now_rfc3339, Actor, ActorId, ActorKind, SessionId, Workspace, WorkspaceId,
};
use actant_kernel::{dispatch_tool_call, HotToolCall};
use actant_storage::{Storage, StorageConfig};

const TOTAL_ITERS: usize = 1000;
const WARMUP_ITERS: usize = 100;
/// Spec 19 §2 p50 for `request_tool_call` is 5 ms; debug-build margin = 3x.
const P50_BUDGET_MS: u128 = 15;
/// Spec 19 §2 p99 for `request_tool_call` is 30 ms; debug-build margin = 10x.
const P99_BUDGET_MS: u128 = 300;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dispatch_tool_call_p99_within_budget() {
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "bench".into(),
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

    let mk_req = || HotToolCall {
        workspace_id: ws.id.clone(),
        actor_id: actor.id.clone(),
        session_id: sid.clone(),
        tool: "file.read".into(),
        arguments: serde_json::json!({"path": "README.md"}),
    };

    // Warmup: discard.
    for _ in 0..WARMUP_ITERS {
        dispatch_tool_call(&engine, mk_req()).await.unwrap();
    }

    // Measure.
    let mut samples: Vec<Duration> = Vec::with_capacity(TOTAL_ITERS);
    for _ in 0..TOTAL_ITERS {
        let t0 = Instant::now();
        dispatch_tool_call(&engine, mk_req()).await.unwrap();
        samples.push(t0.elapsed());
    }

    samples.sort();
    let p50 = samples[samples.len() / 2];
    let p99_idx = ((samples.len() as f64) * 0.99) as usize;
    let p99 = samples[p99_idx.min(samples.len() - 1)];

    println!(
        "dispatch_tool_call p50={:?} (budget {} ms) p99={:?} (budget {} ms)",
        p50, P50_BUDGET_MS, p99, P99_BUDGET_MS
    );

    assert!(
        p50.as_millis() <= P50_BUDGET_MS,
        "kernel dispatch p50 = {:?} exceeded debug-build budget of {} ms \
         (spec p50 = 5 ms × 3 = 15 ms). A p50 regression usually means a new \
         sync hop on the hot path.",
        p50,
        P50_BUDGET_MS,
    );

    assert!(
        p99.as_millis() <= P99_BUDGET_MS,
        "kernel dispatch p99 = {:?} exceeded debug-build budget of {} ms \
         (spec p99 = 30 ms × 10 = 300 ms). Samples: p50={:?}",
        p99,
        P99_BUDGET_MS,
        p50,
    );
}
