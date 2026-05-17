//! actant-bench — shared helpers for the criterion benchmarks under
//! `benches/`. Not consumed by the rest of the workspace.

#![forbid(unsafe_code)]

use actant_command::Engine;
use actant_core::{now_rfc3339, Actor, ActorId, ActorKind, Workspace, WorkspaceId};
use actant_storage::{Storage, StorageConfig};

/// Prepare an in-memory storage with one workspace + actor, return everything
/// the benchmarks need.
pub async fn fresh() -> (Engine, WorkspaceId, ActorId) {
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "bench".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Agent,
        display_name: "bench-agent".into(),
        created_at: now_rfc3339(),
        disabled_at: None,
    };
    s.insert_actor(&actor).await.unwrap();
    (Engine::new(s), ws.id, actor.id)
}
