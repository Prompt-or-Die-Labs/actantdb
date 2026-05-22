# 19 — Performance architecture

ActantDB has accumulated a large surface — chronicle, commands, effects, policy, memory, context, workflows, replay, index, protocols, observability, reliability primitives. If every command synchronously touched all of it, the system would be slow.

The discipline:

> **The synchronous path validates authority, commits intent, updates hot projections, and enqueues effects. Everything expensive happens asynchronously.**

This document specifies what runs in the hot path, what runs in nearline lanes, what runs in cold lanes, and the latency budgets each must meet.

## 1. Two halves

```
ActantDB Kernel            Async Lanes
─────────────────          ────────────────────────────────
extremely fast state       react to committed events
transition engine          do expensive work
                           emit follow-on events

hot path:                  nearline:
  command validate           workflow advance
  compiled policy check      memory candidate generation
  budget/rate fast check     embedding generation
  append event               retrieval enrichment
  update hot projection      reranker calls
  enqueue effect             observability export
  notify subscribers         eval generation
                             incident analysis

                           cold:
                             archive compaction
                             governance exports
                             long-term analytics
                             vector reindexing
                             historical replay
```

The kernel lives in `actant-command::kernel` (a thin composer over command, policy, storage, effects, and subscribe surfaces). The async lanes are ordinary subscribers to the chronicle.

## 2. Latency budgets

Engineering targets, not contractual guarantees:

| Operation                    | p50 (local)   | p99 (local)    |
| ---------------------------- | ------------- | -------------- |
| `append_user_message`        | < 2 ms        | < 20 ms        |
| `request_tool_call`          | < 5 ms        | < 30 ms        |
| `approve_tool_call`          | < 5 ms        | < 30 ms        |
| `complete_effect` (intake)   | < 3 ms        | < 20 ms        |
| `enqueue_effect`             | < 3 ms        | < 20 ms        |
| Subscription fanout          | —             | < 10 ms p50    |
| Hot projection read          | < 0.5 ms      | < 5 ms         |
| Compiled policy check        | < 0.1 ms      | < 1 ms         |

Async lanes have no end-to-end SLO; they have **throughput** targets and **freshness** targets:

| Lane              | Freshness target              |
| ----------------- | ----------------------------- |
| Workflow advance  | < 100 ms after triggering event |
| Memory candidate  | < 5 s after generating observation |
| Embedding         | < 30 s after row becomes eligible |
| Retrieval trace   | < 50 ms after retrieval completes |
| OTel export       | continuous, sampled per policy |
| Eval generation   | < 5 min after DLQ entry        |
| Compliance export | nightly                        |

CI runs a `bench/` harness that enforces the p99 numbers; regressions > 10 % fail the PR.

## 3. Storage tiers

```
L0  in-memory hot projections
     pending approvals, active workflows, leases, budgets, worker health,
     hot memory shortlists, capability tokens, rate-limit/circuit state
L1  append-only event log (WAL/commit log)
     authoritative causal history
L2  projection tables (SQLite Phase 1 → Postgres Phase 6)
     sessions, messages, tool calls, memories, workflows, context builds
L3  semantic + sparse indexes (actant-embed adapters)
L4  artifact / blob store (filesystem / S3 / GCS)
L5  cold archive (compressed events, governance exports, long-term traces,
     analytics warehouse)
```

The hot path touches **L0 + L1** synchronously and **L2** only for the projection write that the command requires. L3, L4, L5 are async-only.

## 4. Hot kernel operations

For every command the kernel runs *only* the steps below. Anything else goes to a lane.

```
1. Authenticate actor              (capability token lookup; L0)
2. Validate command schema         (compiled from registry; in-process)
3. Compiled policy check           (decision DAG; L0)
4. Lightweight budget/rate check   (in-memory counters; L0)
5. Append event                    (L1 WAL)
6. Update hot projection           (L0 + L2 row)
7. Enqueue effect (if needed)      (L0 + L2 effect_queue_entry)
8. Notify subscribers              (in-process changefeed broadcast)
```

Steps 5–7 happen inside one short transaction; step 8 happens immediately after commit.

## 5. Per-operation sync/async breakdown

This is the binding implementation contract for the alpha command set.

### `append_user_message`

```
Sync:
  validate actor
  append agent_event(user_message_received)
  insert message projection row
  notify session subscribers

Async (lanes):
  summary lane               → message summary
  embedding lane             → message embedding (queued via memory.embed effect)
  memory-candidate lane      → scan for new candidates
  entity-extraction lane     → entities into actant-memory::index
  analytics lane             → activity metrics
```

### `request_tool_call`

