# Phase 2 plan — Effect workers + extended primitives

## Goal

Move ActantDB from "in-process scaffold runs the alpha demo" to "a real fleet of workers performs side effects under Guard, with the Phase 2 slice of the Actant Contract enforced." When this phase ends, the alpha demo runs end-to-end with the in-process tool stubs replaced by real workers — and the system catches a deliberate intent-mismatch scenario.

## Duration

4 weeks with a small team (2–3 engineers, 1 designer for Studio updates).

## New crates introduced

| Crate                       | Kind   | Purpose                                                     |
| --------------------------- | ------ | ----------------------------------------------------------- |
| `actant-worker-protocol`    | lib    | Shared library every worker depends on: claim/heartbeat/start/observe/complete clients, lease type, idempotency helpers, structured-observation builders, compensation-plan helpers. |
| `actant-worker-shell`       | bin    | Shell-effect worker. Spawns child processes with OS-isolated sandboxing. |
| `actant-worker-file`        | bin    | File read/write worker with pattern-bound access.            |
| `actant-worker-model`       | bin    | Model-call worker. Adapts OpenAI-compatible providers + local OpenAI-compatible endpoints. |
| `actant-worker-mcp`         | bin    | Bridge to MCP servers for any registered MCP tool.           |

Each worker is a separate binary so it can be deployed and authorized independently. The protocol library is what keeps them consistent.

## Existing crates expanded

| Crate              | Phase 2 expansion                                                       |
| ------------------ | ----------------------------------------------------------------------- |
| `actant-effects`   | Full claim/heartbeat/start/observe/complete cycle wired to workers. Backoff. Dead-lettering. Rich `effect_claim` columns (`input_hash`, `permission_scope_ref`, `sandbox_policy_ref`, `max_attempts`). |
| `actant-policy`    | Intent–action alignment check, drift signal computation, autonomy budgets, delegation. |
| `actant-command`   | New commands: `form_intent`, `fulfill_intent`, `mutate_intent`, `abandon_intent`, `record_observation`, `verify_observation`, `delegate`, `revoke_delegation`, `complete_delegation`, `set_budget`, `consume_budget`, `reset_budget`, `intervene`, `attach_compensation_plan`, `compensate_effect`. |
| `actant-storage`   | Row mappers for the Phase 2 tables (`intent`, `observation`, `delegation`, `budget`, `intervention`, `compensation_plan`, `drift_signal`); claim helper updated to honor lease columns. |
| `actant-subscribe` | Subscription targets for new tables: `intent`, `observation`, `delegation`, `budget`, `intervention`, `drift_signal`. |
| `actant-server`    | `POST /v1/effects/{id}/observe`, drift events on the websocket, command handlers for the new commands. Workers point their HTTP clients here. |

## Specs landing in Phase 2

From `/specs/14-extended-primitives.md` §17:

- Intent + intent-action alignment + autonomy drift
- Observation (structured)
- Budget + delegation
- Compensation plan
- Session phase column + intervention
- Rich effect lease
- Causal DAG (`agent_event.causal_parent_ids` populated by commands)

Phase 3+ primitives (capsules, memory conflict, trust profile, regret, eval, model route decision, context debt) **do not** land in Phase 2. The schema is already migrated (0002), but they remain unused until later phases.

## Worker-fleet architecture

The detailed cross-phase worker architecture lives in `worker-fleet.md`. Phase 2's contribution:

1. The four reference workers above.
2. The `actant-worker-protocol` library.
3. A `WORKER_AUTH_TOKEN` flow so a worker authenticates to the server with its own actor identity.
4. Per-worker sandboxing rules:
   - Shell worker: child processes under a project-rooted sandbox; network denied unless effect grants it.
   - File worker: pattern-bound (`authority_scope.resource_pattern`); refuses writes outside.
   - Model worker: HTTP only; no local FS access; no shell.
   - MCP worker: opens registered MCP transports only; one tool call at a time per lease.

## Studio additions (Phase 2)

See `studio-design.md` for the full pass. Phase 2 specifically:

- **Workers** panel — live heartbeats, in-flight count, claim history.
- **Effect Queue** panel — pending / claimed / running / dead-letter rows with manual retry.
- **Intent Inspector** under each session — intents declared, fulfilled, mismatched.
- **Drift Monitor** — drift_signal rows, score over time, intervention links.
- **Approval Center** gains compensation-plan preview ("This file write is reversible").

