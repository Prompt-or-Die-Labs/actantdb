//! Exercise `Storage::put_artifact` end-to-end: blob lands in the configured
//! [`actant_storage::BlobStore`] and the `artifact` row references it.

use std::sync::Arc;

use actant_core::*;
use actant_storage::{BlobStore, MemoryStore, Storage, StorageConfig};
use bytes::Bytes;

async fn fixture() -> (Storage, WorkspaceId, ActorId, Arc<MemoryStore>) {
    let blob = Arc::new(MemoryStore::with_id("test"));
    let storage = Storage::open(StorageConfig::in_memory())
        .await
        .expect("open")
        .with_blob_store(blob.clone());

    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "demo".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Agent,
        display_name: "agent".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    storage.insert_actor(&actor).await.unwrap();
    (storage, ws.id, actor.id, blob)
}

#[tokio::test]
async fn put_artifact_writes_blob_and_row() {
    let (storage, ws, actor, blob) = fixture().await;
    let body = Bytes::from_static(b"a stored artifact body");
    let id = storage
        .put_artifact(&ws, &actor, "report", body.clone(), Sensitivity::Low)
        .await
        .expect("put_artifact");

    // Metadata row exists and the URI points back at the blob store.
    let uri = storage.get_artifact_uri(&id).await.unwrap().expect("row");
    assert!(uri.starts_with("mem://test/"), "uri = {uri}");

    // The blob round-trips.
    let got = blob.get(&uri).await.expect("blob get");
    assert_eq!(got, body);
}

#[tokio::test]
async fn put_artifact_uses_content_hash_as_key() {
    let (storage, ws, actor, blob) = fixture().await;
    let body = Bytes::from_static(b"identical bytes");
    // Two writes with the same body must produce the same blob URI (content
    // addressed) but two distinct artifact rows.
    let id_a = storage
        .put_artifact(&ws, &actor, "report", body.clone(), Sensitivity::Low)
        .await
        .unwrap();
    let id_b = storage
        .put_artifact(&ws, &actor, "report", body.clone(), Sensitivity::Low)
        .await
        .unwrap();
    assert_ne!(id_a, id_b);
    let uri_a = storage.get_artifact_uri(&id_a).await.unwrap().unwrap();
    let uri_b = storage.get_artifact_uri(&id_b).await.unwrap().unwrap();
    assert_eq!(uri_a, uri_b);

    // The blob holds the expected body.
    let got = blob.get(&uri_a).await.unwrap();
    assert_eq!(got, body);
}

#[tokio::test]
async fn default_blob_store_for_in_memory_db_is_memory() {
    // Without an explicit `with_blob_store`, an in-memory DB still has a
    // working blob store (a MemoryStore) — exercise put_artifact against it.
    let storage = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "demo".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    storage.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::System,
        display_name: "sys".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    storage.insert_actor(&actor).await.unwrap();
    let id = storage
        .put_artifact(
            &ws.id,
            &actor.id,
            "tool_output",
            Bytes::from_static(b"x"),
            Sensitivity::Low,
        )
        .await
        .expect("put_artifact");
    let uri = storage.get_artifact_uri(&id).await.unwrap().unwrap();
    assert!(uri.starts_with("mem://"), "uri = {uri}");

    // The Storage-level blob_store accessor returns a handle we can read from.
    let got = storage.blob_store().get(&uri).await.unwrap();
    assert_eq!(&got[..], b"x");
}
