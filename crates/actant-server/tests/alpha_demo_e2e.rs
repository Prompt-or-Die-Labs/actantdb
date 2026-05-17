//! End-to-end alpha-demo: server + effect queue + shell-worker wired
//! through one process. Verifies the full alpha command sequence from spec
//! 10 §1–11, including a real worker picking up an effect, completing it,
//! and writing the result back through the command engine.

use std::net::SocketAddr;
use std::sync::Arc;

use actant_command::Engine;
use actant_core::*;
use actant_effects::EffectQueue;
use actant_storage::{Storage, StorageConfig};
use actant_worker_protocol::{Handler, HandlerResult};
use async_trait::async_trait;
use serde_json::json;

/// A stub shell handler — runs `echo` and reports stdout. Avoids spawning
/// real processes so the test is portable.
#[derive(Debug, Default)]
struct StubShellHandler;

#[async_trait]
impl Handler for StubShellHandler {
    fn effect_type(&self) -> &'static str {
        "shell.run"
    }
    async fn handle(&self, input: serde_json::Value) -> HandlerResult {
        let cmd = input
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(json!({"exit": 0, "stdout": format!("ran: {cmd}")}))
    }
}

#[tokio::test]
async fn alpha_demo_end_to_end_with_worker() {
    // 1. Boot the server (same code path as the bin).
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    // Seed the default workspace + system actor manually (bootstrap does this
    // when it owns the storage; we own ours here).
    let ws = Workspace {
        id: WorkspaceId::from_string("ws_default".to_string()),
        name: "default".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    storage
        .insert_actor(&Actor {
            id: ActorId::from_string("act_system".to_string()),
            workspace_id: ws.id.clone(),
            kind: ActorKind::System,
            display_name: "system".into(),
            created_at: now_rfc3339(),
            disabled_at: None,
        })
        .await
        .unwrap();

    let state = actant_server::AppState::new(storage.clone());
    let router = actant_server::router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let bound = listener.local_addr().unwrap();
    let base = format!("http://{bound}");
    let _server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let c = reqwest::Client::new();

    // 2. Walk the alpha command sequence via HTTP.
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
    assert!(r.status().is_success());
    let session_id = r.json::<serde_json::Value>().await.unwrap()["result"]["session_id"]
        .as_str()
        .unwrap()
        .to_string();

    c.post(format!("{base}/v1/command"))
        .json(&json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": "append_user_message",
            "input": {"session_id": session_id, "text": "Fix failing tests."}
        }))
        .send()
        .await
        .unwrap();

    let r = c
        .post(format!("{base}/v1/command"))
        .json(&json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": "request_tool_call",
            "input": {
                "session_id": session_id,
                "tool_name": "shell.run",
                "arguments": {"command": "pytest -q"}
            }
        }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["result"]["status"], "pending_approval");
    let tool_call_id = body["result"]["tool_call_id"].as_str().unwrap().to_string();

    // 3. Approve through the HTTP command, then drive an effect through a
    //    real worker. The wedge's command engine doesn't auto-enqueue an
    //    effect for tool calls in v0.1 — we enqueue manually here to
    //    exercise the worker path.
    c.post(format!("{base}/v1/command"))
        .json(&json!({
            "workspace_id": "ws_default",
            "actor_id": "act_system",
            "command_type": "approve_tool_call",
            "input": {"tool_call_id": tool_call_id, "scope": "once"}
        }))
        .send()
        .await
        .unwrap();

    // Enqueue an effect manually so we can exercise the worker.
    let cmd = CommandRecord {
        id: CommandId::new(),
        workspace_id: ws.id.clone(),
        actor_id: ActorId::from_string("act_system".to_string()),
        session_id: Some(SessionId::from_string(session_id.clone())),
        command_type: "test".into(),
        input_inline: None,
        input_hash: "h".into(),
        policy_id: None,
        status: CommandStatus::Committed,
        error: None,
        created_at: now_rfc3339(),
        committed_at: None,
    };
    storage.insert_command(&cmd).await.unwrap();
    let queue = EffectQueue::new(storage.clone());
    let eff_id = queue
        .enqueue(
            &ws.id,
            &cmd.id,
            &ActorId::from_string("act_system".to_string()),
            "shell.run",
            json!({"command": "pytest -q"}),
            RiskLevel::Medium,
        )
        .await
        .unwrap();

    // 4. Worker drains the queue.
    let worker = Worker {
        id: WorkerId::new(),
        workspace_id: ws.id.clone(),
        actor_id: ActorId::from_string("act_system".to_string()),
        name: "test-shell-worker".into(),
        host: None,
        version: None,
        status: "online".into(),
        last_heartbeat_at: None,
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    queue
        .register_worker(&worker, &["shell.run"])
        .await
        .unwrap();
    let lease = queue
        .claim_one(&worker.id, &ws.id, &["shell.run"])
        .await
        .unwrap()
        .expect("expected a lease");
    queue.start(&lease.effect_id).await.unwrap();
    let handler = Arc::new(StubShellHandler);
    let input: serde_json::Value = lease
        .input_inline
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(json!({}));
    let output = handler.handle(input).await.unwrap();
    queue.complete(&lease.effect_id, &output).await.unwrap();

    // 5. Record the tool result back through the command engine so the
    //    chronicle gets a tool_call_finished event.
    let engine = Engine::new(storage.clone());
    engine
        .dispatch(
            &ws.id,
            &ActorId::from_string("act_system".to_string()),
            "record_tool_result",
            json!({"tool_call_id": tool_call_id, "result": output}),
            None,
        )
        .await
        .unwrap();

    // 6. Verify the Chronicle has every causal step.
    let r = c
        .get(format!("{base}/v1/events?session_id={session_id}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    let kinds: Vec<&str> = body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"session_created"));
    assert!(kinds.contains(&"user_message_received"));
    assert!(kinds.contains(&"tool_call_requested"));
    assert!(kinds.contains(&"tool_call_approved"));
    assert!(kinds.contains(&"tool_call_finished"));

    // 7. Effect row should be succeeded.
    use sqlx::Row;
    let row = sqlx::query("SELECT status FROM effect WHERE id = ?")
        .bind(eff_id.as_str())
        .fetch_one(storage.pool())
        .await
        .unwrap();
    let status: String = row.get("status");
    assert_eq!(status, "succeeded");
}
