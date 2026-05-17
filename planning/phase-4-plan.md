# Phase 4 plan — Flow Engine + triggers + regret/eval loop

## Goal

Make ActantDB durable. Workflows survive process restarts, span hours or days, gate on humans, and produce the inputs the regret/eval loop needs to convert failures into evals.

When Phase 4 ends, the second demo from `/specs/10-alpha-demo.md` §14 (daily digest) runs unattended against a real inbox + calendar for a week.

## Duration

4–6 weeks.

## New crates introduced

| Crate              | Kind | Purpose                                                       |
| ------------------ | ---- | ------------------------------------------------------------- |
| `actant-trigger`   | lib  | Cron, event-filter, webhook, and manual trigger runtimes. Wakes workflows. |
| `actant-eval`      | lib  | Eval-case runtime. Reads `eval_case`, runs a replay under the eval's constraints, writes `eval_run` rows. |

`actant-regret` is a module inside `actant-command` (regret commands are first-class commands, but they don't justify a new crate).

## Existing crates expanded

| Crate              | Phase 4 expansion                                                          |
| ------------------ | -------------------------------------------------------------------------- |
| `actant-flow`      | The executor. Phase 1 shipped types and traits; Phase 4 makes it run. All 13 node types from `/specs/07-workflows-and-replay.md` §2 (Phase 1 list) plus `external_webhook` if Phase 4 ships it. Retry, timeout, approval-gate integration. State-machine durability across restarts. |
| `actant-policy`    | `workflow.run` permission; per-workflow budget enforcement; per-trigger authority. |
| `actant-command`   | Workflow commands: `create_workflow`, `retire_workflow`, `start_workflow_run`, `complete_workflow_step`, `cancel_workflow_run`. Regret commands: `file_regret`, `acknowledge_regret`, `resolve_regret`. Eval commands: `create_eval_from_replay`, `run_eval`, `enable_eval`, `disable_eval`. |
| `actant-storage`   | Row mappers for `workflow`, `workflow_node`, `workflow_edge`, `workflow_run`, `workflow_step_run`, `trigger`, `agent_task`, `regret_event`, `eval_case`, `eval_run`, `model_route_decision`. |
| `actant-effects`   | Workflow-dispatch effect type wired through. |
| `actant-subscribe` | New targets: `workflow_run`, `workflow_step_run`, `agent_task`, `regret_event`, `eval_case`, `eval_run`. |
| `actant-server`    | Webhook endpoint (`POST /v1/webhooks/{trigger_id}`). Workflow-DSL parsing on `create_workflow`. |

## Specs landing in Phase 4

From `/specs/14-extended-primitives.md` §17:

- Regret event + eval case
- Model route decision (when real routing with multiple cloud routes exists)
- Context debt scoring (refined now that real prompts are in flight)

Plus Phase 4 scope from `/specs/11-roadmap.md`:

- Workflow definition format (YAML+DSL chosen here; ADR-0008 captures the decision)
- All Phase 1 node types (`agent_task`, `model_call`, `tool_call`, `approval_gate`, `human_task`, `condition`, `parallel_group`, `memory_write`, `file_operation`, `delay`, `subworkflow`)
- `browser_action` and `external_webhook` may slip to Phase 6
- Retry / timeout / approval gates fully wired
- Trigger engine (cron + event + webhook + manual)

## Workflow definition format (ADR-0008)

ADR-0008 will pick YAML+DSL. Phase 4 ships:

```yaml
name: daily_digest
version: 1
trigger:
  kind: cron
  expr: "0 7 * * *"
policy_ref: policy_digest_v1

nodes:
  fetch_inbox:
    type: tool_call
    config: { tool: gmail.list_unread, args: { max: 50 } }
    required_permissions: [gmail.read]
    timeout: 60s

  summarize:
    type: model_call
    config: { route: route_planner, purpose: planner }
    required_permissions: [model.call:route_planner]

  approval:
    type: approval_gate
    config: { risk: medium, summary_from: summarize.output }

  send:
    type: tool_call
    config: { tool: message.send, args_from: summarize.output }
    required_permissions: [message.send:user]

edges:
  - { from: fetch_inbox, to: summarize }
  - { from: summarize, to: approval }
  - { from: approval, to: send, condition: approved }
```

The parser produces `workflow`, `workflow_node`, `workflow_edge` rows inside the `create_workflow` transaction.

## Studio additions (Phase 4)

- **Workflow Board** — runs grouped by workflow, current node(s), pending approvals.
- **Workflow Run Timeline** — every step with start/end, status, output preview.
- **Trigger Manager** — cron schedules, webhook URLs, enable/disable toggles.
- **Regret Inbox** — filed regrets with status, suggested corrective action, resolution link.
- **Eval Catalog** — eval cases with pass/fail history; "Run all" button.

## Test strategy

- **State-machine model tests.** Every node-type transition exercised by a model-based test generator.
- **Durability under restart.** A workflow run that is `waiting_human` survives a server restart; the next event correctly resumes the run.
- **Trigger semantics.** Cron precision (±1 minute), event-trigger filter correctness, webhook authentication.
- **Retry policy.** A node with `retry_policy=exponential(max=5)` actually retries 5 times before failing.
- **Regret-resolution constraint.** A `resolve_regret` without a concrete corrective action reference is rejected with `precondition_failed`.
- **Eval drift.** A checkpoint that no longer resolves (snapshots purged) auto-disables its eval cases.

## Decision gate

Phase 4 passes when:

1. The daily-digest workflow runs unattended for one calendar week against a test inbox + calendar. Daily summaries are produced; the approval gate pauses; on approval the message sends.
2. A workflow with a pending approval survives a process restart and a 10-minute gap before the human approves.
3. A scripted regret round-trip: file regret → acknowledge → resolve with a new policy row → eval case minted from a replay → eval runs and passes.

## Risks

| Risk                                          | Mitigation                                                                  |
| --------------------------------------------- | --------------------------------------------------------------------------- |
| State-machine bugs                            | Model-based tests of every transition. Property tests of `current_node_ids` consistency. |
| Workflow DSL design churn                     | ADR-0008 locks the format on entry to Phase 4. Schema changes require migration + DSL versioning. |
| Eval-case orphaning                           | `eval_case_orphaned` auto-disables; Studio surfaces orphans for re-anchoring. |
| Regret gaming                                 | Resolution must link to a concrete change. Studio shows the chain for audit. |
| Trigger reliability                           | Cron precision relies on a wallclock; a separate scheduler crate may emerge in Phase 6 for high-precision use. |
| Multi-tenancy not yet here                    | Phase 4 still single-workspace per process. Phase 6 fans out. |

## CLI deliverables (Phase 4)

Per `/planning/cli-design.md` § "CLI staging across phases":

- New subcommands: `actant workflow create|run|watch|show|list`, `actant eval list|run`, `actant policy list|show|grant|revoke|test`, `actant generate workflow`, `actant agent run|list` with workflow support, `actant regret list|show|file|resolve`.
- New template: `multi-agent-board`.
- New examples: `workflow-dag`.

## Work packages

- `/agents/actant-trigger.md`
- `/agents/actant-eval.md`
- `/agents/phase-4-extensions.md` (also extends `actant-cli` and `actant-codegen-project` for the workflow generator)

## Sequencing

```
week 1
  ├── ADR-0008 (workflow DSL)
  ├── actant-flow executor: state machine + node dispatch
  └── actant-storage row mappers

week 2
  ├── actant-trigger: cron + manual
  ├── actant-flow: retry / timeout / approval_gate
  └── DSL parser

week 3
  ├── actant-trigger: event + webhook
  ├── actant-eval runtime
  └── actant-command: workflow / regret / eval commands

week 4
  ├── Studio Phase 4 screens
  ├── End-to-end daily-digest demo
  └── (week 5–6 if needed for stabilization)
```
