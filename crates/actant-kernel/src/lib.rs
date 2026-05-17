//! actant-kernel — hot-path coordinator.
//!
//! The kernel's job is to take a tool-call request and dispatch it through
//! Guard, the effect queue, and (when applicable) workers — without crossing
//! a network hop. Phase 1 keeps the kernel single-process and synchronous.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_command::Engine;
use actant_core::*;
use serde::{Deserialize, Serialize};

/// Hot-path tool-call request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotToolCall {
    /// Workspace.
    pub workspace_id: WorkspaceId,
    /// Calling actor.
    pub actor_id: ActorId,
    /// Optional session anchor.
    pub session_id: SessionId,
    /// Tool name.
    pub tool: String,
    /// Arguments.
    pub arguments: serde_json::Value,
}

/// One kernel run-through.
pub async fn dispatch_tool_call(
    engine: &Engine,
    req: HotToolCall,
) -> Result<serde_json::Value, ActantError> {
    let out = engine
        .dispatch(
            &req.workspace_id,
            &req.actor_id,
            "request_tool_call",
            serde_json::json!({
                "session_id": req.session_id.as_str(),
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
            HotToolCall {
                workspace_id: ws.id.clone(),
                actor_id: actor.id.clone(),
                session_id: sid,
                tool: "file.read".into(),
                arguments: serde_json::json!({"path":"README"}),
            },
        )
        .await
        .unwrap();
        assert!(out["tool_call_id"].is_string());
    }
}
