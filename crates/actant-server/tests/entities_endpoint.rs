//! /v1/entities and /v1/entity-relations — wire smoke tests.

use std::net::SocketAddr;

use actant_core::*;
use actant_server::{router, AppState};
use actant_storage::{Storage, StorageConfig};

async fn start() -> String {
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::from_string("ws_default".to_string()),
        name: "d".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::from_string("act_system".to_string()),
        workspace_id: ws.id.clone(),
        kind: ActorKind::System,
        display_name: "system".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    storage.insert_actor(&actor).await.unwrap();

    let state = AppState::new(storage);
    let router = router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .unwrap();
    let bound = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{bound}")
}

#[tokio::test]
async fn create_then_list_entities() {
    let base = start().await;
    let c = reqwest::Client::new();
    let r = c
        .post(format!("{base}/v1/entities"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "type": "person",
            "canonical_name": "Alice",
            "aliases": ["A.", "Alice Smith"],
            "sensitivity": "low",
        }))
        .send()
        .await
        .unwrap();
    assert!(
        r.status().is_success(),
        "create entity failed: {:?}",
        r.status()
    );
    let body: serde_json::Value = r.json().await.unwrap();
    let id = body["id"].as_str().unwrap();
    assert!(id.starts_with("ent_"));

    let r = c
        .get(format!("{base}/v1/entities?workspace_id=ws_default"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    let entities = body["entities"].as_array().unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0]["id"], id);
    assert_eq!(entities[0]["canonical_name"], "Alice");
    assert_eq!(entities[0]["aliases"][1], "Alice Smith");
}

#[tokio::test]
async fn list_entities_filters_by_type() {
    let base = start().await;
    let c = reqwest::Client::new();
    for (typ, name) in &[("person", "Alice"), ("place", "Cafe"), ("person", "Bob")] {
        let r = c
            .post(format!("{base}/v1/entities"))
            .json(&serde_json::json!({
                "workspace_id": "ws_default",
                "type": typ,
                "canonical_name": name,
            }))
            .send()
            .await
            .unwrap();
        assert!(r.status().is_success());
    }
    let r = c
        .get(format!(
            "{base}/v1/entities?workspace_id=ws_default&type=person"
        ))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    let entities = body["entities"].as_array().unwrap();
    assert_eq!(entities.len(), 2);
    for e in entities {
        assert_eq!(e["type"], "person");
    }
}

async fn make_entity(base: &str, name: &str) -> String {
    let r = reqwest::Client::new()
        .post(format!("{base}/v1/entities"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "type": "person",
            "canonical_name": name,
        }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn link_then_list_relations_by_entity() {
    let base = start().await;
    let alice = make_entity(&base, "Alice").await;
    let bob = make_entity(&base, "Bob").await;
    let carol = make_entity(&base, "Carol").await;

    let c = reqwest::Client::new();
    for (src, rel, tgt) in &[(&alice, "knows", &bob), (&bob, "knows", &carol)] {
        let r = c
            .post(format!("{base}/v1/entity-relations"))
            .json(&serde_json::json!({
                "workspace_id": "ws_default",
                "source_entity": src,
                "relation_type": rel,
                "target_entity": tgt,
                "confidence": 0.9,
            }))
            .send()
            .await
            .unwrap();
        assert!(
            r.status().is_success(),
            "create relation failed: {:?}",
            r.status()
        );
        let body: serde_json::Value = r.json().await.unwrap();
        assert!(body["id"].as_str().unwrap().starts_with("rel_"));
    }

    // Bob participates in both relations (as target of one, source of the other).
    let r = c
        .get(format!(
            "{base}/v1/entity-relations?workspace_id=ws_default&entity={bob}"
        ))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    let relations = body["relations"].as_array().unwrap();
    assert_eq!(relations.len(), 2);
}

#[tokio::test]
async fn create_relation_rejects_invalid_confidence() {
    let base = start().await;
    let alice = make_entity(&base, "Alice").await;
    let bob = make_entity(&base, "Bob").await;
    let r = reqwest::Client::new()
        .post(format!("{base}/v1/entity-relations"))
        .json(&serde_json::json!({
            "workspace_id": "ws_default",
            "source_entity": alice,
            "relation_type": "knows",
            "target_entity": bob,
            "confidence": 2.5,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 400);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["error"], "invalid_input");
}
