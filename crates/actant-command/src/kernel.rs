//! Hot-path command coordinator.
//!
//! The kernel's job is to take a tool-call request and dispatch it through
//! Guard, the effect queue, and (when applicable) workers — without crossing
//! a network hop. Phase 1 keeps the kernel single-process and synchronous.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use crate::Engine;
pub use actant_contracts::ActantHotToolCall;
use actant_core::*;

/// One kernel run-through.
pub async fn dispatch_tool_call(
    engine: &Engine,
    req: ActantHotToolCall,
) -> Result<serde_json::Value, ActantError> {
    let workspace_id = WorkspaceId::from_string(req.workspace_id);
    let actor_id = ActorId::from_string(req.actor_id);
    let session_id = SessionId::from_string(req.session_id);
    let out = engine
        .dispatch(
            &workspace_id,
            &actor_id,
            "request_tool_call",
            serde_json::json!({
                "session_id": session_id.as_str(),
                "tool_name": req.tool,
                "arguments": req.arguments,
            }),
            None,
        )
        .await?;
    Ok(out.result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actant_storage::{Storage, StorageConfig};

    #[tokio::test]
    async fn dispatch_round_trip() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let ws = Workspace {
            id: WorkspaceId::new(),
            name: "t".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        s.insert_workspace(&ws).await.unwrap();
        let actor = Actor {
            id: ActorId::new(),
            workspace_id: ws.id.clone(),
            kind: ActorKind::Agent,
            display_name: "a".into(),
            created_at: now_rfc3339(),
            disabled_at: None,
        };
        s.insert_actor(&actor).await.unwrap();
        let engine = Engine::new(s.clone());
        // Create session via the command engine.
        let session = engine
            .dispatch(
                &ws.id,
                &actor.id,
                "create_session",
                serde_json::json!({}),
                None,
            )
            .await
            .unwrap();
        let sid =
            SessionId::from_string(session.result["session_id"].as_str().unwrap().to_string());
        let out = dispatch_tool_call(
            &engine,
            ActantHotToolCall {
                workspace_id: ws.id.to_string(),
                actor_id: actor.id.to_string(),
                session_id: sid.to_string(),
                tool: "file.read".into(),
                arguments: serde_json::json!({"path":"README"}),
            },
        )
        .await
        .unwrap();
        assert!(out["tool_call_id"].is_string());
    }
}
