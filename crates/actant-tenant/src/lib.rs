//! actant-tenant — multi-tenant boundary helpers.
//!
//! Every request that mutates state passes through a [`TenantContext`].
//! Repositories tied to the context refuse to touch rows outside the
//! current workspace.
//!
//! Phase 6.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_auth::Principal;
use actant_core::{ActantError, ActorId, WorkspaceId};
use actant_storage::Storage;
use serde::{Deserialize, Serialize};
use sqlx::Row;

/// One authenticated request's tenant context.
#[derive(Debug, Clone)]
pub struct TenantContext {
    /// Verified principal.
    pub principal: Principal,
    /// Storage handle.
    pub storage: Storage,
}

impl TenantContext {
    /// Construct from an authenticated principal.
    pub fn new(principal: Principal, storage: Storage) -> Self {
        Self { principal, storage }
    }

    /// Workspace id for this request.
    pub fn workspace(&self) -> &WorkspaceId {
        &self.principal.workspace_id
    }

    /// Actor id for this request.
    pub fn actor(&self) -> &ActorId {
        &self.principal.actor_id
    }

    /// Returns true if the principal has the named role.
    pub fn has_role(&self, role: &str) -> bool {
        self.principal.roles.iter().any(|r| r == role)
    }

    /// Throw `PermissionDenied` if the actor is missing the role.
    pub fn require_role(&self, role: &str) -> Result<(), ActantError> {
        if self.has_role(role) {
            Ok(())
        } else {
            Err(ActantError::PermissionDenied(format!(
                "role {role} required; got {:?}",
                self.principal.roles
            )))
        }
    }

    /// Verify a target row belongs to the principal's workspace.
    pub async fn assert_event_in_tenant(&self, event_id: &str) -> Result<(), ActantError> {
        let row = sqlx::query("SELECT workspace_id FROM agent_event WHERE id = ?")
            .bind(event_id)
            .fetch_optional(self.storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        let row = row.ok_or_else(|| ActantError::NotFound(format!("event {event_id}")))?;
        let ws: String = row.get("workspace_id");
        if ws != self.workspace().as_str() {
            return Err(ActantError::PermissionDenied(format!(
                "event {event_id} belongs to another tenant"
            )));
        }
        Ok(())
    }
}

/// Group definition (a named collection of roles + members).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    /// Group name.
    pub name: String,
    /// Roles granted to members.
    pub roles: Vec<String>,
    /// Member actor ids.
    pub members: Vec<ActorId>,
}

impl Group {
    /// True if the given actor is a member.
    pub fn contains(&self, actor: &ActorId) -> bool {
        self.members.iter().any(|m| m.as_str() == actor.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actant_core::{now_rfc3339, Actor, ActorKind, Workspace};
    use actant_storage::StorageConfig;

    fn fake_principal(ws: WorkspaceId, actor: ActorId, roles: Vec<&str>) -> Principal {
        Principal {
            workspace_id: ws,
            actor_id: actor,
            roles: roles.into_iter().map(String::from).collect(),
            expires_at: i64::MAX,
        }
    }

    #[tokio::test]
    async fn role_check_blocks_then_allows() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let ws = WorkspaceId::new();
        let actor = ActorId::new();
        let p = fake_principal(ws.clone(), actor, vec!["viewer"]);
        let ctx = TenantContext::new(p, s);
        assert!(ctx.require_role("viewer").is_ok());
        assert!(ctx.require_role("admin").is_err());
    }

    #[tokio::test]
    async fn cross_tenant_event_blocked() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let ws_a = Workspace {
            id: WorkspaceId::new(),
            name: "a".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        let ws_b = Workspace {
            id: WorkspaceId::new(),
            name: "b".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        s.insert_workspace(&ws_a).await.unwrap();
        s.insert_workspace(&ws_b).await.unwrap();
        let actor = Actor {
            id: ActorId::new(),
            workspace_id: ws_a.id.clone(),
            kind: ActorKind::Human,
            display_name: "a".into(),
            created_at: now_rfc3339(),
            disabled_at: None,
        };
        s.insert_actor(&actor).await.unwrap();
        // Insert an event in ws_b directly.
        sqlx::query(
            "INSERT INTO agent_event (id, workspace_id, actor_id, event_type,
                causality_kind, sensitivity, payload_hash, event_hash, created_at)
             VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind("evt_x")
        .bind(ws_b.id.as_str())
        .bind(actor.id.as_str())
        .bind("test")
        .bind("audit")
        .bind("low")
        .bind("h")
        .bind("h")
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .unwrap();
        // Principal is on ws_a. Access to ws_b's event must fail.
        let p = fake_principal(ws_a.id, actor.id, vec!["admin"]);
        let ctx = TenantContext::new(p, s);
        let res = ctx.assert_event_in_tenant("evt_x").await;
        assert!(matches!(res, Err(ActantError::PermissionDenied(_))));
    }
}
