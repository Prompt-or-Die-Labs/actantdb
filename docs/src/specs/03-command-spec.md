# 03 — Command Spec

This is the complete typed mutation surface of ActantDB. Every command:

1. is **authenticated** to an actor and a workspace,
2. is **validated** against its declared input schema,
3. is **checked** by Guard against the actor's authority,
4. **commits** in a single transaction that writes `command_record`, projection rows, and `agent_event` rows, and optionally enqueues `effect` and `approval_request` rows,
5. **emits** at least one event in the Chronicle.

Commands return either `{ status: "committed", events: [...] }` or `{ status: "rejected", error: { code, message } }`.

Throughout this file, `actor_id` and `workspace_id` are required on every command and are not repeated in each input shape.

## Command catalog (alphabetical)

```
append_agent_message            grant_permission
append_user_message             propose_memory
approve_effect_for_scope        record_memory_use
approve_effect_for_session      record_model_result
approve_effect_once             record_tool_result
approve_memory                  register_tool
approve_tool_call               register_worker
build_context                   reject_memory
cancel_workflow_run             request_model_call
close_session                   request_tool_call
complete_effect                 restrict_memory
complete_workflow_step          retire_workflow
create_replay_checkpoint        retract_command  (system-only)
create_session                  revoke_memory
create_workflow                 revoke_permission
delete_memory                   start_replay_run
deny_effect                     start_workflow_run
deny_tool_call                  unregister_worker
edit_memory                     update_session_title
enqueue_effect                  worker_heartbeat
expire_approval                 worker_status
expire_memory
escalate_approval
```

The rest of this document specifies each command.

---

## Sessions

### `create_session`

Create a conversation session bound to an agent.

```jsonc
{
  "command": "create_session",
  "input": {
    "agent_actor_id": "agent_123",
    "title": "Fix failing tests",        // optional
    "initial_user_message": "Hello"      // optional shortcut
  }
}
```

**Preconditions.** Caller has `session.create` permission for the workspace and for `agent_actor_id`.

**Writes.** `session`. If `initial_user_message` is set, also `message`.

**Events.** `session_created`. If a message was inlined: `user_message_received`.

### `append_user_message`

```jsonc
{
  "command": "append_user_message",
  "input": {
    "session_id": "sess_123",
    "text": "Hello",
    "body_ref": null                    // optional; for large content
  }
}
```

**Preconditions.** Session is `active`. Caller has `session.write` for `session_id`.

**Writes.** `message`.

**Events.** `user_message_received`.

### `append_agent_message`

Same shape as `append_user_message` but role is `agent` and caller must be an `agent` actor.

**Events.** `agent_message_sent`.

### `update_session_title` / `close_session`

Standard small mutations. `close_session` requires `session.close`.

**Events.** `session_title_updated`, `session_closed`.

---

## Context

### `build_context`

Build a context manifest for an upcoming model call.

```jsonc
{
  "command": "build_context",
  "input": {
    "session_id": "sess_123",
    "purpose": "executor",
    "model_route_id": "route_planner",
    "token_budget": 8000,
    "candidate_filters": {                // optional, narrows gathering
      "include_memories": true,
      "include_messages_window": 50,
      "include_artifacts": ["art_..."]
    }
  }
}
```

**Preconditions.** Caller is the session's `agent_actor_id` or has `context.build` on the workspace.

**Behavior.** The Context Engine assembles candidates, scores them, filters by Guard against `model_route.visibility_required`, redacts, truncates to `token_budget`. See `06-context-and-memory.md`.

**Writes.** `context_build`, N × `context_item`.

**Events.** `context_build_started` (before scoring), `context_build_finished` (on commit).

---

## Model calls

### `request_model_call`

```jsonc
{
  "command": "request_model_call",
  "input": {
    "session_id": "sess_123",
    "context_build_id": "ctx_456",
    "route_id": "route_planner",
    "purpose": "planner"
  }
}
```

