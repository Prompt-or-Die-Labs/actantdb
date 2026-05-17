# Phase 6 extensions to existing crates

## Context

Consumed alongside `actant-sync.md`, `actant-audit-export.md`, `studio.md`, and the four SDK work packages. Catalogs the multi-tenant boundary, OIDC scaffold, role/group model, approval pools, quotas, and the CLI cluster commands needed to ship the cloud / team product.

## Scope

Each per-crate section is an independent unit. ADRs 0010 (Postgres vs SQLite-per-workspace) and 0011 (sync conflicts) MUST be authored at the start of Phase 6 before code begins.


## Specs to read first

- `/specs/11-roadmap.md` Phase 6.
- `/specs/01-architecture.md` §"Deployment topologies".
- `/specs/05-security-model.md` §10.
- `/planning/phase-6-plan.md`.
- ADR-0010 (Postgres vs SQLite-per-workspace) and ADR-0011 (sync conflicts) — authored at Phase 6 entry.

## Per-crate work

### `actant-storage`

- Multi-database mode: a `Storage` handle can manage many SQLite files, one per workspace, OR a single Postgres cluster (decision in ADR-0010).
- Connection-pool routing by `workspace_id`.
- Migration runner per workspace.

### `actant-server`

- Multi-workspace boundary: every request carries `workspace_id`; cross-workspace access requires `cross_workspace.*` permissions and emits a distinct audit event.
- OIDC scaffold: `/auth/oidc/login` + `/auth/oidc/callback` with PKCE.
- Service accounts: a non-interactive identity flow for agents and workers.
- Role / group model: `role`, `group`, `group_member` tables (added by Phase 6 migration 0003).
- Approval pools: an approval can be satisfied by any actor in a named pool.
- mTLS enforcement (Phase 6 enables what Phase 1 scaffolded).

### `actant-policy`

Cross-workspace authority: rare but real (shared artifact stores, federated approvals). When a request crosses, emit an audit event recording source + target workspaces.

### `actant-command`

Workspace-management commands: `create_workspace`, `add_member`, `remove_member`, `set_workspace_policy`, `export_audit`, `set_retention`, `create_role`, `assign_role`, `create_group`, `add_to_group`.

### `actant-cli`

`actantdb cluster …` family:

```
actantdb cluster status
actantdb cluster member list
actantdb cluster member add <email> --role <role>
actantdb cluster quota show
actantdb cluster quota set
actantdb cluster retention show
actantdb cluster retention set
actantdb cluster export --window <ISO> --to <s3://...>
```

### `actant-subscribe`

Cross-workspace subscriptions for admins (workspace-set filter).

## Acceptance criteria

- [ ] Phase 6 decision gate in `/planning/phase-6-plan.md` passes.
- [ ] Multi-tenant isolation property tests: no command in workspace A can read/write/observe rows in workspace B without explicit cross-workspace authority.
- [ ] OIDC flow tested against ≥2 providers in CI.
- [ ] Audit exports satisfy a sample SOC 2 evidence checklist.

## Do NOT

- Do NOT remove single-workspace mode. `ActantDB Local` must keep working.
- Do NOT bake any provider-specific auth into the core. OIDC is generic.
- Do NOT introduce cross-tenant joins inside `actant-storage`; always go through `actant-policy` cross-workspace authority checks.
