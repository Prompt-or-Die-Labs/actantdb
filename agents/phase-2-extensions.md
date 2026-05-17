# Phase 2 extensions to existing crates

## Context

This work package is consumed alongside the four Phase 2 worker packages and `actant-worker-protocol.md`. It catalogs what must be added to each existing Phase 1 crate to support Phase 2's extended primitives (intent, observation, delegation, budget, intervention, compensation, drift, rich effect lease, causal DAG). Treat it as a multi-crate work package: each per-crate section below is independently executable, but they must all land in the same release.

## Scope

The per-crate sections below define what changes in each existing crate. A coding agent should work one crate at a time, in the dependency order shown in `/agents/README.md`.

## Specs to read first

- `/specs/13-actant-contract.md` §6 (intent), §7 (observation), §15 (budget), §16 (delegation), §17 (intervention), §11 (effects), §12 (chronicle causal DAG).
- `/specs/14-extended-primitives.md` §§1–6, §9, §11, §15, §16.
- `/specs/adr/0004-intent-action-alignment.md`.
- `/planning/phase-2-plan.md`.

## Per-crate work

### `actant-storage`

Add row mappers, insert/update helpers, and `Transaction` methods for:

- `intent` — `insert_intent`, `update_intent_status`, `latest_open_intent_for_actor`.
- `observation` — `insert_observation`, `update_verification_status`.
- `delegation` — `insert_delegation`, `revoke_delegation`, `complete_delegation`, `list_active_for_child`.
- `budget` — `insert_budget`, `consume_budget` (atomic increment with `enforcement_action` check), `reset_budget`.
- `intervention` — `insert_intervention`.
- `compensation_plan` — `insert_compensation_plan`, `mark_consumed`.
- `drift_signal` — `insert_drift_signal`.
- `effect_claim` — extend the existing claim helper to carry `input_hash`, `permission_scope_ref`, `sandbox_policy_ref`, `max_attempts`.
- `agent_event` — extend `append_event` to accept `causal_parent_ids` (JSON array).
- `session` — `update_session_phase`.
- `tool` — `update_undo_capability`, `update_risk_classifier_ref`.

Acceptance: each helper has a unit test exercising happy + error paths; concurrent `consume_budget` correctness verified by a property test (no over-spend).

### `actant-policy`

Add Guard functionality:

- **Intent–action alignment.** New helper `align(intent: &Intent, proposed: &ProposedEffect) -> AlignmentDecision`. Returns `Allow | RequireApproval | Deny`. Phase 2 implementation: string-class match on `proposed_action_class` + resource overlap against `resource_targets`.
- **Drift scoring.** New helper `compute_drift(actor: &ActorId, session: &SessionId) -> DriftScore` aggregating intent-mismatches, resource-mismatches, risk-escalations, sequence-unusualness over a rolling window.
- **Budget verification.** `verify_budget(actor, session, workflow_run, effect_type)` checked before enqueue.
- **Delegation handling.** Authority lookup unions parent scopes with delegated subsets when child has an active delegation.

Acceptance: each function has fixture-driven tests; alignment ships in **log-only mode** by default with a workspace policy field `intent_enforcement = 'log_only' | 'enforce'`.

### `actant-command`

Add commands:

- `form_intent`, `fulfill_intent`, `mutate_intent`, `abandon_intent`.
- `record_observation`, `verify_observation`.
- `delegate`, `revoke_delegation`, `complete_delegation`.
- `set_budget`, `consume_budget` (internal-only; called by command pipeline before enqueueing effects), `reset_budget`.
- `intervene` (single command with `intervention_type` dispatch).
- `attach_compensation_plan` (internal, called by command engine for `tool.undo_capability != 'irreversible'`).
- `compensate_effect`.

Acceptance: each command has the standard test set (schema validation, authorization denial, success, idempotency, subscriber notification).

Extend `request_tool_call` to require an open `intent_id` for agent-kind actors; auto-form a default intent for non-agent actors. Call `align()` before enqueueing.

### `actant-effects`

Update `claim_pending_effect` to honor rich lease columns. Add `record_observation` on the worker API endpoint. Workers' `complete_effect` carries `final_input_hash`; refuse to complete if it differs from `effect_claim.input_hash`.

### `actant-subscribe`

Add subscription targets: `intent`, `observation`, `delegation`, `budget`, `intervention`, `drift_signal`, `compensation_plan`.

### `actant-server`

Wire the new commands to `POST /v1/command`. Add `POST /v1/effects/{id}/observe`. Webhook auth scaffold (Phase 4 will use). Update OpenAPI metadata so SDK codegen picks up the new commands.

## Acceptance criteria (cross-crate)

- [ ] Phase 2 decision gate in `/planning/phase-2-plan.md` §"Decision gate" passes.
- [ ] No existing Phase 1 test regresses.
- [ ] `just ci` green.
- [ ] Worker conformance suite green for all four reference workers.

## Do NOT

- Do NOT touch the Phase 1 alpha command set's external contract. New columns may be added; renames are forbidden.
- Do NOT make intent alignment enforcing by default. Log-only first.
- Do NOT skip the worker-side input-hash recheck. It's the structural defense against the "argument tampered between scheduling and execution" attack.
