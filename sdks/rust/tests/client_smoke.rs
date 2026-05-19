//! Round-trip every HTTP endpoint against a `wiremock` stub. Asserts the
//! client correctly serializes paths, query strings, and request bodies, and
//! decodes each documented response shape.

use actantdb_client::{ActantClient, ReplayMode, Sensitivity};
use serde_json::json;
use url::Url;
use wiremock::matchers::{body_json, body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn fresh_client() -> (ActantClient, MockServer) {
    let server = MockServer::start().await;
    let url = Url::parse(&server.uri()).expect("mock server uri parses");
    let client = ActantClient::new(url)
        .with_token("dev-token")
        .with_workspace_id("ws_demo")
        .with_actor_id("act_user");
    (client, server)
}

#[tokio::test]
async fn healthz_round_trip() {
    let (client, server) = fresh_client().await;
    Mock::given(method("GET"))
        .and(path("/v1/healthz"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"status": "ok", "time": "2026-05-19T00:00:00Z"})),
        )
        .mount(&server)
        .await;
    let h = client.healthz().await.expect("healthz");
    assert_eq!(h.status.as_deref(), Some("ok"));
    assert!(h.is_healthy());
}

#[tokio::test]
async fn healthz_phase_probes() {
    let (client, server) = fresh_client().await;
    for phase in &["startup", "live", "ready"] {
        Mock::given(method("GET"))
            .and(path(format!("/v1/healthz/{phase}")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"phase": phase, "ok": true})),
            )
            .mount(&server)
            .await;
    }
    let s = client.healthz_startup().await.expect("startup");
    assert_eq!(s.phase.as_deref(), Some("startup"));
    let l = client.healthz_live().await.expect("live");
    assert!(l.is_healthy());
    let r = client.healthz_ready().await.expect("ready");
    assert!(r.is_healthy());
}

#[tokio::test]
async fn metadata_commands_projects_names() {
    let (client, server) = fresh_client().await;
    Mock::given(method("GET"))
        .and(path("/v1/metadata/commands"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "commands": [
                {"name": "create_session"},
                {"name": "append_user_message"},
                {"name": "approve_tool_call"},
            ]
        })))
        .mount(&server)
        .await;
    let names = client.metadata_commands().await.expect("metadata");
    assert_eq!(
        names,
        vec!["create_session", "append_user_message", "approve_tool_call"]
    );
}

#[tokio::test]
async fn create_session_returns_id() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/command"))
        .and(body_json(json!({
            "workspace_id": "ws_demo",
            "actor_id": "act_user",
            "command_type": "create_session",
            "input": {"title": "fix tests"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "command_id": "cmd_1",
            "event_id": "evt_1",
            "result": {"session_id": "sess_42"}
        })))
        .mount(&server)
        .await;
    let sid = client
        .create_session(None, None, Some("fix tests"))
        .await
        .expect("create_session");
    assert_eq!(sid, "sess_42");
}

#[tokio::test]
async fn append_user_and_agent_messages() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/command"))
        .and(body_string_contains("append_user_message"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"command_id": "u1", "event_id": "e1", "result": {}})),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/command"))
        .and(body_string_contains("append_agent_message"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"command_id": "a1", "event_id": "e2", "result": {}})),
        )
        .mount(&server)
        .await;

    let u = client
        .append_user_message(None, None, "sess_1", "hello")
        .await
        .unwrap();
    assert_eq!(u.command_id, "u1");
    let a = client
        .append_agent_message(None, None, "sess_1", "hi back")
        .await
        .unwrap();
    assert_eq!(a.command_id, "a1");
}

#[tokio::test]
async fn tool_call_lifecycle() {
    let (client, server) = fresh_client().await;
    for ct in [
        "request_tool_call",
        "approve_tool_call",
        "deny_tool_call",
        "record_tool_result",
    ] {
        Mock::given(method("POST"))
            .and(path("/v1/command"))
            .and(body_string_contains(ct))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"command_id": ct, "result": {"command": ct}})),
            )
            .mount(&server)
            .await;
    }
    let r = client
        .request_tool_call(None, None, "sess_1", "shell.run", json!({"cmd": "ls"}))
        .await
        .unwrap();
    assert_eq!(r.command_id, "request_tool_call");
    let a = client
        .approve_tool_call(None, None, "tc_1", "once")
        .await
        .unwrap();
    assert_eq!(a.command_id, "approve_tool_call");
    let d = client
        .deny_tool_call(None, None, "tc_2", "too risky")
        .await
        .unwrap();
    assert_eq!(d.command_id, "deny_tool_call");
    let res = client
        .record_tool_result(None, None, "tc_1", json!({"stdout": ""}))
        .await
        .unwrap();
    assert_eq!(res.command_id, "record_tool_result");
}

#[tokio::test]
async fn memory_lifecycle() {
    let (client, server) = fresh_client().await;
    for ct in ["propose_memory", "approve_memory", "reject_memory"] {
        Mock::given(method("POST"))
            .and(path("/v1/command"))
            .and(body_string_contains(ct))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"command_id": ct, "result": {"command": ct}})),
            )
            .mount(&server)
            .await;
    }
    let p = client
        .propose_memory(
            None,
            None,
            "user prefers dark mode",
            "preference",
            Sensitivity::Low,
            0.9,
            json!({"source": "chat"}),
        )
        .await
        .unwrap();
    assert_eq!(p.command_id, "propose_memory");
    let a = client
        .approve_memory(None, None, "cand_1")
        .await
        .unwrap();
    assert_eq!(a.command_id, "approve_memory");
    let r = client
        .reject_memory(None, None, "cand_2", Some("duplicate"))
        .await
        .unwrap();
    assert_eq!(r.command_id, "reject_memory");
}

