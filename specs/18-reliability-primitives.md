# 18 — Reliability primitives

Autonomous work runs for minutes, hours, days, or months and is failure-prone, tool-heavy, cost-sensitive, and externally rate-limited. ActantDB ships **all** the primitives a developer would otherwise hand-build:

```
Throttle  Queue  Retry  Lease  Schedule
Budget    Circuit  Cache  DLQ   Lock
Ingress   Idempotency
```

Some of these already exist in earlier specs (Schedule in §4 of `07-workflows-and-replay.md`, Budget in §5 of `14-extended-primitives.md`, Cache in `actant-cache`). This file specifies the remaining ones and adds the integration rules.

## Goal

> ActantDB should be the backend where an agent can safely run for minutes, hours, days, or months without the developer hand-building orchestration infrastructure.

## 1. ActantThrottle — multi-axis rate limiting

### Schema

```sql
rate_limit_policy (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    name                TEXT NOT NULL,
    scope_type          TEXT NOT NULL,    -- 'actor'|'agent'|'subagent'|'tenant'|'workflow'
                                          -- |'tool'|'provider'|'model'|'worker'|'sensitivity'
    scope_pattern       TEXT NOT NULL,
    algorithm           TEXT NOT NULL,    -- 'token_bucket'|'leaky_bucket'|'fixed_window'
                                          -- |'sliding_window'|'concurrency'|'wfq'
                                          -- |'priority'|'deadline'|'adaptive'
    limit_value         INTEGER NOT NULL,
    refill_rate         REAL,
    window_seconds      INTEGER,
    burst_size          INTEGER,
    priority_weight     REAL,
    fairness_key        TEXT,
    created_at          TEXT NOT NULL
);

rate_limit_state (
    id                  TEXT PRIMARY KEY,
    policy_id           TEXT NOT NULL,
    scope_key           TEXT NOT NULL,
    tokens_available    REAL,
    window_start        TEXT,
    used_count          INTEGER,
    reset_at            TEXT,
    updated_at          TEXT NOT NULL,
    UNIQUE (policy_id, scope_key)
);
```

### Behavior

Every effect request is evaluated against all matching policies. Decision shape:

```json
{ "decision": "allow|delayed|denied", "policy": "...", "retry_after_ms": 4200, "fairness_key": "ws_123" }
```

`actant-throttle` reads `RateLimit-*` HTTP headers returned from upstream providers and updates `adaptive` policies on the fly — so an OpenAI 429 makes the next attempt know to wait.

### CLI

```
actant throttle list
actant throttle show <name>
actant throttle set <name> --limit 60/min --burst 10
actant throttle status
actant throttle simulate --actor coding_agent --effect model.call
```

## 2. ActantQueue — queues + backpressure

### Schema

```sql
effect_queue_entry (
    id              TEXT PRIMARY KEY,
    effect_id       TEXT NOT NULL,
    queue_name      TEXT NOT NULL,
    priority        INTEGER NOT NULL,
    fairness_key    TEXT,
    available_at    TEXT NOT NULL,
    deadline_at     TEXT,
    attempts        INTEGER NOT NULL,
    status          TEXT NOT NULL,    -- 'queued'|'claimed'|'completed'|'failed'|'dead_letter'
    created_at      TEXT NOT NULL
);
```

### Policies

```
FIFO  priority  WFQ  deadline-first  least-cost-provider  local-first  sensitivity-aware  tenant-fair
```

### Backpressure

When queues breach thresholds, ActantDB emits `queue_backpressure_detected` with recommended actions: pause new workflows, prefer local models, batch embeddings, defer low-priority, require human confirmation. `actant-flow` consults this signal before scheduling more nodes.

### CLI

```
actant queue list
actant queue show <name>
actant queue drain <name>
actant queue pause <name>
actant queue resume <name>
actant queue dead-letter
```

## 3. ActantRetry — explicit retry policies

The Phase 1 effect protocol already retries (`/specs/04-effect-protocol.md` §4). Phase 2+ promotes retry policies to first-class rows so they can be referenced across effects and edited at runtime.

### Schema

```sql
retry_policy (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    name                TEXT NOT NULL,
    max_attempts        INTEGER NOT NULL,
    backoff_type        TEXT NOT NULL,    -- 'exponential'|'linear'|'fixed'|'fibonacci'
    initial_delay_ms    INTEGER NOT NULL,
    max_delay_ms        INTEGER NOT NULL,
    jitter              INTEGER NOT NULL, -- 0/1
    retry_on            TEXT NOT NULL,    -- JSON array of error classes
    do_not_retry_on     TEXT NOT NULL
);
```

