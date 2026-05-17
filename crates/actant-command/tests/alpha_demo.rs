//! Exercises the alpha command set from /specs/10-alpha-demo.md.

use actant_command::Engine;
use actant_core::*;
use actant_storage::{Storage, StorageConfig};

async fn fresh_engine() -> (Engine, WorkspaceId, ActorId) {
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let engine = Engine::new(s.clone());
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "alpha".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Agent,
        display_name: "agent_coder".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    s.insert_actor(&actor).await.unwrap();
    (engine, ws.id, actor.id)
}

#[tokio::test]
async fn create_session_and_append_message() {
    let (engine, ws, actor) = fresh_engine().await;
    let out = engine
        .dispatch(&ws, &actor, "create_session", serde_json::json!({}), None)
        .await
        .unwrap();
    let session_id = out.result["session_id"].as_str().unwrap().to_string();
    let _ = engine
        .dispatch(
            &ws,
            &actor,
            "append_user_message",
            serde_json::json!({"session_id": session_id, "text": "Fix tests."}),
            None,
        )
        .await
        .unwrap();
    let events = engine
        .storage()
        .events_in_session(&SessionId::from_string(session_id))
        .await
        .unwrap();
    let kinds: Vec<&str> = events.iter().map(|e| e.event_type.as_str()).collect();
    assert!(kinds.contains(&"session_created"));
    assert!(kinds.contains(&"user_message_received"));
}

#[tokio::test]
async fn tool_call_path_with_approval() {
    let (engine, ws, actor) = fresh_engine().await;
    let out = engine
        .dispatch(&ws, &actor, "create_session", serde_json::json!({}), None)
        .await
        .unwrap();
    let session_id = out.result["session_id"].as_str().unwrap().to_string();

    // shell.run -> require_approval (no constrain hint for `ls`)
    let req = engine
        .dispatch(
            &ws,
            &actor,
            "request_tool_call",
            serde_json::json!({
                "session_id": session_id,
                "tool_name": "shell.run",
                "arguments": {"command": "ls"}
            }),
            None,
        )
        .await
        .unwrap();
    let tc_id = req.result["tool_call_id"].as_str().unwrap().to_string();
    assert_eq!(req.result["status"], "pending_approval");

    // approve
    engine
        .dispatch(
            &ws,
            &actor,
            "approve_tool_call",
            serde_json::json!({"tool_call_id": tc_id, "scope": "once"}),
            None,
        )
        .await
        .unwrap();

    // record result
    engine
        .dispatch(
            &ws,
            &actor,
            "record_tool_result",
            serde_json::json!({"tool_call_id": tc_id, "result": {"stdout":"file1"}}),
            None,
        )
        .await
        .unwrap();

    let events = engine
        .storage()
        .events_in_session(&SessionId::from_string(session_id))
        .await
        .unwrap();
    let kinds: Vec<&str> = events.iter().map(|e| e.event_type.as_str()).collect();
    assert!(kinds.contains(&"tool_call_requested"));
    assert!(kinds.contains(&"tool_call_approved"));
    assert!(kinds.contains(&"tool_call_finished"));
}

#[tokio::test]
async fn memory_proposal_and_approval() {
    let (engine, ws, actor) = fresh_engine().await;
    let out = engine
        .dispatch(
            &ws,
            &actor,
            "propose_memory",
            serde_json::json!({"text":"Project uses pytest.","category":"fact","confidence":0.9}),
            None,
        )
        .await
        .unwrap();
    let mc_id = out.result["memory_candidate_id"]
        .as_str()
        .unwrap()
        .to_string();
    let out2 = engine
        .dispatch(
            &ws,
            &actor,
            "approve_memory",
            serde_json::json!({"memory_candidate_id": mc_id}),
            None,
        )
        .await
        .unwrap();
    assert!(out2.result["memory_id"].is_string());
}

#[tokio::test]
async fn idempotency_replay() {
    let (engine, ws, actor) = fresh_engine().await;
    let out1 = engine
        .dispatch(
            &ws,
            &actor,
            "create_session",
            serde_json::json!({}),
            Some("k1"),
        )
        .await
        .unwrap();
    let out2 = engine
        .dispatch(
            &ws,
            &actor,
            "create_session",
            serde_json::json!({}),
            Some("k1"),
        )
        .await
        .unwrap();
    assert_eq!(out1.command_id.as_str(), out2.command_id.as_str());
    assert_eq!(out2.result["idempotent_replay"], true);
}
