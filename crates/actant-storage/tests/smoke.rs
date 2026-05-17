//! Storage smoke tests.
//!
//! - in-memory DB opens
//! - schema migration applies
//! - basic insert/select round-trip works

use actant_core::*;
use actant_storage::{Storage, StorageConfig};

#[tokio::test]
async fn opens_and_applies_schema() {
    let s = Storage::open(StorageConfig::in_memory())
        .await
        .expect("open");
    let applied = s.applied_migrations().await.expect("applied");
    assert!(applied.contains(&"0001_initial".to_string()));
    assert!(applied.contains(&"0002_extended_primitives".to_string()));
    assert!(applied.contains(&"0003_ai_native_and_reliability".to_string()));
}

#[tokio::test]
async fn workspace_actor_round_trip() {
    let s = Storage::open(StorageConfig::in_memory())
        .await
        .expect("open");
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "demo".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.expect("insert");
    let got = s.get_workspace(&ws.id).await.expect("get").expect("found");
    assert_eq!(got.name, "demo");

    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Agent,
        display_name: "agent_coder".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    s.insert_actor(&actor).await.expect("insert");
    let got = s.get_actor(&actor.id).await.expect("get").expect("found");
    assert_eq!(got.display_name, "agent_coder");
    assert_eq!(got.kind, ActorKind::Agent);
}

#[tokio::test]
async fn chronicle_chain_links() {
    let s = Storage::open(StorageConfig::in_memory())
        .await
        .expect("open");
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "demo".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Human,
        display_name: "wes".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    s.insert_actor(&actor).await.unwrap();
    let session = Session {
        id: SessionId::new(),
        workspace_id: ws.id.clone(),
        title: None,
        initiator_actor_id: actor.id.clone(),
        agent_actor_id: None,
        status: SessionStatus::Active,
        created_at: now_rfc3339(),
        closed_at: None,
    };
    s.insert_session(&session).await.unwrap();

    // First event — chain from genesis.
    let payload = serde_json::json!({"text":"hello"});
    let payload_canon = canonical_json(&payload);
    let payload_hash = sha256_hex(payload_canon.as_bytes());
    let chain_a = chain_hash(&"0".repeat(64), &payload_hash);
    let e1 = AgentEvent {
        id: EventId::new(),
        workspace_id: ws.id.clone(),
        actor_id: actor.id.clone(),
        session_id: Some(session.id.clone()),
        parent_event_id: None,
        event_type: "user_message".into(),
        causality_kind: CausalityKind::Observation,
        sensitivity: Sensitivity::Low,
        authority_scope_id: None,
        payload_ref: None,
        payload_inline: Some(payload_canon),
        payload_hash: payload_hash.clone(),
        event_hash: chain_a.clone(),
        created_at: now_rfc3339(),
        model_call_id: None,
        tool_call_id: None,
        workflow_run_id: None,
        memory_id: None,
        artifact_id: None,
        command_id: None,
        effect_id: None,
    };
    s.append_event(&e1).await.unwrap();

    let last = s.last_event_hash(&ws.id, Some(&session.id)).await.unwrap();
    assert_eq!(last, Some(chain_a.clone()));

    let events = s.events_in_session(&session.id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_hash, chain_a);
}
