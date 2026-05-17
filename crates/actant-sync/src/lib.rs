//! actant-sync — cluster / multi-device synchronization.
//!
//! Phase 1 surface: a deterministic difference engine between two event
//! streams. Phase 6 supplies the wire protocol.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::*;

/// Compute the set of event ids in `a` not present in `b`.
pub fn missing_in(a: &[AgentEvent], b: &[AgentEvent]) -> Vec<EventId> {
    let set: std::collections::HashSet<&str> = b.iter().map(|e| e.id.as_str()).collect();
    a.iter()
        .filter(|e| !set.contains(e.id.as_str()))
        .map(|e| e.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: &str) -> AgentEvent {
        AgentEvent {
            id: EventId::from_string(id.to_string()),
            workspace_id: WorkspaceId::new(),
            actor_id: ActorId::new(),
            session_id: None,
            parent_event_id: None,
            event_type: "x".into(),
            causality_kind: CausalityKind::Audit,
            sensitivity: Sensitivity::Low,
            authority_scope_id: None,
            payload_ref: None,
            payload_inline: None,
            payload_hash: "h".into(),
            event_hash: "h".into(),
            created_at: now_rfc3339(),
            model_call_id: None,
            tool_call_id: None,
            workflow_run_id: None,
            memory_id: None,
            artifact_id: None,
            command_id: None,
            effect_id: None,
        }
    }

    #[test]
    fn missing_set() {
        let a = vec![ev("e1"), ev("e2"), ev("e3")];
        let b = vec![ev("e1")];
        let m = missing_in(&a, &b);
        assert_eq!(m.len(), 2);
    }
}
