# 07 — Workflows and Replay

This document specifies the **Flow Engine** (durable workflows) and the **Replay Engine** (checkpoints and reruns). They share infrastructure: workflows are heavy users of Chronicle, effects, and approvals, and replay needs to be able to reconstruct a workflow's exact state at any prior event.

Sections:

1. Workflow data model (recap)
2. Node types and semantics
3. Workflow run lifecycle
4. Triggers
5. Determinism and idempotency in workflows
6. Replay targets and modes
7. Checkpoint contents
8. Running a replay
9. Replay diffs
10. Worked replay examples

---

## 1. Workflow data model (recap)

From `02-data-model.sql`:

```
workflow             (id, name, version, status, policy_id, definition_ref, ...)
workflow_node        (workflow_id, node_key, node_type, config_ref,
                      required_permissions, retry_policy, timeout_policy)
workflow_edge        (workflow_id, from_node_id, to_node_id, condition_ref, order_index)
workflow_run         (workflow_id, status, current_node_ids, trigger_event_id, ...)
workflow_step_run    (workflow_run_id, node_id, status, effect_id,
                      approval_request_id, output_ref, ...)
trigger              (workflow_id, kind, config_ref, enabled)
```

A workflow definition is a versioned artifact (`definition_ref`). When `create_workflow` commits, the parser reads the definition and writes the nodes and edges. Once written, the workflow shape is immutable for that version; a new version creates a new `workflow` row.

---

## 2. Node types and semantics

| `node_type`         | What it does                                                                                       |
| ------------------- | -------------------------------------------------------------------------------------------------- |
| `agent_task`        | Hand off to an agent actor. Step is `running` until the agent submits a result.                    |
| `model_call`        | Enqueue a `model.call` effect; advance on `model_call_finished`.                                   |
| `tool_call`         | Enqueue a `tool.call` effect (subject to approval); advance on `tool_call_finished`.               |
| `approval_gate`     | Create an `approval_request` with no effect attached; advance on approve/deny.                     |
| `human_task`        | Surface a task on the agent task board for a named human actor; advance when the human marks done. |
| `condition`         | Evaluate `condition_ref` against the run's local state; pick outgoing edge by truth value.         |
| `parallel_group`    | Fan out to all outgoing edges; barrier on a `parallel_join` sibling node.                          |
| `memory_write`      | Issue a `propose_memory` (or `approve_memory` if pre-authorized) inside the run.                   |
| `file_operation`    | Specialized form of `tool_call` for file IO; uses `file.read` / `file.write` effect types.         |
| `browser_action`    | Specialized `tool_call` for browser; uses `browser.act` effect type.                               |
| `external_webhook`  | Issue an outbound HTTP request and wait for inbound callback; advance on callback.                 |
| `delay`             | Sleep until `delay.until` (RFC3339); implemented as a scheduled wakeup, not a busy wait.            |
| `subworkflow`       | Start a child `workflow_run`; advance when the child terminates.                                   |

**Edges.**

- Default edges have no condition and are taken in `order_index` order. Multiple unconditional outgoing edges from a non-`parallel_group` node is a definition error.
- Conditional edges have a `condition_ref` evaluated against the run's local state (the union of all completed step outputs).
- A node with zero outgoing edges is a terminal node; reaching it completes the run if no other nodes are still in flight.

---

## 3. Workflow run lifecycle

```
created
   │
   ▼
running ───────────────┬───────────────┬───────────────┐
   │                   │               │               │
   │ enqueues effect   │ awaits human  │ awaits child  │ awaits trigger callback
   ▼                   ▼               ▼               ▼
waiting_effect    waiting_human   waiting_subworkflow  waiting_webhook
   │                   │               │               │
   └────── effect ─────┴────  done  ───┴───  done  ────┘
   │                                                   │
   ▼                                                   │
running ◄──────────────────────────────────────────────┘
   │
   ▼
completed | failed | cancelled
```

`current_node_ids` is the list of nodes presently advancing. For a serial workflow it has one entry; for a parallel run it has multiple.

**State transitions are commands.** `complete_workflow_step`, `cancel_workflow_run`, `start_workflow_run` are the only writes. Direct projection mutation is forbidden (invariant 1 in `05-security-model.md`).

**Resumability.** A run that is `waiting_*` survives process restarts. On startup, Flow Engine re-reads `workflow_run` rows in `running` / `waiting_*` and resumes by listening for the events that would advance them.

**Failure handling.** A failed step transitions the step to `failed`; the workflow's `retry_policy` decides whether to retry the node, fall through to an error edge, or mark the run `failed`.

---

## 4. Triggers