```
Sync:
  validate tool contract (compiled)
  check compiled permission + intent-action alignment
  fast budget + rate-limit check
  append agent_event(tool_call_requested)
  insert tool_call (status by Guard decision)
  create approval_request OR effect_queue_entry
  notify subscribers (approval center; worker queue)

Async:
  risk-explanation lane      → natural-language risk note attached to the approval
  trace lane                 → OTel span export
  incident-pattern lane      → "is this part of a drift cluster?"
  eval-candidate lane        → flag for possible eval if outcome surprises
```

### `approve_tool_call`

```
Sync:
  authority check (approver eligibility)
  update approval_request (status, scope_granted)
  update tool_call (status=approved)
  insert effect (or move from awaiting_approval to pending)
  append agent_event(tool_call_approved)
  if scope_granted != 'once': insert authority_scope
  notify worker queue + Studio

Async:
  approval-fatigue analytics
  policy-learning lane       → suggest a scope grant if pattern repeats
  audit-export lane          → next nightly batch
```

### `complete_effect`

```
Sync:
  validate lease (worker_id + input_hash match effect_claim)
  insert effect_result
  update effect (succeeded/failed)
  insert downstream projection (tool_call_finished or model_call_finished)
  append agent_event(effect_completed) + chained event
  release effect_claim
  notify subscribers

Async:
  artifact-summary lane      → summarize tool output for Studio + memory feed
  embedding lane             → embed observation if eligible
  memory-candidate lane
  workflow-advance lane      → progress the parent workflow_run
  eval lane                  → if this effect mapped to an open eval candidate
```

The full table extends to every command in `03-command-spec.md`; the rule of thumb: **anything that has to traverse history, call a model, hit a vector store, talk to a worker, or emit telemetry is async by default.**

## 6. Compiled policy

Three tiers, fastest first:

1. **Capability tokens.** Per session/workflow start, compile authority into a compact token: `(actor_id, scope_hash, allowed_actions_bitmap, resource_pattern_handles, sensitivity_ceiling, expiry, policy_version)`. Most commands check the token in microseconds.
2. **Decision DAG.** Policy bundles compile into a graph indexed by `(effect_type, actor_kind, resource_kind, sensitivity, workflow_mode, approval_mode)`. Lookup is array-indexed, not string-matched.
3. **Full policy engine.** Used only for cold paths: new workflow registration, new delegation, new external tool, incident review, enterprise compliance, policy simulation. Tens of milliseconds is acceptable here.

The compiler runs at every `policy_changed` / `authority_scope_changed` event. Hot tokens are invalidated by the changefeed.

## 7. Retrieval profiles

Retrieval choice is made by the caller per request. ActantIndex supports admission control through profiles:

```yaml
retrieval_profiles:
  fast:            # hot caches + lexical + memory recency only
    dense_top_k: 20
    sparse_top_k: 20
    rerank: false
    graph_expand: false
  balanced:        # default for coding-agent template
    dense_top_k: 50
    sparse_top_k: 50
    rerank_top_k: 50
    graph_expand: false
  deep:            # debugging + long-form research
    dense_top_k: 200
    sparse_top_k: 200
    graph_expand: true
    rerank_top_k: 200
  local_private:   # local model, sensitive context
    local_only: true
    cloud_rerank: false
  degraded:        # under backpressure
    dense_top_k: 10
    sparse_top_k: 10
    rerank: false
```

`actant-memory::index::plan` picks the profile from `(latency_goal_ms, sensitivity_ceiling, budget_remaining, task_complexity, backpressure)`. The chosen profile is recorded on `retrieval_trace`.

## 8. Subscription discipline

Subscriptions are filtered server-side. The kernel publishes events; each subscriber declares an indexed filter; the changefeed dispatches only matching rows.

Anti-patterns the kernel refuses:

- "Subscribe to all `agent_event` rows globally." Rejected unless the actor has `audit.global_subscribe`.
- "Subscribe to everything in workspace X." Rejected unless paginated and rate-bounded.
- "Subscribe with a non-indexed filter." Rejected; filters must hit an index.

Subscription updates are **coalesced**: rapid same-row changes within a short window collapse into one push. Slow consumers receive a `lag` notification and re-snapshot.

## 9. Backpressure

Every async lane can signal backpressure to the kernel via `lane_backpressure_signal` rows. The kernel reacts:

```
queue_depth high          → admission: refuse "deep" retrieval; auto-downgrade to "balanced"
embedding_backlog high    → admission: defer non-critical memory embeds
model_provider rate-limited → admission: prefer local routes; circuit half-open
subscription_lag high     → admission: degrade fanout to coalesced batches
memory_pressure high      → admission: drop OTel sampling rate
approval_fatigue high     → admission: batch low-risk approvals into a digest
```

Backpressure responses are observable via Studio (Workers + Lanes panels) and via OTel metrics.

## 10. Hot caches

The kernel and adjacent crates hold these in memory, invalidated by changefeed events:

