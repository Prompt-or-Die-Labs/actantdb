//! Spin up the server in-process and exercise the HTTP surface.

use std::net::SocketAddr;

use actant_server::bootstrap;
use serde_json::json;

async fn start() -> (String, tokio::task::JoinHandle<()>) {
    let (router, _state) = bootstrap(None).await.expect("bootstrap");
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let h = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (format!("http://{bound}"), h)
}

#[tokio::test]
async fn healthz_and_command_dispatch() {
    let (base, _h) = start().await;
    let c = reqwest::Client::new();

    let r = c.get(format!("{base}/v1/healthz")).send().await.unwrap();
    assert!(r.status().is_success());

    let r = c
        .get(format!("{base}/v1/metadata/commands"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    let commands = body["commands"].as_array().unwrap();
    assert!(commands.iter().any(|v| v == "create_session"));

    let r = c
        .post(format!("{base}/v1/command"))
        .json(&json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": "create_session",
            "input": {}
        }))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "create_session: {}", r.status());
    let body: serde_json::Value = r.json().await.unwrap();
    assert!(body["result"]["session_id"].is_string());
}