**Preconditions.** `context_build_id` matches a successful build whose visibility matches `route_id`. Caller has `model.call` for `route_id`.

**Writes.** `model_call` (status=`requested`), `effect` (type=`model.call`).

**Events.** `model_call_requested`.

### `record_model_result`

Called by the model worker through `complete_effect` (which forwards to this command). Direct callers are workers only.

```jsonc
{
  "command": "record_model_result",
  "input": {
    "model_call_id": "mc_789",
    "status": "completed",            // or 'failed'
    "response_ref": "art_...",
    "input_tokens": 1234,
    "output_tokens": 567,
    "cost_usd": 0.0123,
    "latency_ms": 2340,
    "error": null
  }
}
```

**Writes.** `model_call` (status, response_ref, metrics).

**Events.** `model_call_finished`.

---

## Tool calls

### `register_tool`

```jsonc
{
  "command": "register_tool",
  "input": {
    "name": "shell.run",
    "kind": "shell",
    "required_permission": "shell.run",
    "default_risk_level": "high",
    "input_schema_ref": "art_..."
  }
}
```

**Writes.** `tool`, `tool_schema_version`.

**Events.** `tool_registered`.

### `request_tool_call`

```jsonc
{
  "command": "request_tool_call",
  "input": {
    "session_id": "sess_123",
    "tool_name": "shell.run",
    "arguments": { "command": "pytest" },
    "produced_by_model_call_id": "mc_789"   // optional
  }
}
```

**Preconditions.** `tool_name` exists. Caller has `tool.call:<tool_name>` or Guard returns `allow_with_approval` (which produces an approval request rather than rejection).

