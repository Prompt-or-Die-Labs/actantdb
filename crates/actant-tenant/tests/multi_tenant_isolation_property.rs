//! Multi-tenant isolation property test — closes the "multi-tenant
//! isolation property tests — only cross_tenant_event_blocked happy-path"
//! gap on `agents/phase-6-extensions.md`.
//!
//! Generates many random (tenant_id, event_id) pairs, inserts each event
//! into storage under its tenant, then asserts that for every tenant the
//! `assert_event_in_tenant` boundary refuses access to any event NOT
//! owned by that tenant — across the whole cartesian product.

use actant_auth::Principal;
use actant_core::{now_rfc3339, ActorId, ActorKind, Actor, ActantError, Workspace, WorkspaceId};
use actant_storage::{Storage, StorageConfig};
use actant_tenant::TenantContext;
use proptest::prelude::*;

#[derive(Debug, Clone)]
struct EventSpec {
    tenant_index: usize,
    event_id: String,
}

/// Build a strategy that yields (n_tenants, Vec<EventSpec>) where the
/// event count is bounded between 10 and 100 and the tenant count
/// between 2 and 5. Event ids are unique by construction (index-suffixed).
fn fixture_strategy() -> impl Strategy<Value = (usize, Vec<EventSpec>)> {
    (2usize..=5usize, 10usize..=100usize).prop_map(|(n_tenants, n_events)| {
        let specs = (0..n_events)
            .map(|i| EventSpec {
                tenant_index: i % n_tenants,
                event_id: format!("evt_iso_{i:04}"),
            })
            .collect();
        (n_tenants, specs)
    })
}

fn fake_principal(ws: WorkspaceId, actor: ActorId, roles: Vec<&str>) -> Principal {
    Principal {
        workspace_id: ws,
        actor_id: actor,
        roles: roles.into_iter().map(String::from).collect(),
        expires_at: i64::MAX,
    }
}

async fn run_one_case(n_tenants: usize, specs: &[EventSpec]) -> Result<(), String> {
    let s = Storage::open(StorageConfig::in_memory())
        .await
        .map_err(|e| e.to_string())?;

    // Stand up `n_tenants` workspaces; one actor per workspace.
    let mut workspaces: Vec<WorkspaceId> = Vec::with_capacity(n_tenants);
    let mut actors: Vec<ActorId> = Vec::with_capacity(n_tenants);
    for i in 0..n_tenants {
        let ws = Workspace {
            id: WorkspaceId::new(),
            name: format!("ws_{i}"),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        s.insert_workspace(&ws).await.map_err(|e| e.to_string())?;
        let actor = Actor {
            id: ActorId::new(),
            workspace_id: ws.id.clone(),
            kind: ActorKind::Human,
            display_name: format!("a_{i}"),
            created_at: now_rfc3339(),
            disabled_at: None,
        };
        s.insert_actor(&actor).await.map_err(|e| e.to_string())?;
        workspaces.push(ws.id);
        actors.push(actor.id);
    }

    // Insert each event under its declared tenant via raw SQL (mirroring
    // the inline test in `actant_tenant::tests::cross_tenant_event_blocked`).
    for spec in specs {
        let ws = &workspaces[spec.tenant_index];
        let actor = &actors[spec.tenant_index];
        sqlx::query(
            "INSERT INTO agent_event (id, workspace_id, actor_id, event_type,
                causality_kind, sensitivity, payload_hash, event_hash, created_at)
             VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind(&spec.event_id)
        .bind(ws.as_str())
        .bind(actor.as_str())
        .bind("test")
        .bind("audit")
        .bind("low")
        .bind("h")
        .bind("h")
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .map_err(|e| e.to_string())?;
    }

    // For every (querying_tenant, event) pair, the boundary MUST permit
    // exactly the events owned by querying_tenant and deny all others.
    for (q_idx, ws) in workspaces.iter().enumerate() {
        let principal = fake_principal(ws.clone(), actors[q_idx].clone(), vec!["admin"]);
        let ctx = TenantContext::new(principal, s.clone());
        for spec in specs {
            let res = ctx.assert_event_in_tenant(&spec.event_id).await;
            if spec.tenant_index == q_idx {
                if let Err(e) = res {
                    return Err(format!(
                        "owner tenant {q_idx} was denied own event {}: {e}",
                        spec.event_id
                    ));
                }
            } else {
                match res {
                    Err(ActantError::PermissionDenied(_)) => {}
                    Err(other) => {
                        return Err(format!(
                            "tenant {q_idx} got unexpected error querying \
                             event {} owned by tenant {}: {other}",
                            spec.event_id, spec.tenant_index
                        ));
                    }
                    Ok(()) => {
                        return Err(format!(
                            "isolation breach: tenant {q_idx} permitted to access \
                             event {} owned by tenant {}",
                            spec.event_id, spec.tenant_index
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// 20 random fixtures, each spanning up to 100 events across up to
    /// 5 tenants. For every event in every fixture, every non-owner
    /// tenant MUST be refused access.
    #[test]
    fn no_tenant_can_read_anothers_events((n_tenants, specs) in fixture_strategy()) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        match rt.block_on(run_one_case(n_tenants, &specs)) {
            Ok(()) => {}
            Err(e) => prop_assert!(false, "{e}"),
        }
    }
}
