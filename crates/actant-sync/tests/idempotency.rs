//! Idempotency — pushing the same batch twice must leave the destination
//! unchanged on the second push (no duplicate files, cursor identical).

use std::collections::HashSet;
use std::sync::Arc;

use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use actant_sync::{Destination, FilesystemDestination};
use tempfile::TempDir;

fn ev(workspace: &WorkspaceId, actor: &ActorId, session: &SessionId, idx: usize) -> AgentEvent {
    let parent_hash = "0".repeat(64);
    let payload = serde_json::json!({"idx": idx});
    let pc = canonical_json(&payload);
    let ph = sha256_hex(pc.as_bytes());
    AgentEvent {
        id: EventId::from_string(format!("evt_{idx:04}")),
        workspace_id: workspace.clone(),
        actor_id: actor.clone(),
        session_id: Some(session.clone()),
        parent_event_id: None,
        event_type: "demo".into(),
        causality_kind: CausalityKind::Audit,
        sensitivity: Sensitivity::Low,
        authority_scope_id: None,
        payload_ref: None,
        payload_inline: Some(pc),
        payload_hash: ph.clone(),
        event_hash: chain_hash(&parent_hash, &ph),
        created_at: format!("2026-05-19T00:00:{:02}Z", idx),
        model_call_id: None,
        tool_call_id: None,
        workflow_run_id: None,
        memory_id: None,
        artifact_id: None,
        command_id: None,
        effect_id: None,
    }
}

async fn fresh_storage_and_batch(n: usize) -> (Storage, WorkspaceId, Vec<AgentEvent>) {
    let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
    let ws = Workspace {
        id: WorkspaceId::new(),
        name: "n".into(),
        created_at: now_rfc3339(),
        archived_at: None,
    };
    s.insert_workspace(&ws).await.unwrap();
    let actor = Actor {
        id: ActorId::new(),
        workspace_id: ws.id.clone(),
        kind: ActorKind::Human,
        display_name: "a".into(),
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
    let mut batch = Vec::new();
    for i in 0..n {
        let e = ev(&ws.id, &actor.id, &session.id, i);
        s.append_event(&e).await.unwrap();
        batch.push(e);
    }
    (s, ws.id, batch)
}

fn list_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&p) {
            for e in entries.flatten() {
                let path = e.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    out.push(path);
                }
            }
        }
    }
    out.sort();
    out
}

#[tokio::test]
async fn double_push_leaves_destination_unchanged() {
    let tmp = TempDir::new().unwrap();
    let (_storage, workspace, batch) = fresh_storage_and_batch(50).await;
    let dest = Arc::new(FilesystemDestination::new(tmp.path()).unwrap());

    let cursor = dest.push(&workspace, None, &batch).await.unwrap();
    assert!(cursor.is_some());

    let files_after_first = list_files(tmp.path());
    let mtimes_after_first: Vec<_> = files_after_first
        .iter()
        .map(|p| std::fs::metadata(p).unwrap().modified().unwrap())
        .collect();
    let cursor_after_first = dest.cursor(&workspace).await.unwrap();

    // Push the identical batch a second time.
    let cursor2 = dest
        .push(&workspace, cursor.as_ref(), &batch)
        .await
        .unwrap();
    assert_eq!(
        cursor, cursor2,
        "cursor value identical after redundant push"
    );

    let files_after_second = list_files(tmp.path());
    assert_eq!(
        files_after_first, files_after_second,
        "no new files after redundant push (got {} -> {})",
        files_after_first.len(),
        files_after_second.len(),
    );
    // No duplicates: file set is exactly the 50 events + 1 cursor file.
    let unique: HashSet<_> = files_after_second.iter().collect();
    assert_eq!(unique.len(), files_after_second.len());
    assert_eq!(
        files_after_second.iter().filter(|p| p.extension()
            .and_then(|s| s.to_str())
            == Some("json"))
        .count(),
        50,
        "50 event files, no duplicates"
    );

    assert_eq!(
        dest.cursor(&workspace).await.unwrap(),
        cursor_after_first,
        "cursor value identical"
    );

    // Modification times for at least one file _may_ change because the
    // implementation does an atomic write of identical bytes — we accept
    // this as an idempotency definition: the *content* and the *file set*
    // are unchanged. We assert on the file set only.
    let _ = mtimes_after_first;
}