| `trigger.kind`  | Behavior                                                                                  |
| --------------- | ----------------------------------------------------------------------------------------- |
| `cron`          | The system schedules `start_workflow_run` at the configured cadence.                      |
| `event`         | A standing subscription to `agent_event` with a filter; matching events trigger a run.    |
| `webhook`       | An inbound HTTP endpoint mounted by the server triggers a run with the body as input.     |
| `manual`        | Run only starts via explicit `start_workflow_run`.                                        |

Triggers carry their own `enabled` flag; disabling a trigger does not affect runs already in flight.

---

## 5. Determinism and idempotency in workflows

Workflows are not pure functions of their inputs — they call models, tools, and humans. So "deterministic replay" is impossible in general. What ActantDB guarantees instead:

1. **Step boundaries are deterministic.** Given the same `workflow_run.id` and the same prior `workflow_step_run` outputs, the engine will choose the same next node(s) by following the same edges. The only source of non-determinism inside the engine is `condition_ref` evaluation, which is over recorded state.
2. **Effect outputs are reusable.** A replay (mode `recorded`) re-uses stored `effect_result` rows by `effect_id`. The model is never re-called.
3. **Approval decisions are deterministic.** An `approval_request` is replayed by reading the recorded decision; replays do not re-prompt humans.
4. **Time can be virtualized.** A `delay` node is short-circuited in replay; it advances immediately.

For mode `experimental` (where models/tools are re-invoked), no determinism is promised. The replay produces a `replay_diff` that surfaces the divergences.

---

## 6. Replay targets and modes

A replay is defined by `(target, mode, overrides)`.

**Targets** — what is being replayed:

| Target                      | Scope                                                |
| --------------------------- | ---------------------------------------------------- |
| `session`                   | A `session_id` from a chosen `event_id`.             |
| `workflow_run`              | A `workflow_run_id` from a chosen step.              |
| `model_call`                | A single `model_call_id`.                            |
| `tool_call_sequence`        | A contiguous run of `tool_call`s in a session.       |
| `memory_decision`           | A `memory_candidate` review decision.                |
| `agent_task`                | A `agent_task` lifecycle.                            |
| `approval_path`             | A chain of approval requests for a parent event.     |
| `context_build`             | A single `context_build_id`.                         |

**Modes** — how strict the replay is:

| Mode            | Re-invokes models? | Re-invokes tools? | Asks humans? | Uses original policy? |
| --------------- | ------------------ | ----------------- | ------------ | --------------------- |
| `recorded`      | no (reuses output) | no (reuses output)| no           | yes                   |
| `experimental`  | yes                | yes               | no (uses recorded decisions; new approvals auto-approved per overrides) | yes |
| `policy`        | no                 | no                | no           | no — uses `overrides.policy_id` |
| `model`         | yes (override)     | no                | no           | yes                   |
| `memory`        | no                 | no                | no           | yes (with edited memory set) |
| `tool`          | no                 | yes (override)    | no           | yes                   |
| `local_only`    | yes (local routes) | yes (local-only)  | no           | yes                   |

**Overrides.** `replay_run.overrides_ref` is an artifact whose structure depends on the mode:

```jsonc
// model override
{ "model_route_id": "route_qwen_coder_local" }

// memory override
{ "exclude_memory_ids": ["mem_1", "mem_7"], "edit": { "mem_2": "new text" } }

// policy override
{ "policy_id": "policy_strict_no_cloud" }

// tool override
{ "mock_tools": { "shell.run": "noop" } }
```

---

## 7. Checkpoint contents

A `replay_checkpoint` is *sufficient* for replay iff it captures everything needed to reconstruct the state. The schema columns:

```
event_id                     anchor in agent_event
state_snapshot_ref           artifact: serialized projection state up to event_id
model_route_snapshot_ref     artifact: model_route + model_provider rows at the time
permission_snapshot_ref      artifact: authority_scope + policy rows at the time
memory_snapshot_ref          artifact: memory rows (text, embedding_ref, sensitivity, visibility, scope)
session_id / workflow_run_id / context_build_id (optional anchors)
```

**When checkpoints are created.**

- Automatic: every N events per session/run (configurable; default 100), and at every `model_call_requested`, `tool_call_approved`, `workflow_started`, `workflow_step_completed`.
- Manual: `create_replay_checkpoint` command.
- Pre-replay: when a `start_replay_run` references a non-checkpointed event, the engine first creates a checkpoint at that event.

**Why these four snapshots.**

| Snapshot           | Replay mode it enables                                  |
| ------------------ | ------------------------------------------------------- |
| State              | All modes — reconstructs the world up to the anchor.    |
| Model route        | `model`, `local_only` — know the original route to diff against. |
| Permission         | `policy` — compare original vs override policy.         |
| Memory             | `memory` — know what memories existed and their visibility at the time. |