```
authority_cache         keyed by actor_id   invalidated by permission_changed
policy_decision_cache   keyed by (effect_type, actor_kind, resource_kind, sensitivity)
                                            invalidated by policy_changed
tool_contract_cache     keyed by tool_id   invalidated by tool_schema_changed
model_registry_cache    keyed by route_id   invalidated by model_route_changed
rate_limit_state        keyed by scope_key   periodically snapshotted
budget_state            keyed by scope_id    invalidated by budget_consumed
workflow_state          keyed by workflow_run_id  invalidated by workflow_step_completed
worker_heartbeat_state  keyed by worker_id   updated by heartbeat
pending_approvals       keyed by reviewer_id  invalidated by approval_resolved
hot_memory_shortlist    keyed by session_id   refreshed periodically
```

Every cache entry inherits the sensitivity of its source row. `secret`-class entries are not cached at all (consistent with `/specs/adr/0014` cache rules).

## 11. Deployment modes

Same kernel, different enabled async lanes. Modes:

| Mode             | Lanes enabled                                                    |
| ---------------- | ---------------------------------------------------------------- |
| `local-fast`     | minimal: workflow, embedding (local only), retrieval-trace.       |
| `developer`      | + memory-candidate, OTel stdout, eval shadow, replay checkpoints.|
| `team`           | + governance evidence, audit export, prompt registry, model routing decisions, multi-tenant trim. |
| `enterprise`     | + policy-as-code, supply-chain verification, OTel OTLP export, SIEM bridge, legal hold. |
| `regulated`      | + payments record-keeping (AP2), retention enforcement, attestation. |
| `research`       | + experimental modes (alternate rerankers, multivector, ColBERT-class).|

Modes are workspace-level (`workspace.deployment_mode`). Switching modes flips which lanes subscribe; the hot kernel is unchanged.

## 12. Progressive enrichment

Write minimal facts immediately; add richness later.

Example: `tool_call_requested`.

```
Immediate (hot path):
  tool_name, redacted args hash, actor, status, risk_class

Within seconds (lanes):
  natural-language risk explanation
  detailed artifact links
  source-quality impact
  compliance mapping
  eval candidates
```

UI consumers render the minimal record instantly, then update through subscriptions as enrichment arrives.

## 13. Sharding (cloud mode)

Phase 6+ shards by workspace. Cross-workspace operations are async events, never distributed transactions. A "team agent" coordinating across workspaces is implemented as outbound A2A messages, not as cross-shard SQL.

## 14. What never goes in the hot path

The kernel structurally refuses these in a command transaction:

```
model.call                    (always an effect)
embedding generation          (always async)
reranker call                 (always async)
shell/file/browser execution  (always an effect)
HTTP request                  (always an effect)
vector store query            (only if caller explicitly requests fast profile under admission control)
graph expansion               (always async)
compliance evidence rollup    (always cold)
OTel OTLP export              (always async, sampled)
artifact processing           (always async)
```

A grep over the eventual `actant-command::commands/*.rs` for these patterns inside a `Transaction<'_>` must return zero hits. This is an architectural test.

## 15. Implementation sequence

| Phase | Performance discipline                                          |
| ----- | --------------------------------------------------------------- |
| 1     | `actant-command::kernel` + capability tokens + compiled policy + hot projection tier + L0 cache for budgets/rate-limits. Benchmark harness in `bench/`. |
| 2     | Lanes pattern formalized via `actant-subscribe` consumers. Backpressure signals. Workers fully async. |
| 3     | Hot memory shortlists. Selective subscription filter validator. Snapshot/compaction for SQLite. |
| 4     | Workflow deterministic-shell discipline enforced; replay deterministic from event log. |
| 5     | Replay uses recorded outputs by default; experimental mode re-invokes via lanes. |
| 6     | Sharding + columnar analytics side store + cross-region considerations. |

## 16. Invariants

1. **No model/tool/HTTP/embedding inside a command transaction.** Verified by grep.
2. **No unfiltered subscriptions on the hot path.** Filters must hit an index; verified by subscription engine.
3. **Hot projection writes never block on lane work.** Lanes consume after commit.
4. **Every command's transaction is bounded.** A wall-clock guard logs any commit > 50 ms.
5. **Capability tokens are the first authority check.** Full policy engine is only invoked on cache miss or compile.
6. **Backpressure is always observable.** Every lane emits its backlog as a metric.

## Verification

- [ ] `actant-command::kernel`'s dispatch table covers every alpha command.
- [ ] Compiled policy decision DAG is array-indexed (no string match on the hot path).
- [ ] Bench harness in `bench/` measures every operation in §2 and asserts p50/p99.
- [ ] Grep test: zero HTTP / process spawn / model SDK calls inside any `Transaction<'_>` block.
- [ ] Every deployment mode in §11 has a documented lane set.
- [ ] Every backpressure response in §9 has a corresponding admission-control hook in the kernel.
