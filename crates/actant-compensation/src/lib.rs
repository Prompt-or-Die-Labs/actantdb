//! actant-compensation — compensation-plan generator.
//!
//! Phase 2 primitive (see `/specs/14-extended-primitives.md`). Whenever an
//! effect for a tool with `undo_capability != 'irreversible'` is scheduled,
//! a `compensation_plan` row is written so the system has a recipe for
//! rolling back if a downstream step fails.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::{now_rfc3339, ActantError, EffectId, WorkspaceId};
use actant_storage::Storage;
use serde::{Deserialize, Serialize};

/// Tool reversibility classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UndoCapability {
    /// Cannot be undone (e.g. shell.run, email.send).
    Irreversible,
    /// Can be undone within a bounded window (`file.write` → restore prior
    /// content from `pre_state_artifact_ref`).
    Reversible,
    /// Logical undo only (e.g. flag-flip).
    Logical,
}

/// What the compensation should do, in shape suitable for a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensationPlan {
    /// Row id.
    pub id: String,
    /// Effect this plan compensates.
    pub effect_id: EffectId,
    /// Capability classification.
    pub undo_capability: UndoCapability,
    /// Effect type to schedule as the inverse.
    pub compensation_effect_type: Option<String>,
    /// Optional artifact carrying the pre-state (file write → previous content).
    pub pre_state_artifact_ref: Option<String>,
}

/// Generate a plan and persist it. Returns `Ok(None)` for irreversible tools.
pub async fn generate(
    storage: &Storage,
    workspace: &WorkspaceId,
    effect: &EffectId,
    undo_capability: UndoCapability,
    pre_state_artifact_ref: Option<String>,
) -> Result<Option<CompensationPlan>, ActantError> {
    if matches!(undo_capability, UndoCapability::Irreversible) {
        return Ok(None);
    }
    let compensation_effect_type = match undo_capability {
        UndoCapability::Reversible => Some("file.restore".to_string()),
        UndoCapability::Logical => Some("flag.unset".to_string()),
        UndoCapability::Irreversible => None,
    };
    let id = format!("comp_{}", ulid::Ulid::new());
    sqlx::query(
        "INSERT INTO compensation_plan
            (id, workspace_id, effect_id, undo_capability,
             compensation_effect_type, pre_state_artifact_ref, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace.as_str())
    .bind(effect.as_str())
    .bind(match undo_capability {
        UndoCapability::Reversible => "reversible",
        UndoCapability::Logical => "logical",
        UndoCapability::Irreversible => "irreversible",
    })
    .bind(&compensation_effect_type)
    .bind(&pre_state_artifact_ref)
    .bind(now_rfc3339())
    .execute(storage.pool())
    .await
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(Some(CompensationPlan {
        id,
        effect_id: effect.clone(),
        undo_capability,
        compensation_effect_type,
        pre_state_artifact_ref,
    }))
}

/// Mark a plan consumed (the compensation has been applied; it shouldn't be
/// applied again).
pub async fn mark_consumed(storage: &Storage, plan_id: &str) -> Result<(), ActantError> {
    sqlx::query("UPDATE compensation_plan SET consumed_at = ? WHERE id = ?")
        .bind(now_rfc3339())
        .bind(plan_id)
        .execute(storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actant_core::*;
    use actant_storage::StorageConfig;

    async fn fixture() -> (Storage, WorkspaceId, EffectId) {
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
            kind: ActorKind::System,
            display_name: "x".into(),
            created_at: now_rfc3339(),
            disabled_at: None,
        };
        s.insert_actor(&actor).await.unwrap();
        // Need a command_record (effect FK).
        let cmd = CommandRecord {
            id: CommandId::new(),
            workspace_id: ws.id.clone(),
            actor_id: actor.id.clone(),
            session_id: None,
            command_type: "t".into(),
            input_inline: None,
            input_hash: "h".into(),
            policy_id: None,
            status: CommandStatus::Committed,
            error: None,
            created_at: now_rfc3339(),
            committed_at: None,
        };
        s.insert_command(&cmd).await.unwrap();
        let effect_id = EffectId::new();
        sqlx::query(
            "INSERT INTO effect (id, workspace_id, command_id, requested_by_actor_id,
                                 effect_type, status, risk_level, input_hash,
                                 attempt_count, max_attempts, created_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(effect_id.as_str())
        .bind(ws.id.as_str())
        .bind(cmd.id.as_str())
        .bind(actor.id.as_str())
        .bind("file.write")
        .bind("pending")
        .bind("medium")
        .bind("h")
        .bind(0i64)
        .bind(3i64)
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .unwrap();
        (s, ws.id, effect_id)
    }

    #[tokio::test]
    async fn reversible_writes_a_plan() {
        let (s, ws, effect) = fixture().await;
        let plan = generate(
            &s,
            &ws,
            &effect,
            UndoCapability::Reversible,
            Some("art:pre".into()),
        )
        .await
        .unwrap()
        .expect("plan");
        assert_eq!(
            plan.compensation_effect_type.as_deref(),
            Some("file.restore")
        );
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM compensation_plan")
            .fetch_one(s.pool())
            .await
            .unwrap();
        assert_eq!(n, 1);
        mark_consumed(&s, &plan.id).await.unwrap();
        let (consumed,): (Option<String>,) =
            sqlx::query_as("SELECT consumed_at FROM compensation_plan WHERE id = ?")
                .bind(&plan.id)
                .fetch_one(s.pool())
                .await
                .unwrap();
        assert!(consumed.is_some());
    }

    #[tokio::test]
    async fn irreversible_skips_plan() {
        let (s, ws, effect) = fixture().await;
        let plan = generate(&s, &ws, &effect, UndoCapability::Irreversible, None)
            .await
            .unwrap();
        assert!(plan.is_none());
    }
}
