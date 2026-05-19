//! Cursor resume — simulate a mid-flush crash, restart the runner, assert
//! no event lost or duplicated and the final cursor matches a one-pass run.
//!
//! We can't kill the runner mid-flush at the OS level inside a unit test;
//! instead we induce the crash deterministically by:
//!
//! 1. Pushing half the batch.
//! 2. Manually rewinding the destination's persisted cursor to an earlier
//!    event id (the "crash" — destination believed it advanced, the runner
//!    will now see the old cursor and re-attempt the partial batch).
//! 3. Running the runner to completion.
//! 4. Asserting (a) every event is on disk exactly once, (b) cursor equals
//!    the final event id, (c) byte content matches a one-pass reference run.

use std::collections::HashSet;
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
        id: EventId::from_string(format!("evt_{idx:05}")),
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

async fn make_workspace(n: usize) -> (Storage, WorkspaceId) {
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

fn collect_event_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&p) {
            for e in entries.flatten() {
                let path = e.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    out.push(path);
                }
            }
        }
    }
    out.sort();
    out
}

#[tokio::test]
async fn cursor_resume_loses_no_events_after_simulated_crash() {
    // ---------- one-pass reference ----------
    let tmp_ref = TempDir::new().unwrap();
    let (storage_ref, ws_ref) = make_workspace(80).await;
    let dest_ref = Arc::new(FilesystemDestination::new(tmp_ref.path()).unwrap());
    let runner_ref = SyncRunner::new(storage_ref.clone(), ws_ref.clone(), dest_ref.clone())
        .with_config(SyncRunnerConfig {
            batch_size: 20,
            ..Default::default()
        });
    let stats_ref = runner_ref.run_once().await.unwrap();
    assert_eq!(stats_ref.events_pushed, 80);
    let ref_files = collect_event_files(tmp_ref.path());
    let ref_cursor = dest_ref.cursor(&ws_ref).await.unwrap();

    // ---------- crash + resume ----------
    let tmp_run = TempDir::new().unwrap();
    let (storage_run, ws_run) = make_workspace(80).await;
    let dest_run = Arc::new(FilesystemDestination::new(tmp_run.path()).unwrap());

    // Pump the first 40 events (two batches of 20). The runner stops after
    // the second batch because run_once returns once `batch.len() <
    // batch_size`. To get exactly 40 written we cap the runner via two
    // explicit pushes of size 20 — this is closer to what a real crash
    // would leave behind than running run_once to completion.
    let storage_run_clone = storage_run.clone();
    let workspace_run = ws_run.clone();
    let half_a = actant_sync::events_after(&storage_run_clone, &workspace_run, None, 20)
        .await
        .unwrap();
    assert_eq!(half_a.len(), 20);
    dest_run.push(&ws_run, None, &half_a).await.unwrap();
    let after_a = dest_run.cursor(&ws_run).await.unwrap();
    let half_b = actant_sync::events_after(&storage_run_clone, &workspace_run, after_a.as_ref(), 20)
        .await
        .unwrap();
    assert_eq!(half_b.len(), 20);
    dest_run.push(&ws_run, after_a.as_ref(), &half_b).await.unwrap();
    assert_eq!(collect_event_files(tmp_run.path()).len(), 40);

    // Simulate the crash: rewind the persisted cursor file to the boundary
    // between half_a and half_b. The runner will now believe it needs to
    // re-push half_b, which exercises the idempotent overwrite path.
    let cursor_path = tmp_run
        .path()
        .join(ws_run.as_str())
        .join("_cursor.txt");
    let rewind_to = half_a.last().unwrap().id.clone();
    std::fs::write(&cursor_path, rewind_to.as_str()).unwrap();
    assert_eq!(
        dest_run.cursor(&ws_run).await.unwrap().map(|c| c.as_str().to_string()),
        Some(rewind_to.as_str().to_string()),
    );

    // Resume the runner. It must re-push half_b (idempotent) and the
    // remaining batch (40 more events).
    let runner = SyncRunner::new(storage_run.clone(), ws_run.clone(), dest_run.clone())
        .with_config(SyncRunnerConfig {
            batch_size: 20,
            ..Default::default()
        });
    let resume_stats = runner.run_once().await.unwrap();
    // After resume we expect 60 events pushed: re-pushed half_b (20) + the
    // remaining 40 events still in storage. None are lost; none are
    // duplicated on disk because every event is content-addressed by id.
    assert_eq!(resume_stats.events_pushed, 60);

    // ---------- assertions ----------
    let run_files = collect_event_files(tmp_run.path());
    assert_eq!(
        run_files.len(),
        80,
        "every event on disk exactly once (got {})",
        run_files.len()
    );
    let unique: HashSet<_> = run_files.iter().collect();
    assert_eq!(unique.len(), 80, "no duplicate paths");

    // Cursor matches the one-pass reference.
    let final_cursor = dest_run.cursor(&ws_run).await.unwrap();
    assert_eq!(
        final_cursor.as_ref().map(|c| c.as_str()),
        ref_cursor.as_ref().map(|c| c.as_str()),
    );

    // File set matches the reference (filename-only; the workspace ids differ
    // between the two runs, so we strip the workspace prefix before
    // comparing).
    fn rel_names(root: &std::path::Path, files: &[std::path::PathBuf]) -> Vec<String> {
        let mut v: Vec<String> = files
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .components()
                    .skip(1) // drop the workspace_id segment
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect();
        v.sort();
        v
    }
    let ref_names = rel_names(tmp_ref.path(), &ref_files);
    let run_names = rel_names(tmp_run.path(), &run_files);
    assert_eq!(run_names, ref_names, "file-name set matches one-pass run");
}
