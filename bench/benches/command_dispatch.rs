//! Benchmark: command engine throughput — append_user_message in a hot loop.

use actant_bench::fresh;
use criterion::{criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;

fn bench_append_user_message(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (engine, ws, actor) = rt.block_on(fresh());
    let session_id = rt
        .block_on(engine.dispatch(&ws, &actor, "create_session", serde_json::json!({}), None))
        .unwrap()
        .result["session_id"]
        .as_str()
        .unwrap()
        .to_string();

    c.bench_function("command_append_user_message", |b| {
        let session = session_id.clone();
        b.iter(|| {
            rt.block_on(async {
                engine
                    .dispatch(
                        &ws,
                        &actor,
                        "append_user_message",
                        serde_json::json!({"session_id": session, "text": "hello"}),
                        None,
                    )
                    .await
                    .unwrap();
            });
        });
    });
}

criterion_group!(benches, bench_append_user_message);
criterion_main!(benches);
