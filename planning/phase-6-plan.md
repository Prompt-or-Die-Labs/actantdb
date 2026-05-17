# Phase 6 plan — Cloud / team

## Goal

Hosted multi-workspace deployment with SSO, team-level permissions, selective sync, audit exports, and retention policies. Two design-partner customers run ActantDB in production for a month with at least 100 daily commands per workspace each.

## Duration

8–12 weeks.

## New crates introduced

| Crate                | Kind | Purpose                                                       |
| -------------------- | ---- | ------------------------------------------------------------- |
| `actant-sync`        | lib  | Selective sync engine for local-first multi-device mode. Each row's `sync_policy` from its capsule (or default) governs replication. |
| `actant-audit-export` | lib | Nightly JSONL of Chronicle slices, filtered by retention + sensitivity. Pluggable destinations (S3, GCS, local). |
| `actant-quota`       | lib | Per-workspace + per-actor quotas: command rate, subscription count, artifact bytes, model spend. Phase 1 had limits per-actor + per-workspace; Phase 6 adds enforcement at the cluster level and persistent counters. |

`actant-sso` lives inside `actant-server` as an `auth` module rather than a separate crate; OIDC flows don't justify a top-level crate.

## Existing crates expanded

| Crate              | Phase 6 expansion                                                          |
| ------------------ | -------------------------------------------------------------------------- |
| `actant-server`    | Multi-tenant boundary; OIDC + mTLS; service accounts for agents and workers; role / group model; team-approval pools (multiple approvers allowed). |
| `actant-storage`   | Multi-database mode (one SQLite per workspace; one Postgres for the cluster). ADR-0010 picks the tier-1 backend. |
| `actant-policy`    | Cross-workspace authority (the rare cases that need it: shared artifact stores, team approvals). |
| `actant-command`   | Workspace-management commands: `create_workspace`, `add_member`, `remove_member`, `set_workspace_policy`, `export_audit`, `set_retention`. |
| `actant-cli`       | `actantdb cluster …` subcommand family. Health, member, role, quota, retention. |
| `actant-subscribe` | Cross-workspace subscriptions (e.g., a workspace admin watching all of their workspaces' approval requests). |

## Specs landing in Phase 6

The Phase 6 scope in `/specs/11-roadmap.md`:

- Multi-workspace + multi-tenant
- SSO (OIDC) + service accounts
- Team permissions (roles, groups, approval pools)
- Selective sync for local-first multi-device
- Audit exports
- Retention policies
- Hosted deployment image (Docker, Helm)
- Self-hosted enterprise installation guide

Plus the cross-phase track items that have accumulated:

- Migration to Postgres backend (or, if SQLite is deemed sufficient for cluster mode, that becomes ADR-0010-A).

## Studio additions (Phase 6)

- **Workspace switcher** — top-level navigation between workspaces.
- **Members + roles** panel.
- **Quotas** panel — current usage vs limit per quota dimension.
- **Audit Explorer** — full-text + structured search over Chronicle slices.
- **Retention manager** — per-workspace retention windows + on-demand purges.
- **Sync settings** — what syncs where; capsule-level overrides.

## Test strategy

- **Multi-tenant isolation tests.** No command issued in one workspace can read, write, or even observe rows in another. Property tests with fuzzed cross-workspace IDs.
- **Selective sync conformance.** A laptop + phone pair syncs approval requests but not memory text; capsule policies are honored on both ends.
- **SSO flow.** OIDC implicit/code flows tested against at least two providers in CI.
- **Quota exhaustion.** Bucket exhaustion returns 429 with `Retry-After`; quota recovery works on schedule.
- **Audit export integrity.** A nightly export is byte-identical when re-run against the same time window.
- **Retention.** Records past retention are tombstoned (audit skeleton preserved) and their payloads deleted.

## Decision gate

Phase 6 passes when:

1. Two design-partner customers (one team of 5–10, one personal-agent user) run ActantDB in production for one full calendar month with ≥100 daily commands per workspace each.
2. SOC 2 evidence flow: audit exports satisfy at least one well-known compliance check list (a sample evidence package can be produced from the export artifacts).
3. A laptop + phone pair sync correctly: approval requests visible on phone, raw private memory text not transferred.
4. A cross-workspace authority violation is structurally impossible (verified by the multi-tenant isolation property tests).
5. The self-hosted install completes on a single VM in under 30 minutes using the published Helm chart.

## Risks

| Risk                            | Mitigation                                                                       |
| ------------------------------- | -------------------------------------------------------------------------------- |
| Operational toil                | Per-workspace observability + quotas from day one of the phase. Studio surfaces over-budget workspaces. |
| SOC 2 / audit deliverables drift| Audit Explorer + export retention rules are reviewed against a check list before customer sign-off. |
| Postgres backend churn          | ADR-0010 chooses Postgres vs SQLite-per-workspace; either way, the same migration runner contract holds. |
| Multi-tenant performance        | Database-level partition by workspace_id; per-workspace pool sizing; reviewed under load tests at week 6. |
| Sync conflict resolution        | Phase 6 ships last-write-wins for projection rows; the Chronicle is append-only and conflict-free by design. ADR-0011 captures the rule. |

## CLI deliverables (Phase 6)

Per `/planning/cli-design.md` § "CLI staging across phases":

- New subcommands: `actant cluster status|member|quota|retention|export`, `actant deploy cloud|status|logs`, cross-workspace flags on existing commands, `actant policy` cross-workspace mode.
- New template: `enterprise-agent`.
- All ten templates ship by end of phase.
- Three deployment scaffolds: `actant deploy scaffold docker-compose|k8s|helm`.

## Work packages

- `/agents/actant-sync.md`
- `/agents/actant-audit-export.md`
- `/agents/phase-6-extensions.md` (also extends `actant-cli` for cluster + deploy subcommands)
- `/agents/studio.md` (full dashboard — see `studio-design.md` for the consolidated design)
- `/agents/sdk-ts.md`, `/agents/sdk-python.md`, `/agents/sdk-swift.md`, `/agents/sdk-rust.md` — all four SDKs reach v1 by end of Phase 6.

## Sequencing

```
weeks 1-2
  ├── ADR-0010 (Postgres vs SQLite-per-ws) + ADR-0011 (sync conflicts)
  ├── actant-server: multi-workspace boundary
  └── actant-server: OIDC scaffold

weeks 3-4
  ├── actant-sync
  └── actant-audit-export

weeks 5-6
  ├── actant-quota
  ├── Studio multi-tenant screens
  └── Load tests + tuning

weeks 7-8
  ├── Design partner 1 onboarded
  └── Design partner 2 onboarded

weeks 9-12
  ├── Live ops + iteration on both partners
  └── Decision gate review at end of month
```
