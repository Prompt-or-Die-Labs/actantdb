# Phase 4 extensions to existing crates

## Context

Consumed alongside `actant-trigger.md` and `actant-eval.md`. Catalogs Phase 4 changes in existing crates: the Flow Engine executor lands here, workflow commands ship, regret/eval commands ship, and the model-route-decision row begins to be written on every model call.

## Scope

Each per-crate section is an independent unit of work; together they enable the daily-digest workflow demo from `/specs/10-alpha-demo.md` §14.


## Specs to read first

- `/specs/07-workflows-and-replay.md` §§1–5.
- `/specs/14-extended-primitives.md` §6 (regret), §7 (eval), §12 (model route decision).
- `/specs/adr/0006-regret-hooks.md`.
- `/planning/phase-4-plan.md`.

## Per-crate work

### `actant-storage`

Row mappers + helpers for `regret_event`, `eval_case`, `eval_run`, `model_route_decision`. Workflow tables (`workflow`, `workflow_node`, `workflow_edge`, `workflow_run`, `workflow_step_run`, `trigger`, `agent_task`) already exist; Phase 4 adds the query helpers the executor needs:

- `start_run`, `update_run_status`, `advance_current_nodes`.
- `start_step`, `complete_step`, `fail_step`.
- `list_runs_for_workflow`, `list_active_runs`.

### `actant-flow`

The executor (Phase 1 shipped only types + traits). Implement every node type:

- `agent_task` — assign to actor, await result.
- `model_call` — enqueue `model.call` effect via `actant-effects`; advance on result.
- `tool_call` — same with `tool.call`.
- `approval_gate` — create `approval_request`; advance on approve/deny.
- `human_task` — surface `agent_task` row; advance on user-marked-done.
- `condition` — evaluate against run-local state.
- `parallel_group` — fan out, barrier on join.
- `memory_write` — issue `propose_memory` (or `approve_memory` if pre-authorized).
- `file_operation` — `tool_call` specialized to file effect type.
- `browser_action` — `tool_call` specialized to `browser.act`.
- `delay` — scheduled wakeup.
- `subworkflow` — start a child `workflow_run`, advance on terminate.
- `external_webhook` — outbound HTTP + inbound callback wait (logic here; route in `actant-server`).

Add retry / timeout enforcement using per-node policies. State machine survives process restarts (recover from `workflow_run.status='waiting_*'`).

### DSL parser

Phase 4 chooses YAML+DSL (see `/planning/phase-4-plan.md` §"Workflow definition format"). Parser is part of `actant-flow` and writes `workflow_node` + `workflow_edge` rows inside the `create_workflow` transaction.

### `actant-command`

Workflow commands: `create_workflow`, `retire_workflow`, `start_workflow_run`, `complete_workflow_step`, `cancel_workflow_run`. Regret commands: `file_regret`, `acknowledge_regret`, `resolve_regret`. Eval commands: `create_eval_from_replay`, `run_eval`, `enable_eval`, `disable_eval`. Model-route decision is created implicitly inside `request_model_call`.

### `actant-policy`

Workflow-run authority: per-workflow + per-trigger scopes; budget enforcement at run start; `workflow.run` permission.

### `actant-effects`

Add `workflow.dispatch` effect type wired through.

### `actant-subscribe`

New targets: `workflow_run`, `workflow_step_run`, `agent_task`, `regret_event`, `eval_case`, `eval_run`, `model_route_decision`.

### `actant-server`

`POST /v1/webhooks/{trigger_id}` endpoint (with HMAC verification). Workflow CRUD endpoints (`POST /v1/workflows`, etc.) where appropriate.

## Acceptance criteria

- [ ] Daily-digest workflow from `/specs/10-alpha-demo.md` §14 runs unattended for one calendar week.
- [ ] A paused workflow run survives a 10-minute process gap.
- [ ] Model-based tests of every node-type transition pass.
- [ ] `resolve_regret` without a concrete corrective action reference is rejected with `precondition_failed`.

## Do NOT

- Do NOT couple workflow execution to a single process. The executor must recover from restart.
- Do NOT bypass the effect queue for any node that produces a side effect.
- Do NOT introduce a generic workflow-step-effect type. Each node type has its mapping.
