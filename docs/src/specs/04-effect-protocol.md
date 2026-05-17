# 04 — Effect Protocol

The Effect Engine moves side effects out of the database transaction. Every real-world change — calling a model, running a shell command, clicking a browser, writing a file, sending an HTTP request, drafting an email — is scheduled as an **effect row**, claimed by a **worker**, executed outside the database, and re-recorded as **events**.

This document specifies:

1. the effect lifecycle
2. the worker protocol
3. idempotency semantics
4. retry and backoff
5. dead-lettering
6. heartbeats and lease loss
7. the canonical effect type catalog
8. invariants

---

## 1. Effect lifecycle

```
            ┌─────────────────────────────────────────────────────────┐
            │                                                         │
            ▼                                                         │
┌──────────────────┐  approval_required  ┌──────────────────────┐    │
│ pending          │────────────────────▶│ awaiting_approval    │    │
│ (just created)   │                     └──────────┬───────────┘    │
└────────┬─────────┘                                │ approved        │
         │                                          ▼                 │
         │                              ┌────────────────────────┐    │
         │   claimed by worker          │ pending  (re-released) │────┘
         ├─────────────────────────────▶└────────────────────────┘
         │
         ▼
┌──────────────────┐ heartbeat       ┌──────────────┐ succeeded
│ claimed          │────────────────▶│ running      │─────────────▶ ┌────────────┐
└────────┬─────────┘                 └────────┬─────┘                │ succeeded  │
         │ lease lost                          │  failed             └────────────┘
         ▼                                     ▼
┌──────────────────┐                  ┌──────────────┐ exhausted   ┌────────────┐
│ pending (retry)  │                  │ failed       │────────────▶│ dead_letter│
└──────────────────┘                  └──────────────┘             └────────────┘
```

Statuses (matching `effect.status` in `02-data-model.sql`):

```
pending             initial state after enqueue_effect
awaiting_approval   Guard required human approval
claimed             a worker has taken the lease
running             the worker has called start_effect
succeeded           terminal: complete_effect(succeeded=true)
failed              terminal-for-attempt; may retry to pending
dead_letter         terminal: attempts exhausted
cancelled           terminal: explicit cancellation
```

---

## 2. Worker protocol

A worker is an actor (`actor.kind = 'worker'`) that polls or subscribes for `effect` rows matching its declared `worker_capability.effect_type`.

The protocol uses three RPCs:

### `claim_effect`

```
POST /v1/effects/claim
{
  "worker_id": "wkr_1",
  "effect_types": ["shell.run"],
  "lease_seconds": 60,
  "max_count": 1
}
```

Response is zero-or-more `effect` rows. Claiming creates an `effect_claim` row with `claimed_at` and `expires_at = claimed_at + lease_seconds`. Effect status moves `pending → claimed`.

Claim selection is FIFO over `effect.created_at` within each `effect_type`, filtered by `next_attempt_at <= now()`.

**Atomicity.** Claim must be implemented with a transactional update that sets `status='claimed'` and `assigned_worker_id` only if the current row state is `pending` — preventing double-claims under concurrent workers.

### `heartbeat`

```
POST /v1/effects/{effect_id}/heartbeat
{ "worker_id": "wkr_1", "extend_seconds": 60 }
```

Extends `effect_claim.expires_at`. If a heartbeat does not arrive before the lease expires, the lease is revoked (see §6) and the effect returns to `pending` with `attempt_count` incremented.

### `complete_effect`

The command from `03-command-spec.md`. Records the result, transitions the effect to `succeeded` or `failed`, releases the claim, and (inside the same transaction) chains to the downstream command (`record_tool_result`, `record_model_result`, etc.).

### Optional: `start_effect`

Before doing work, a worker should call:

```
POST /v1/effects/{effect_id}/start
{ "worker_id": "wkr_1" }
```

This is purely an observability signal — it transitions `claimed → running` and emits `effect_started`. Workers that omit this are not protocol-conformant but the effect will still complete correctly.

### Optional: `stream_observation`

For long-running effects (browser sessions, multi-step shell workflows), workers may stream observations:

```
POST /v1/effects/{effect_id}/observe
{ "worker_id": "wkr_1", "observation_ref": "art_..." }
```

Each observation creates an `agent_event` of type `effect_observed` linked to the effect. Subscribers see live updates.

---

## 3. Idempotency

**Rule.** Workers MUST be safe to retry. Idempotency is enforced at two layers:

1. **Effect layer.** Each effect carries an `idempotency_key`. The pair `(workspace_id, idempotency_key)` is unique. A duplicate `enqueue_effect` with the same key returns the existing effect.
2. **Worker layer.** Workers SHOULD use the `idempotency_key` when calling external APIs that support it (Stripe-style, GitHub `X-Idempotency-Key`, etc.). For APIs that do not support idempotency keys, workers SHOULD persist a local de-dupe ledger keyed by `effect_id` so re-execution after lease loss is safe.

**Counter-example: `shell.run` of a non-idempotent command.** Workers MUST NOT silently retry destructive shell commands. If `attempt_count > 1` and `risk_level >= high`, the worker rejects the claim with `requires_re_approval`, transitioning the effect to `awaiting_approval`.

---

## 4. Retries and backoff

Every effect carries `attempt_count` and `max_attempts`. On `failed`:

```
attempt_count += 1
if attempt_count >= max_attempts:
    status = 'dead_letter'
    emit dead_lettered event
else:
    status = 'pending'
    next_attempt_at = now() + backoff(attempt_count)
```

Backoff schedule (defaults; per-effect_type overrides allowed in policy):

```
attempt 1 → 2s
attempt 2 → 8s
attempt 3 → 30s
attempt 4 → 2m
attempt 5 → 10m
```