**Writes.** `tool_call` (status depends on Guard's decision):

| Guard decision        | tool_call.status      | also created                                  |
| --------------------- | --------------------- | --------------------------------------------- |
| `allow`               | `approved`            | `effect` (type=`tool.call`)                   |
| `allow_with_approval` | `pending_approval`    | `approval_request`                            |
| `deny`                | `denied`              | (no effect; command_record records denial)    |

**Events.** `tool_call_requested`, and one of `tool_call_approved`, `tool_call_pending_approval`, or `tool_call_denied`.

### `approve_tool_call` / `deny_tool_call`

```jsonc
{
  "command": "approve_tool_call",
  "input": {
    "tool_call_id": "tc_456",
    "scope": "once"                  // 'once'|'session'|'scope'|'forever'
  }
}
```

**Preconditions.** Caller is an authorized approver for the tool/risk level. The associated `approval_request` is `pending`.

**Writes.** `approval_request` (status), `tool_call` (status), `effect` (created if approving), `authority_scope` (created if `scope != 'once'`).

**Events.** `tool_call_approved` or `tool_call_denied`.

### `record_tool_result`

Same pattern as `record_model_result`: invoked indirectly by `complete_effect` for a tool worker.

```jsonc
{
  "command": "record_tool_result",
  "input": {
    "tool_call_id": "tc_456",
    "status": "completed",          // or 'failed'
    "result_ref": "art_...",
    "error": null
  }
}
```

**Events.** `tool_call_finished`.

---

## Effects (low-level)

### `enqueue_effect`

Used by Flow Engine and other internal callers when an effect is needed without going through a higher-level command. Most callers should use `request_tool_call` / `request_model_call` instead.

```jsonc
{
  "command": "enqueue_effect",
  "input": {
    "effect_type": "http.request",
    "input_ref": "art_...",
    "idempotency_key": "abc-123",
    "required_permission": "http.request",
    "risk_level": "low"
  }
}
```

**Writes.** `effect`.

**Events.** `effect_enqueued`.

### `complete_effect`

Worker-only command. Reports the outcome of an effect and triggers the appropriate downstream record (`record_model_result`, `record_tool_result`, etc.) inside the same transaction.

```jsonc
{
  "command": "complete_effect",
  "input": {
    "effect_id": "eff_111",
    "succeeded": true,
    "result_ref": "art_...",
    "error": null
  }
}
```

**Writes.** `effect`, `effect_result`, and whichever downstream projection the effect_type drives.

**Events.** `effect_completed` (plus the downstream event from the chained command, e.g. `tool_call_finished`).

### `approve_effect_once` / `approve_effect_for_session` / `approve_effect_for_scope` / `deny_effect` / `expire_approval` / `escalate_approval`

Generic forms of the tool-call approval commands, used when the requesting subsystem is not a tool call (e.g. an unattended workflow needs human approval). Inputs differ only in the granted `scope`.

**Events.** `effect_approved`, `effect_denied`, `approval_expired`, `approval_escalated`.

---

## Permissions

### `grant_permission`

```jsonc
{
  "command": "grant_permission",
  "input": {
    "target_actor_id": "agent_123",
    "permission": "file.read",
    "resource_pattern": "~/Projects/Swoosh/**",
    "sensitivity_ceiling": "medium",
    "allowed_actions": ["read"],
    "expires_at": "2026-12-31T23:59:59Z"
  }
}
```

**Preconditions.** Caller has `permission.grant` and at least the same scope.

**Writes.** `authority_scope`.

**Events.** `permission_granted`.

### `revoke_permission`

```jsonc
{
  "command": "revoke_permission",
  "input": { "scope_id": "auth_999" }
}
```

**Writes.** `authority_scope.revoked_at`.

**Events.** `permission_revoked`.

---

## Memory

### `propose_memory`

```jsonc
{
  "command": "propose_memory",
  "input": {
    "text": "User prefers Python tests with pytest, not unittest.",
    "category": "preference",
    "confidence": 0.83,
    "sensitivity": "low",
    "source_event_ids": ["evt_1", "evt_2"]
  }
}
```

**Writes.** `memory_candidate`.

**Events.** `memory_candidate_created`.

### `approve_memory` / `reject_memory` / `edit_memory`

```jsonc
{
  "command": "approve_memory",
  "input": {
    "candidate_id": "mc_1",
    "scope": "global",
    "expires_at": null
  }
}
```

`edit_memory` allows the reviewer to alter `text`, `category`, `sensitivity` before approval. `reject_memory` requires a reason.

**Writes.** `memory_candidate.status`, `memory` (on approval).

**Events.** `memory_approved`, `memory_rejected`, `memory_edited`.

### `record_memory_use`

```jsonc
{
  "command": "record_memory_use",
  "input": {
    "memory_id": "mem_1",
    "context_build_id": "ctx_456",
    "model_call_id": "mc_789",
    "outcome": "used"
  }
}
```

**Writes.** `memory_use`, increments `memory.usage_count`, sets `memory.last_used_at`.

**Events.** `memory_used`.

### `restrict_memory` / `expire_memory` / `revoke_memory` / `delete_memory`

| Command          | Effect                                                                |
| ---------------- | --------------------------------------------------------------------- |
| `restrict_memory`| Adds a visibility constraint (e.g. `never_model: cloud_*`).           |
| `expire_memory`  | Sets `expires_at`; future context builds will not include it.         |
| `revoke_memory`  | Sets `revoked_at`; immediate exclusion from all context.              |
| `delete_memory`  | Cryptographic erasure: clears `text`, deletes embedding, keeps row.   |

**Events.** `memory_restricted`, `memory_expired`, `memory_revoked`, `memory_deleted`.

---

## Workflows

### `create_workflow`

```jsonc
{
  "command": "create_workflow",
  "input": {
    "name": "daily_digest",
    "version": 1,
    "definition_ref": "art_..."
  }
}
```

**Writes.** `workflow`, `workflow_node` (N), `workflow_edge` (M). Phase 0 specifies that the parsing of `definition_ref` into nodes/edges happens inside the command.

**Events.** `workflow_created`.

### `retire_workflow`

**Events.** `workflow_retired`.

### `start_workflow_run`

```jsonc
{
  "command": "start_workflow_run",
  "input": {
    "workflow_id": "wf_1",
    "trigger_event_id": null,
    "input_ref": "art_..."
  }
}
```

**Writes.** `workflow_run` (status=`running`), `workflow_step_run` rows for entry nodes.

**Events.** `workflow_started`.

### `complete_workflow_step`

Called by Flow Engine when a step's effect finishes.

**Writes.** `workflow_step_run`, advances `workflow_run.current_node_ids`, possibly enqueues effects for next nodes.

**Events.** `workflow_step_completed`, and if the run terminates, `workflow_completed` or `workflow_failed`.

### `cancel_workflow_run`

**Events.** `workflow_cancelled`.

---

## Workers

### `register_worker`

```jsonc
{
  "command": "register_worker",
  "input": {
    "name": "shell-worker-01",
    "capabilities": ["shell.run", "file.read", "file.write"],
    "host": "laptop.local",
    "version": "0.1.0"
  }
}
```

**Writes.** `worker`, `worker_capability` (N).

**Events.** `worker_registered`.

### `unregister_worker` / `worker_status`

Mark a worker `draining` or `offline`. Active claims must finish or expire first.

**Events.** `worker_status_changed`.

### `worker_heartbeat`

```jsonc
{
  "command": "worker_heartbeat",
  "input": {
    "in_flight_count": 2,
    "cpu_pct": 12.4,
    "mem_mb": 412.0
  }
}
```

**Writes.** `worker_heartbeat`, updates `worker.last_heartbeat_at`.

**Events.** none — heartbeats are too noisy for Chronicle; they live only in the heartbeat table for the worker monitor dashboard.

---

## Replay

### `create_replay_checkpoint`

```jsonc
{
  "command": "create_replay_checkpoint",
  "input": {
    "event_id": "evt_999",
    "session_id": "sess_123",
    "workflow_run_id": null,
    "context_build_id": "ctx_456"
  }
}
```

The system also creates checkpoints automatically at periodic intervals and at decision points (see `07-workflows-and-replay.md`).

**Writes.** `replay_checkpoint` plus snapshot artifacts in the artifact store.

**Events.** `replay_checkpoint_created`.

### `start_replay_run`

```jsonc
{
  "command": "start_replay_run",
  "input": {
    "checkpoint_id": "chk_1",
    "mode": "experimental",
    "overrides_ref": "art_..."        // model_route, tool policy, memory edits
  }
}
```

**Writes.** `replay_run` (status=`pending`); the Replay Engine will pick it up and start emitting synthetic events scoped to the replay.

**Events.** `replay_started`.

---

## System-only

### `retract_command`

Used to surgically retract a command that committed by mistake. Does not delete the original `command_record`; appends a compensating record and tombstone events. Only callable by `system` actors with explicit human approval.

**Events.** `command_retracted`.

---

## Standard errors

| Code                       | Meaning                                                  |
| -------------------------- | -------------------------------------------------------- |
| `unauthenticated`          | No valid actor identity attached to the request.         |
| `forbidden`                | Authority check failed; details in `decision_reason`.    |
| `invalid_input`            | Schema validation failed; field-level errors returned.   |
| `precondition_failed`      | A referenced row does not exist or is in a bad state.    |
| `conflict`                 | Idempotency or version conflict.                         |
| `not_found`                | Referenced entity not found.                             |
| `policy_blocked`           | Guard returned `deny` with an explicit policy reason.    |
| `approval_required`        | Returned from query helpers; commands never return this — they create an approval_request instead. |
| `internal_error`           | Bug. Recorded in `command_record.error`.                 |

---

## Verification

- [ ] Every command writes to tables that exist in `02-data-model.sql`.
- [ ] Every event referenced here appears in the Chronicle event list in `01-architecture.md`.
- [ ] Every command that may produce a side effect uses the Effect Engine — no command directly performs I/O.
- [ ] Every command that touches memory respects the lifecycle described in `06-context-and-memory.md`.
- [ ] Every approval-producing command sets a `scope_granted` consistent with `05-security-model.md`.