A checkpoint missing any of these fails `start_replay_run` with `precondition_failed`.

---

## 8. Running a replay

```
start_replay_run(checkpoint_id, mode, overrides_ref)
   │
   ▼
Replay Engine:
   load state_snapshot              → in-memory replica
   load model/permission/memory snapshots
   begin replay event loop:
     for each agent_event after checkpoint.event_id (up to optional stop_at):
        translate the event into a replayed command in the replay scope:
          mode=recorded:
            for effect-bound events, reuse effect_result by effect_id
            for model_call_finished, reuse response_ref
            for tool_call_finished, reuse result_ref
            for approval decisions, reuse the decision
          mode=experimental:
            re-enqueue effects with new effect ids in the replay scope
            wait for replay workers to complete them
            (replay workers are configured per mode; they may be sandboxed mocks)
          mode=policy:
            re-evaluate Guard with overrides.policy_id
            if decision differs, branch:
              if original was allow and override is deny  → record diff, halt branch
              if original was deny and override is allow  → execute via recorded result if available
        record a synthetic replay event linked to original_event_id
        write a replay_diff row capturing the comparison
end of stream:
   replay_run.status = completed
   summary_ref artifact contains the rolled-up diff
```

Replays never write to non-replay projection rows or to the real Chronicle; they have their own namespace inside `replay_run` and `replay_diff`. (Phase 0 leaves the precise storage of replay-scoped synthetic state to Phase 5; Phase 1 keeps it in artifacts.)

---

## 9. Replay diffs

A `replay_diff` row captures one comparison point. `kind`:

| Kind        | Meaning                                                      |
| ----------- | ------------------------------------------------------------ |
| `identical` | Synthetic event matches the original byte-for-byte.          |
| `changed`   | Same `event_type` but different payload (e.g. different model output, different tool arguments). |
| `missing`   | Original existed; replay did not produce it (e.g. policy denial pruned a branch). |
| `extra`     | Replay produced an event the original did not.               |

The `summary_ref` artifact on `replay_run` aggregates diffs into the human-facing categories from the README:

```
original succeeded, replay failed
tool arguments changed
different memory was selected
context omitted important file
approval path differed
cloud model used sensitive memory
latency/cost changed
```

---

## 10. Worked replay examples

### 10.1 `mode=memory` — "what if I delete this memory?"

A coding agent's recent run made an incorrect assumption based on a stale memory (`mem_42`: "this repo uses Jest"). The repo actually uses Vitest.

```
start_replay_run({
  checkpoint_id: "chk_99",
  mode: "memory",
  overrides_ref: art_({ exclude_memory_ids: ["mem_42"] })
})
```

Replay reruns the context build phase with `mem_42` excluded; the model call uses recorded outputs because mode is `memory`, but the *context manifest* changes. The diff highlights that without `mem_42` the planner would have proposed a different test command.

The user, satisfied, runs `revoke_memory({ memory_id: "mem_42" })` and re-runs the original task fresh.

### 10.2 `mode=policy` — "what if cloud models were forbidden?"

An auditor wants to know whether the workspace would have completed today's tasks under a stricter policy that forbids cloud routes.

```
start_replay_run({
  checkpoint_id: "chk_morning",
  mode: "policy",
  overrides_ref: art_({ policy_id: "policy_no_cloud" })
})
```

The replay re-evaluates Guard on every model/tool decision. Where the original used a cloud route, the override denies; replay records `deny` and halts the branch with a diff entry. The summary lists the tasks that would have failed and why.

### 10.3 `mode=experimental` — "would a smaller model have done as well?"

A developer wants to compare gpt-4 vs qwen-coder on a yesterday's coding task.

```
start_replay_run({
  checkpoint_id: "chk_task_start",
  mode: "experimental",
  overrides_ref: art_({ model_route_id: "route_qwen_coder_local" })
})
```

Replay re-invokes the model worker with the alternate route, re-runs the tools (which read the same files) inside a sandboxed workspace, and compares outputs. The diff shows whether tests would still have passed.

---

## Verification

- [ ] Every node type listed maps to an effect type or a Chronicle event sequence that exists in this spec set.
- [ ] Every replay mode is satisfiable by the four snapshot refs on `replay_checkpoint`.
- [ ] Every diff kind in §9 can be produced by the loop in §8.
- [ ] No workflow command directly performs I/O — every external action is mediated by the Effect Engine (consistent with invariants in `05-security-model.md`).
- [ ] Replays do not write to non-replay projection rows or to the main Chronicle.