Jitter: ±25 %.

**Non-retriable errors.** A worker can mark a failure non-retriable in `complete_effect`:

```
{ "succeeded": false, "error": {...}, "retriable": false }
```

This skips the retry path and transitions directly to `dead_letter`.

---

## 5. Dead-lettering

A `dead_letter` effect:

- emits `effect_dead_lettered`,
- creates a `human.notify` follow-on effect for the workspace operators (unless suppressed by policy),
- if the effect is gated by a `workflow_step_run`, marks the step `failed` with the original error,
- can be re-attempted by an operator via `enqueue_effect` with a new `idempotency_key`, which Guard treats as a fresh request.

---

## 6. Heartbeats and lease loss

A worker missing two consecutive heartbeats (lease window expires) loses the claim:

```
on lease expiry:
  release effect_claim
  effect.status = 'pending'
  effect.assigned_worker_id = null
  effect.attempt_count += 1
  emit effect_lease_lost
  apply backoff to next_attempt_at
```

The next claim arrives from any capable worker, possibly the same one. This is why idempotency is non-optional.

Workers should size `lease_seconds` to the 99th-percentile completion time of their slowest effect, plus a safety margin.

---

## 7. Effect type catalog

Initial set. Each row lists the effect type, the table or subsystem it maps to, the typical required permission, and the default risk level.

| `effect_type`         | Maps to                       | `required_permission`        | Default risk |
| --------------------- | ----------------------------- | ---------------------------- | ------------ |
| `model.call`          | `model_call`                  | `model.call:<route>`         | low          |
| `tool.call`           | `tool_call`                   | `tool.call:<name>`           | varies       |
| `shell.run`           | tool_call (kind=shell)        | `shell.run`                  | high         |
| `browser.act`         | tool_call (kind=browser)      | `browser.tabs.write`         | medium       |
| `file.read`           | tool_call (kind=file)         | `file.read:<pattern>`        | low          |
| `file.write`          | tool_call (kind=file)         | `file.write:<pattern>`       | high         |
| `http.request`        | tool_call (kind=http)         | `http.request:<host>`        | medium       |
| `calendar.read`       | tool_call (kind=app)          | `calendar.read`              | low          |
| `email.draft`         | tool_call (kind=app)          | `email.draft`                | low          |
| `email.send`          | tool_call (kind=app)          | `email.send`                 | high         |
| `message.send`        | tool_call (kind=app)          | `message.send:<channel>`     | medium       |
| `memory.embed`        | `embedding_ref`               | `memory.write`               | low          |
| `workflow.dispatch`   | `workflow_run`                | `workflow.run`               | low          |
| `human.notify`        | (notification fan-out)        | `human.notify`               | low          |

`tool.call` is a meta-type used when the worker resolves the specific tool kind dynamically (e.g. an MCP worker for many MCP tools). Sub-kinds give Guard a finer surface to bind permissions to.

---

## 8. Invariants

The effect protocol is correct when *all* of the following hold:

1. **No effect outside a command transaction.** Every effect row is created inside the transaction of the command that requested it. Workers never create effects directly.
2. **No side effect inside a transaction.** Workers operate strictly outside the database transaction that scheduled the effect.
3. **Exactly-one claim at a time.** At any moment, an `effect.status='claimed'` row has exactly one open `effect_claim`.
4. **At-least-once execution.** The system guarantees the worker will get the effect at least once. Combined with idempotency, this gives effective exactly-once behavior.
5. **Approval gates are sticky.** An effect that entered `awaiting_approval` cannot be claimed until an `approve_effect_*` command commits.
6. **No cross-tenant effects.** Workers may only claim effects whose `workspace_id` matches a workspace they are registered for.
7. **Lease loss = retry.** Lease loss always increments `attempt_count` and applies backoff; the worker that timed out has no special privilege to re-claim.
8. **Dead-letter requires human re-entry.** Once dead-lettered, an effect cannot self-recover.

---

## 9. Worker registration and capability discovery

A worker becomes eligible to claim effects when:

```
worker.status = 'online'
worker_capability includes the effect_type
worker.workspace_id matches the effect.workspace_id
worker actor has the appropriate permissions for the effect_type
```

Workers may declare additional metadata (host, version, region, hardware) that Guard or the scheduler can use to prefer specific workers — for instance, routing `model.call` for sensitive context only to a worker with `region='local'`.

---

## 10. Failure modes worth naming

| Mode                            | What protects us                                                       |
| ------------------------------- | ---------------------------------------------------------------------- |
| Worker crashes mid-effect       | Lease expires → effect returns to pending → idempotency lets retry.    |
| Worker double-completes         | `complete_effect` is idempotent on `effect_id`; second call is no-op.  |
| Worker completes after timeout  | `complete_effect` checks the `effect_claim.released_at`; if the claim was released, the result is recorded as `effect_late_completion` (audit event), and the canonical result is the one from the successor worker. |
| External API double-charges     | Worker uses external idempotency keys when available; otherwise local ledger keyed by `effect_id`. |
| Approval racing with execution  | Approval gate is enforced at claim time; a `pending` effect cannot be claimed if the latest `approval_request` is not `approved`. |
| Schema drift between tool versions | `tool_call.schema_version` is captured at request time; the worker compares and rejects on mismatch with `precondition_failed`. |

---

## Verification

- [ ] Every effect type listed here has at least one worker capability in a Phase 1 reference worker.
- [ ] Every status in the lifecycle has at least one transition that produces a Chronicle event.
- [ ] The idempotency invariant holds against the schema (`UNIQUE (workspace_id, idempotency_key)` in `02-data-model.sql`).
- [ ] Replay can reproduce effect outcomes by reading the `effect_result` table without re-executing the worker.