Effects and workflow nodes reference `retry_policy.id`. Worker-side retries honor `retry_on` / `do_not_retry_on` lists.

### Idempotency required

A retried effect MUST be safe to retry. This is enforced at the tool layer:

```
tool.idempotency_required = true   →  the tool is idempotent (e.g. model.call with the same prompt)
tool.idempotency_required = false  →  the tool is NOT auto-retried; failures escalate to approval
```

Tools with `false` and `default_risk_level >= high` go straight to `awaiting_approval` on any retry beyond the first.

## 4. ActantCircuit — circuit breakers

### Schema

```sql
circuit_state (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    dependency_key      TEXT NOT NULL,   -- 'provider:openai'|'tool:shell.run'|'mcp:github'
    state               TEXT NOT NULL,   -- 'closed'|'open'|'half_open'|'degraded'
    failure_count       INTEGER NOT NULL,
    success_count       INTEGER NOT NULL,
    opened_at           TEXT,
    half_open_at        TEXT,
    reason              TEXT,
    updated_at          TEXT NOT NULL,
    UNIQUE (workspace_id, dependency_key)
);
```

### Behavior

When a circuit opens, `actant-models` (model routing) and `actant-effects` (claim) refuse to route to that dependency. `actant-flow` records the route choice in `model_route_decision.fallbacks`. The chronicle records `circuit_opened` / `circuit_closed` events.

Adaptive thresholds per dependency type:

```
provider     5 failures in 60s → open for 30s → half_open with 10% traffic
mcp          3 failures in 60s → open for 60s
tool.shell   10 failures in 5m → degrade (high-priority only)
```

### CLI

```
actant circuit list
actant circuit show provider:openai
actant circuit open provider:openai
actant circuit reset provider:openai
```

## 5. ActantDLQ — dead-letter handling

### Schema

```sql
dead_letter_item (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    original_effect_id  TEXT,
    workflow_run_id     TEXT,
    failure_type        TEXT NOT NULL,
    failure_summary     TEXT NOT NULL,
    attempts            INTEGER NOT NULL,
    last_error          TEXT,
    created_at          TEXT NOT NULL
);
```

### Behavior

When an effect exhausts retries (`max_attempts` from its `retry_policy`) it transitions to `dead_letter` and a `dead_letter_item` row appears. A `human.notify` follow-on effect surfaces it to operators (unless suppressed). DLQ is the **bridge to evals**: `actant dlq convert-to-eval <id>` mints an `eval_case` from the failure.

### CLI

```
actant dlq list
actant dlq show <id>
actant dlq retry <id>
actant dlq discard <id>
actant dlq convert-to-eval <id>
```

## 6. ActantLock — resource locks

### Schema

```sql
lock (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    resource_key        TEXT NOT NULL,
    owner_actor_id      TEXT NOT NULL,
    lease_id            TEXT,
    expires_at          TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    UNIQUE (workspace_id, resource_key)
);
```

Key conventions:

```
lock:file:<path>
lock:ticket:<id>
lock:memory:<id>
lock:workflow:<name>
lock:actor:<id>
lock:tenant:<id>
```

Locks are lease-bounded; expiry is mandatory. The `actant-lock` API exposes `acquire`, `extend`, `release`, `force_release` (audited).

## 7. ActantIngress — external events

### Schema

```sql
ingress_event (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    source              TEXT NOT NULL,    -- 'webhook'|'email'|'calendar'|'fs'|'mcp_resource'|'a2a'|'manual'
    event_type          TEXT NOT NULL,
    payload_ref         TEXT NOT NULL,
    signature_valid     INTEGER,
    dedupe_key          TEXT,
    received_at         TEXT NOT NULL,
    UNIQUE (workspace_id, source, dedupe_key)
);
```

### Behavior

- Webhooks: HMAC verification against per-trigger secret. Invalid signatures are recorded with `signature_valid=0` and never advance any workflow.
- Email + calendar: pulled via `actant-protocol` adapters and entered with `source='email'` / `'calendar'`.
- Deduplication via `(source, dedupe_key)` UNIQUE.
- Triggers in `actant-trigger` map `ingress_event` rows to workflow runs.

### CLI