#[tokio::test]
async fn events_endpoint() {
    let (client, server) = fresh_client().await;
    Mock::given(method("GET"))
        .and(path("/v1/events"))
        .and(query_param("session_id", "sess_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "events": [{
                "id": "evt_1",
                "workspace_id": "ws_demo",
                "actor_id": "act_user",
                "session_id": "sess_1",
                "event_type": "agent.run.started",
                "causality_kind": "command",
                "sensitivity": "low",
                "payload_hash": "deadbeef",
                "event_hash": "0000",
                "created_at": "2026-05-19T00:00:00Z"
            }]
        })))
        .mount(&server)
        .await;
    let events = client.events("sess_1").await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, "evt_1");
    assert_eq!(events[0].causality_kind, "command");
}

#[tokio::test]
async fn approvals_endpoint() {
    let (client, server) = fresh_client().await;
    Mock::given(method("GET"))
        .and(path("/v1/approvals"))
        .and(query_param("workspace_id", "ws_demo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "approvals": [{
                "id": "appr_1",
                "tool_call_id": "tc_1",
                "requested_by": "act_user",
                "risk_level": "high",
                "summary": "shell.run with rm",
                "status": "pending"
            }]
        })))
        .mount(&server)
        .await;
    let pending = client.approvals(None).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "appr_1");
    assert_eq!(pending[0].risk_level, "high");
}

#[tokio::test]
async fn replay_checkpoint_and_run() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/replay/checkpoint"))
        .and(body_json(json!({
            "workspace_id": "ws_demo",
            "event_id": "evt_abc"
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"checkpoint_id": "cp_1"})),
        )
        .mount(&server)
        .await;
    let cp_id = client.replay_checkpoint(None, "evt_abc").await.unwrap();
    assert_eq!(cp_id, "cp_1");

    Mock::given(method("POST"))
        .and(path("/v1/replay/run"))
        .and(body_json(json!({
            "actor_id": "act_user",
            "checkpoint_id": "cp_1",
            "mode": "memory",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "a": "run_orig",
            "b": "run_replay",
            "entries": []
        })))
        .mount(&server)
        .await;
    let diff = client
        .replay_run(None, "cp_1", ReplayMode::Memory)
        .await
        .unwrap();
    assert_eq!(diff.a, "run_orig");
    assert_eq!(diff.b, "run_replay");
}

#[tokio::test]
async fn sync_since_round_trip() {
    let (client, server) = fresh_client().await;
    Mock::given(method("POST"))
        .and(path("/v1/sync/since"))
        .and(body_json(json!({
            "workspace_id": "ws_demo",
            "since_event_id": "",
            "limit": 100u32
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "events": [{
                "id": "evt_1",
                "event_type": "agent.run.started",
                "actor_id": "act_user",
                "payload_hash": "h",
                "payload_inline": "{}",
                "created_at": "2026-05-19T00:00:00Z"
            }],
            "next_since": "evt_1"
        })))
        .mount(&server)
        .await;
    let r = client.sync_since(None, "", 100).await.unwrap();
    assert_eq!(r.events.len(), 1);
    assert_eq!(r.next_since.as_deref(), Some("evt_1"));
}

#[tokio::test]
async fn memories_endpoint() {
    let (client, server) = fresh_client().await;
    Mock::given(method("GET"))
        .and(path("/v1/memories"))
        .and(query_param("workspace_id", "ws_demo"))
        .and(query_param("status", "approved"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "memories": [{"id": "mem_1", "text": "user prefers dark mode"}]
        })))
        .mount(&server)
        .await;
    let mems = client.memories(None, "approved").await.unwrap();
    assert_eq!(mems.len(), 1);
}

#[tokio::test]
async fn bearer_token_attached() {
    let (client, server) = fresh_client().await;
    Mock::given(method("GET"))
        .and(path("/v1/healthz"))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer dev-token",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "ok"})))
        .mount(&server)
        .await;
    let h = client.healthz().await.expect("authed health probe");
    assert_eq!(h.status.as_deref(), Some("ok"));
}

#[tokio::test]
async fn missing_default_workspace_fails_loud() {
    let server = MockServer::start().await;
    let client = ActantClient::new(Url::parse(&server.uri()).unwrap());
    // No default workspace, no override → InvalidInput before any HTTP call.
    let err = client
        .approvals(None)
        .await
        .expect_err("must reject without workspace_id");
    match err {
        actantdb_client::ActantError::InvalidInput { message, .. } => {
            assert!(message.contains("workspace_id"));
        }
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}

#[tokio::test]
async fn openapi_returns_raw_text() {
    let (client, server) = fresh_client().await;
    Mock::given(method("GET"))
        .and(path("/v1/openapi.yaml"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw("openapi: 3.1.0\n", "application/yaml"),
        )
        .mount(&server)
        .await;
    let raw = client.openapi().await.unwrap();
    assert!(raw.starts_with("openapi:"));
}
