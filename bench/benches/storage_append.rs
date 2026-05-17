//! Benchmark: ledger event append latency on an in-memory SQLite store.

use actant_bench::fresh;
use actant_core::*;
use criterion::{criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;

fn bench_append(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (engine, ws, actor) = rt.block_on(fresh());
    let session = {
        let r = rt
            .block_on(engine.dispatch(&ws, &actor, "create_session", serde_json::json!({}), None))
            .unwrap();
        SessionId::from_string(r.result["session_id"].as_str().unwrap().to_string())
    };
    let storage = engine.storage().clone();

    c.bench_function("storage_append_event", |b| {
        b.iter(|| {
            rt.block_on(async {
                let payload = serde_json::json!({"x": 1});
                let canon = canonical_json(&payload);
                let hash = sha256_hex(canon.as_bytes());
                let prev = storage
                    .last_event_hash(&ws, Some(&session))
                    .await
                    .unwrap()
                    .unwrap_or_else(|| "0".repeat(64));
                let chain = chain_hash(&prev, &hash);
                let e = AgentEvent {
                    id: EventId::new(),
                    workspace_id: ws.clone(),
                    actor_id: actor.clone(),
                    session_id: Some(session.clone()),
                    parent_event_id: None,
                    event_type: "bench_event".into(),
                    causality_kind: CausalityKind::Audit,
                    sensitivity: Sensitivity::Low,
                    authority_scope_id: None,
                    payload_ref: None,
                    payload_inline: Some(canon),
                    payload_hash: hash,
                    event_hash: chain,
                    created_at: now_rfc3339(),
                    model_call_id: None,
                    tool_call_id: None,
                    workflow_run_id: None,
                    memory_id: None,
                    artifact_id: None,
                    command_id: None,
                    effect_id: None,
                };
                storage.append_event(&e).await.unwrap();
            });
        });
    });
}

criterion_group!(benches, bench_append);
criterion_main!(benches);
