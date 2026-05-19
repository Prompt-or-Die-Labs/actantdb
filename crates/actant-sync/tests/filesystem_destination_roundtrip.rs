//! `FilesystemDestination` round-trip — pushes 100 events to a tempdir,
//! asserts files written + cursor advanced + resume picks up where it left off.
//!
//! Closes the verification clause on [`GAPS.md` row #16] for the filesystem
//! backend.

use std::sync::Arc;

use actant_core::*;
use actant_storage::{Storage, StorageConfig};
use actant_sync::{Destination, FilesystemDestination, SyncRunner, SyncRunnerConfig};
use tempfile::TempDir;

fn ev(workspace: &WorkspaceId, actor: &ActorId, session: &SessionId, idx: usize) -> AgentEvent {
    let parent_hash = "0".repeat(64);
    let payload = serde_json::json!({"idx": idx});
    let pc = canonical_json(&payload);
    let ph = sha256_hex(pc.as_bytes());
    AgentEvent {
        id: EventId::from_string(format!("evt_{idx:06}")),
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
        // Stable timestamp varying by idx so `events_after` is monotonic
        // even on platforms with low-resolution clocks. Format: RFC-3339.
        created_at: format!("2026-05-19T00:{:02}:{:02}Z", idx / 60, idx % 60),
        model_call_id: None,
        tool_call_id: None,
        workflow_run_id: None,
        memory_id: None,
        artifact_id: None,
        command_id: None,
        effect_id: None,
    }
}

async fn fresh_store(n: usize) -> (Storage, WorkspaceId) {
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
    for i in 0..n {
        s.append_event(&ev(&ws.id, &actor.id, &session.id, i))
            .await
            .unwrap();
    }
    (s, ws.id)
}

fn count_event_files(root: &std::path::Path) -> usize {
    let mut n = 0;
    for entry in walkdir(root) {
        if entry.extension().and_then(|s| s.to_str()) == Some("json") {
            n += 1;
        }
    }
    n
}

fn walkdir(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&p) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}

#[tokio::test]
async fn filesystem_destination_roundtrips_100_events() {
    let tmp = TempDir::new().unwrap();
    let (storage, workspace) = fresh_store(100).await;
    let dest = Arc::new(FilesystemDestination::new(tmp.path()).unwrap());

    // Sanity: cursor empty before any push.
    assert!(dest.cursor(&workspace).await.unwrap().is_none());

    let runner = SyncRunner::new(storage.clone(), workspace.clone(), dest.clone())
        .with_config(SyncRunnerConfig {
            batch_size: 25,
            ..Default::default()
        });
    let stats = runner.run_once().await.unwrap();
    assert_eq!(stats.events_pushed, 100);
    assert_eq!(stats.batches, 4, "100 events / batch_size=25 -> 4 batches");

    // Every event lands as its own `.json` file under
    // `<workspace_id>/<YYYY-MM-DD>/<event_id>.json`. Per-event = 100 files.
    let files = count_event_files(tmp.path());
    assert_eq!(
        files, 100,
        "every event produces exactly one .json (got {files})"
    );

    // Cursor advanced to the final event.
    let cursor = dest.cursor(&workspace).await.unwrap();
    assert_eq!(cursor.as_ref().map(|c| c.as_str()), Some("evt_000099"));

    // Resume: another run_once on the same destination MUST be a no-op
    // because the cursor already points past every storage row.
    let stats2 = runner.run_once().await.unwrap();
    assert_eq!(
        stats2.events_pushed, 0,
        "resume after full drain pushes nothing"
    );
    assert_eq!(count_event_files(tmp.path()), 100, "no duplicate files");

    // Append 10 more events; resume picks up exactly those.
    {
        let actor = ActorId::from_string("act_resume");
        let session = SessionId::from_string("sess_resume");
        // We need an actor + session row for the FKs.
        let s2 = storage.clone();
        s2.insert_actor(&Actor {
            id: actor.clone(),
            workspace_id: workspace.clone(),
            kind: ActorKind::Human,
            display_name: "r".into(),
            created_at: now_rfc3339(),
            disabled_at: None,
        })
        .await
        .unwrap();
        s2.insert_session(&Session {
            id: session.clone(),
            workspace_id: workspace.clone(),
            title: None,
            initiator_actor_id: actor.clone(),
            agent_actor_id: None,
            status: SessionStatus::Active,
            created_at: now_rfc3339(),
            closed_at: None,
        })
        .await
        .unwrap();
        for i in 100..110 {
            s2.append_event(&ev(&workspace, &actor, &session, i))
                .await
                .unwrap();
        }
    }
    let stats3 = runner.run_once().await.unwrap();
    assert_eq!(stats3.events_pushed, 10);
    assert_eq!(
        count_event_files(tmp.path()),
        110,
        "10 additional files for the new events"
    );
    let cursor = dest.cursor(&workspace).await.unwrap();
    assert_eq!(cursor.as_ref().map(|c| c.as_str()), Some("evt_000109"));
}