## Test strategy

Detailed in `test-strategy.md`. Phase-2-specific tests:

- **Worker conformance suite.** A harness drives a mock server and asserts every reference worker satisfies the protocol. Used to certify third-party workers later.
- **Lease-loss property tests.** Random worker kills mid-effect produce clean re-claim with `attempt_count += 1`.
- **Idempotency conformance.** Each worker is tested against duplicate-claim scenarios; the result either short-circuits via local de-dupe ledger or re-runs with the external idempotency key.
- **Intent–action alignment fixtures.** A small corpus of `(intent, proposed_effect, expected_decision)` triples runs in CI; alignment regressions break the build.
- **Drift fixtures.** Sessions with deliberate drift (resource jumps, risk escalation, off-task tools) verify the signal crosses threshold and triggers the right intervention.

## Decision gate

Phase 2 passes when:

1. The alpha demo (`/specs/10-alpha-demo.md`) runs end-to-end with the in-process tool stubs replaced by `actant-worker-shell` + `actant-worker-file` + `actant-worker-model`.
2. Killing any worker mid-effect produces a clean lease-loss → retry path that the next worker picks up; the result still appears in the user's chat.
3. A scripted drift scenario (agent declares `inspect tests`, proposes `rm -rf ~/.ssh`) emits an `intent_action_mismatch` event and either blocks the effect or escalates to approval — verifiable in the Studio Drift Monitor.
4. An end-to-end approval flow with compensation preview ("revert this file write") functions in Studio.
5. The worker conformance suite passes for all four reference workers.

## Risks

| Risk                                          | Mitigation                                                                        |
| --------------------------------------------- | --------------------------------------------------------------------------------- |
| Worker security regressions                   | Workers authenticate as their own actor; Guard gates every claim; OS isolation per kind. |
| Intent–action alignment false positives        | Ship in **log-only** mode for the first 2 weeks of any deployment; promote to enforcing once base rate is understood. Phase 4 may swap in a model-assisted evaluator. |
| Backpressure under load                       | Per-effect-type concurrency limits server-side and worker-side. Circuit breakers in policy. |
| Idempotency leaks                             | Each worker's de-dupe ledger gets a property test. External-API idempotency keys recorded in `effect_result.error` when present. |
| Drift threshold tuning                        | Workspace-level `drift_threshold` defaults to 0.7. Studio surfaces score distributions so operators can tune. |
| Schema additions break Phase 1 agents         | Migration 0002 is non-destructive. Phase 1 work packages already exist; Phase 2 adds new commands and tables but doesn't change old ones. |

## CLI deliverables (Phase 2)

Per `/planning/cli-design.md` § "CLI staging across phases":

- New subcommands: `actant effect list|inspect|retry`, `actant worker list|start|logs|register`, `actant tool list|register|show`, `actant intent list|inspect`, `actant intervene`, `actant mcp add|import|list|tools|wrap`.
- New templates: `browser-agent`, `mcp-agent`, `desktop-agent`.
- New examples: `tool-approval`, `mcp-github`.
- `actant doctor` learns about worker health and intent enforcement mode.

## Work packages (pointers)

- `/agents/actant-worker-protocol.md`
- `/agents/actant-worker-shell.md`
- `/agents/actant-worker-file.md`
- `/agents/actant-worker-model.md`
- `/agents/actant-worker-mcp.md`
- `/agents/phase-2-extensions.md` (extends `actant-effects`, `actant-policy`, `actant-command`, `actant-storage`, `actant-subscribe`, `actant-server`, plus `actant-cli` for the new subcommands)

## Sequencing within the phase

```
week 1
  ├── actant-worker-protocol (shared lib)
  ├── actant-effects expansion (lease columns + claim helper)
  └── actant-storage row mappers for Phase 2 tables

week 2
  ├── actant-worker-shell  ┐
  ├── actant-worker-file   │  in parallel
  ├── actant-worker-model  ┘
  └── actant-policy: intent + alignment

week 3
  ├── actant-worker-mcp
  ├── actant-policy: budget + delegation + drift
  └── actant-command: new commands + intervention

week 4
  ├── actant-server endpoints
  ├── actant-subscribe new tables
  ├── Studio Phase 2 screens
  └── End-to-end demo + decision gate
```