```
actant ingress list
actant ingress test <source>:<event_type>
actant ingress replay <id>
```

## 8. ActantIdempotency — universal idempotency

Every command and every effect carries an idempotency key. Repeated submissions return the original result rather than producing a duplicate side effect.

### Schema

```sql
idempotency_record (
    idempotency_key     TEXT NOT NULL,
    workspace_id        TEXT NOT NULL,
    actor_id            TEXT NOT NULL,
    command_type        TEXT NOT NULL,
    input_hash          TEXT NOT NULL,
    result_ref          TEXT,
    created_at          TEXT NOT NULL,
    PRIMARY KEY (workspace_id, idempotency_key)
);
```

`command_record` already references the idempotency key via header (`Idempotency-Key`); migration 0003 adds the universal index table for fast lookup.

## 9. Defaults catalog

Every scaffolded project gets these `actant.yaml` defaults:

```yaml
defaults:
  workflows:
    retries: { max_attempts: 3, backoff: exponential, jitter: true }
  model_calls:
    timeout: 60s
    rate_limit: 60/min/provider
    fallback: enabled
    cache: safe_only
  tool_calls:
    approval_required_for: [shell.run, file.write, email.send, browser.submit_form]
    timeout: 120s
    idempotency_required: true
  memory:
    candidate_review: required
    auto_approve_low_risk: false
    max_candidates_per_session: 10
  approvals:
    max_pending_per_user: 20
    fatigue_limit: 5/min
    expiry: 24h
  workers:
    heartbeat_interval: 10s
    lease_timeout: 60s
    max_concurrent_effects: 4
  budgets:
    default_workflow_cost_usd: 2.00
    default_workflow_model_calls: 20
```

## 10. Cross-cutting integration loop

```
Agent wants to act
  ↓ Command engine validates
  ↓ Guard checks permissions + intent–action alignment
  ↓ Throttle checks rate limits
  ↓ Budget checks remaining allowance
  ↓ Circuit checks dependency health
  ↓ Queue schedules effect with priority + fairness
  ↓ Lease assigns worker (input_hash bound, sandbox bound)
  ↓ Worker executes
  ↓ Retry handles failure if retry policy allows
  ↓ Observation recorded
  ↓ Workflow advances
  ↓ Memory + context + replay updated
  ↓ DLQ catches exhausted retries
  ↓ Subscribers update live
  ↓ Trace exporter emits OTel spans
```

Every arrow has a span. Every step is replayable.

## 11. Phase staging

| Phase | Reliability deliverables                                          |
| ----- | ----------------------------------------------------------------- |
| 1     | Effect queue (in `effect` table; `actant-effects`), idempotency_record, basic retries, sane defaults catalog. CLI: `actant queue list`, `actant retry show`, `actant dlq list`. |
| 2     | Full `actant-throttle`, `actant-circuit`, `actant-lock`, `actant-ingress`. Adaptive provider rate ingestion. Worker lease input-hash enforcement. |
| 3     | Backpressure signals from queue → flow. Fairness keys per workspace. |
| 4     | Cron + event + webhook triggers in `actant-trigger` interoperate with `actant-ingress`. Failure-to-eval pipeline (DLQ → eval). |
| 6     | Multi-tenant rate limits, quotas, audit-ready throttle traces.    |

## 12. Invariants

1. **No effect bypasses Throttle + Budget + Circuit.** Every enqueue runs the trio.
2. **No retry without idempotency.** Tools with `idempotency_required=false` and `risk >= high` cannot be auto-retried.
3. **No silent DLQ.** A `dead_letter` transition emits a `dead_letter_item` and (by default) a `human.notify` effect.
4. **No webhook without signature verification.** `ingress_event.signature_valid` defaults to 0; only verified events flow into triggers.
5. **No lock without expiry.** Every `lock` row has `expires_at`; force-release is audited.
6. **No idempotency leak across actors.** The unique key is `(workspace_id, idempotency_key)` and lookups are filtered by `actor_id`.

## Verification

- [ ] Every table here has a CREATE TABLE in `/migrations/0003_ai_native_and_reliability.sql`.
- [ ] Every CLI command names in §1-§7 exists in `actant-cli` Phase staging.
- [ ] Every invariant in §12 has a corresponding test in the named crate.
- [ ] No new code path skips Throttle + Budget + Circuit at enqueue time.
- [ ] Defaults in §9 ship inside the `coding-agent` template's `actant.yaml`.
