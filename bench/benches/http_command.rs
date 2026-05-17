//! End-to-end HTTP throughput: bench POST /v1/command against a live
//! actantdb-server process running in-process.

use std::net::SocketAddr;
use std::time::Duration;

use actant_bench::fresh;
use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use criterion::{criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;

fn bench_http_command(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (engine, ws, actor) = rt.block_on(fresh());
    let _ = (engine, ws, actor);

    // Spin up a full server with a seeded workspace.
    let (base, _handle) = rt.block_on(async {
        let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let ws = Workspace {
            id: WorkspaceId::from_string("ws_default".to_string()),
            name: "bench".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        storage.insert_workspace(&ws).await.unwrap();
        storage
            .insert_actor(&Actor {
                id: ActorId::from_string("act_system".to_string()),
                workspace_id: ws.id.clone(),
                kind: ActorKind::System,
                display_name: "x".into(),
                created_at: now_rfc3339(),
                disabled_at: None,
            })
            .await
            .unwrap();
        let state = actant_server::AppState::new(storage);
        let router = actant_server::router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
            .await
            .unwrap();
        let bound = listener.local_addr().unwrap();
        let h = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (format!("http://{bound}"), h)
    });

    // Pre-create a session so the bench only measures append_user_message.
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let body: serde_json::Value = client
        .post(format!("{base}/v1/command"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": "create_session",
            "input": {}
        }))
        .send()
        .unwrap()
        .json()
        .unwrap();
    let session_id = body["result"]["session_id"].as_str().unwrap().to_string();

    c.bench_function("http_append_user_message", |b| {
        b.iter(|| {
            client
                .post(format!("{base}/v1/command"))
                .json(&serde_json::json!({
                    "workspace_id": "ws_default",
                    "actor_id": "act_system",
                    "command_type": "append_user_message",
                    "input": {"session_id": session_id, "text": "hello"}
                }))
                .send()
                .unwrap();
        });
    });
}

criterion_group!(benches, bench_http_command);
criterion_main!(benches);
