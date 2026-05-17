# 01 — Architecture

ActantDB is composed of ten core subsystems plus three companion stores. Every subsystem is reachable through commands, observed through subscriptions, and recorded in the Chronicle.

```
ActantDB
├── 1.  Chronicle           (causal event ledger; causal DAG via causal_parent_ids)
├── 2.  Command Engine      (typed mutation surface)
├── 3.  Projection Store    (queryable derived tables)
├── 4.  Subscription Engine (live row replication)
├── 5.  Effect Engine       (side-effect queue + workers)
├── 6.  Guard               (policy, permissions, approvals)
├── 7.  Context Engine      (model-context firewall)
├── 8.  Memory Engine       (provenance + lifecycle)
├── 9.  Flow Engine         (durable workflows)
├── 10. Replay Engine       (checkpoints + reruns)
└── Companion stores: Artifact Store, Secret Vault, Semantic Index
```

The Phase 1 alpha implements the ten subsystems above. Phase 2+ adds extended primitives that cross-cut these subsystems rather than forming new ones:

```
Extended primitives (Phase 2+; see specs/14-extended-primitives.md)
├── Intent Layer            (Guard pre-check: intent vs proposed effect)
├── Observation Layer       (structured evidence, distinct from raw events)
├── Capsule Layer           (policy travels with derivations; sensitivity lineage)
├── Authority Calculus      (Delegation + Budget extend Guard's surface)
├── Self-improvement Loop   (Regret → Eval, plus Memory Conflict resolution)
├── Steering Layer          (Intervention as first-class command)
└── Trust Layer             (behavioral trust profiles modulate risk)
```

AI-native + reliability primitives ship alongside as their own subsystems:

```
AI-native (specs/15-actant-index.md, 16-protocols.md, 17-observability.md)
├── ActantIndex             (hybrid dense+sparse+graph retrieval, reranking, traces)
├── Embedder Registry       (FastEmbed, MLX, OpenAI, Voyage, Cohere, Jina, Mixedbread, Nomic)
├── Prompt + Tool Registry  (versioned artifacts; replay reads exact prompt + schema version)
├── Model Registry + Router (capability metadata; selection records `model_route_decision`)
├── Semantic Cache          (sensitivity-aware; secret never cached, high local-only)
├── Trace                   (OpenTelemetry GenAI + OpenInference compatible)
└── Protocol Adapters       (MCP, A2A, AP2)

Reliability primitives (specs/18-reliability-primitives.md)
├── Throttle    (multi-axis rate limits, adaptive provider headers)
├── Queue       (priority + fairness + backpressure)
├── Retry       (declarative retry policies)
├── Lease       (input-hash-bound worker mandates)
├── Circuit     (per-dependency breakers)
├── Cache       (consumed by Index, Memory, Context)
├── DLQ         (dead-letter → eval pipeline)
├── Lock        (lease-bounded resource locks)
├── Ingress     (HMAC webhooks, email, calendar, fs, MCP, A2A)
└── Idempotency (universal; every command and effect)
```

The whole system is organized as a **hot kernel + async lanes** (`specs/19-performance-architecture.md`, ADR-0018). The hot kernel (`actant-kernel`) runs only: actor authentication, compiled policy check, fast budget/rate check, event append, hot projection update, effect enqueue, subscription notify. Everything expensive — embeddings, model calls, reranking, workflow advancement, OTel export, compliance evidence, eval shadow — runs in async lanes that subscribe to the chronicle.

```
                          ┌──────────────────────────┐
                          │ Clients / Agents / Studio│
                          └────────────┬─────────────┘
                                       │
                                       ▼
              ┌─────────────────────────────────────────────┐
              │ actant-kernel (hot path; p99 < 30 ms)        │
              │  validate → policy → budget → append event   │
              │  → hot projection → effect enqueue → notify  │
              └─────────────────────┬───────────────────────┘
                                    │
                ┌───────────────────┼───────────────────┐
                ▼                   ▼                   ▼
        L0 hot projections   L1 event log WAL    Effect queue + leases
                                    │
                                    ▼
              ┌─────────────────────────────────────────────┐
              │ Async lanes (see /planning/lane-catalog.md)  │
              │  workflows, embeddings, retrieval enrichment,│
              │  memory, evals, OTel export, compliance,     │
              │  graph extraction, drift, trust, audit ...   │
              └─────────────────────────────────────────────┘
```

This document specifies each subsystem at the architectural level. Tables live in `02-data-model.sql`; commands live in `03-command-spec.md`; the worker protocol lives in `04-effect-protocol.md`. The deeper framing is in `13-actant-contract.md`.

---

## 1. Chronicle — the causal event ledger

**Purpose.** Store what happened, who caused it, under what authority, and with what downstream effects.

**Invariants.**

- Append-only. Events are never updated or deleted; deletion is modeled as a tombstone event (see `05-security-model.md` — cryptographic erasure).
- Every event has exactly one **actor** (who) and zero-or-one **parent event** (cause).
- Every event has a **payload hash** even if the payload itself is stored out-of-band (artifact store, secret ref, embedding ref).
- Events are **hashed in a chain** (`event_hash` includes `parent_event_id`'s hash) so the ledger is tamper-evident.

**Event types** (non-exhaustive — see `03-command-spec.md` for the full list):

```
user_message_received        memory_candidate_created
context_build_started        memory_approved
context_build_finished       memory_rejected
model_call_requested         workflow_started
model_call_finished          workflow_step_completed
tool_call_requested          agent_assigned_task
tool_call_approved           permission_granted
tool_call_denied             permission_revoked
tool_call_started            artifact_created
tool_call_finished           replay_checkpoint_created
```

**Why Chronicle matters.** It is the unit that lets the system answer:

```
Why did the agent do this?
What context did it see?
Who approved it?
Which tool was used?
What memory influenced the action?
What changed externally?
Can we replay from before this event?
```

That is the fundamental unit of accountable autonomy.

---

## 2. Command Engine — typed mutation surface

**Purpose.** Mutate state only through explicit, typed, permission-checked commands. No raw `UPDATE` against projection tables is ever exposed.

SpacetimeDB's *reducers* inspire this: they are the mutation path and run transactionally with rollback on failure. ActantDB commands are the agent-native version of that idea.

**Command lifecycle.**

```
receive command
→ authenticate actor
→ validate input schema
→ load policy snapshot
→ check authority
→ begin transaction
→ append command_record
→ update projections
→ append agent_event(s)
→ enqueue effect(s) if needed
→ commit transaction
→ notify subscribers
```

**Transactional boundaries.**

- A command's transaction includes: `command_record` insert, projection updates, `agent_event` inserts, `effect` inserts (the *intent* to do side work), and `approval_request` inserts.
- A command's transaction excludes: actually performing the side effect. Workers do that, asynchronously, outside the transaction.

**Why this matters.** It guarantees that the only durable way to change agent state is through a record that names the actor, the inputs, the policy snapshot, and the resulting events. No middleware, no manual SQL, no out-of-band writes.

---

## 3. Projection Store — current state

**Purpose.** Maintain queryable tables derived from the event ledger.

Events are the source of truth. Projections are the current readable state.

**Projection categories.**

| Category                   | Examples                                                       |
| -------------------------- | -------------------------------------------------------------- |
| **Ledger projections**     | `messages`, `model_calls`, `tool_calls`, `memories`            |
| **Operational projections**| `approval_request`, `effect`, `workflow_run`, `agent_task`     |
| **Policy projections**     | `authority_scope`, `policy`, `actor`                           |
| **Analytic projections**   | `model_call_stats`, `effect_failure_rate`, `memory_usage_stats`|
| **Replay projections**     | `replay_checkpoint`, `replay_run`, `replay_diff`               |

Projection writes are part of the command transaction. Projection reads are eventually consistent for subscribers (notification happens after commit) but strongly consistent for the actor that just issued the command.

---

## 4. Subscription Engine — live state to clients

**Purpose.** Push live updates to clients (apps, CLIs, dashboards, agents, workers).

SpacetimeDB-style: clients subscribe to a table + filter; the server replicates matching rows and streams updates when those rows change.

**Subscription targets.**

```
pending approvals         model-call telemetry
active tool calls         worker heartbeats
memory candidates         agent task board
workflow status           context builds
audit events              policy decisions
```

**Example.**

```ts
db.subscribe("approval_request", {
  status: "pending",
  workspace_id: "ws_123"
})
```

**Delivery semantics.**

- **At-least-once with monotonic versioning.** Each subscription delivers row versions; clients dedupe by `(table, row_id, version)`.
- **Initial snapshot, then incremental.** First message is the current matching rows; subsequent messages are inserts/updates/deletes.
- **Backpressure-aware.** Slow clients are paused; if the lag exceeds a threshold the server cancels the subscription and the client must resubscribe (and re-snapshot).
- **Filtered server-side.** Visibility is enforced server-side via the actor's authority — clients cannot subscribe to rows they cannot read.

---

## 5. Effect Engine — side effects outside the transaction

**Purpose.** Safely handle external side effects (model calls, shell commands, browser clicks, file writes, HTTP requests, calendar/email actions). This is one of ActantDB's deepest departures from a normal database.

**The principle.** Database transactions must not directly perform side effects. Otherwise:

- A failed I/O rolls back the database, but the external world already changed.
- A succeeded I/O commits the database, but a downstream commit failure leaves a half-applied world.
- Long-running effects (model calls, browser sessions) would hold transactions open.

**The decomposition.**

```
Command commits intent.
Effect row records the requested side effect.
Worker claims, executes, streams observations.
Worker completes the effect.
Effect result becomes a new event.
```

**Worker protocol (detail in `04-effect-protocol.md`).**

```
claim_effect → heartbeat → start_effect → stream_observations → complete_effect
```

**Effect types (initial set).**

```
model.call          file.write             email.draft
tool.call           http.request           message.send
shell.run           calendar.read          memory.embed
browser.act         workflow.dispatch      human.notify
file.read
```

Every effect has an **idempotency key**, a **required permission**, and a **risk level** that Guard uses to decide whether human approval is required.

---

## 6. Guard — policy, permissions, approvals

**Purpose.** Make authority explicit. Inspired by NIST's AI RMF — turn governance from documentation into runtime primitives.

**Authority model.**

- Every actor has zero-or-more **authority scopes**. A scope grants a `permission` over a `resource_pattern` up to a `sensitivity_ceiling`, with `allowed_actions` and optional `expires_at`.
- Every command and every effect declares the `required_permission` it consumes.
- Guard evaluates `(actor, command/effect, resource, sensitivity)` and returns one of: `allow`, `allow_with_approval`, `deny`.

**Approval flow.**

```
command requests effect
→ Guard computes risk_level (low | medium | high | critical)
→ if approval required:
     create approval_request
     notify approvers via subscription
     pause effect until approve_effect_* command commits
   else:
     proceed
```

**Guard answers.**

```
Can this actor do this?
Can this model see this?
Can this memory be used here?
Can this data sync remotely?
Can this workflow run unattended?
Does this action need human approval?
```

---

## 7. Context Engine — model-context firewall

**Purpose.** Make model context inspectable, governed, and replayable. Every model call has a **context manifest**.

**The manifest.** A `context_build` row plus N `context_item` rows. Items are tagged with: source, included/blocked, blocked reason, sensitivity, token count, rank score, visibility (`local_model_allowed`, `cloud_model_allowed`, `human_only`, `never_model`, `never_sync`).

**Build pipeline.**

```
1. gather candidates from: messages, memories, files, artifacts, prior tool results
2. score candidates (relevance, recency, agent intent)
3. filter by Guard (sensitivity vs model target)
4. redact (PII, secret refs, prompt-injection markers)
5. truncate to token budget
6. emit context_build + context_item rows
7. emit context_build_finished event
```

**Why this matters.** This is what enables an auditor to ask "Did the cloud model see browser history?" and get a definitive yes/no, not a guess from logs.

---

## 8. Memory Engine — governable agent memory

**Purpose.** Memory is not a vector DB row. Memory has a lifecycle, a provenance, a sensitivity, and a usage history.

**Lifecycle.**

```
observed → candidate → pending_review → approved → active
        → used → superseded → expired → revoked → deleted
```

**Provenance.** Every `memory` row references the `source_event_ids` (events that justified extracting this memory) and the originating `memory_candidate`. Every model call records which memories appeared in its context (`memory_use` rows). The graph `event → memory_candidate → memory → memory_use → model_call → tool_call` is fully traversable.

**User-facing affordances this enables.**

```
"Why do you remember this?"
"When did you learn this?"
"When did you use this?"
"Stop using this in work contexts."
"Never send this memory to a cloud model."
"Delete this memory and its embeddings."
```

---

## 9. Flow Engine — durable workflows

**Purpose.** Agents need workflows that survive process restarts, span hours/days, and gate on human approval.

**Examples.**

```
daily digest          ticket triage         repo maintenance
code review           customer escalation   meeting follow-up
research monitoring   browser automation    multi-agent task boards
```

**Node types.**

```
agent_task            condition             memory_write
model_call            parallel_group        file_operation
tool_call             human_task            browser_action
approval_gate         delay                 external_webhook
                                            subworkflow
```

**Run lifecycle.**

```
created → running → (paused | waiting_human | waiting_effect)
        → resumed → completed | failed | cancelled
```

Flow Engine reuses Chronicle (every step transition is an event), Effect Engine (nodes that need side effects enqueue them), and Guard (approval gates use the same approval flow as ad-hoc commands).

---

## 10. Replay Engine — the killer feature

**Purpose.** Rerun, debug, and compare agent behavior from any prior point.

**Replay modes.**

| Mode                    | Meaning                                                |
| ----------------------- | ------------------------------------------------------ |
| **Recorded**            | Reconstruct using stored model/tool outputs.           |
| **Experimental**        | Rerun model/tool steps from a checkpoint.              |
| **Policy**              | Rerun with stricter or looser permissions.             |
| **Model**               | Compare model A vs model B at the same checkpoint.     |
| **Memory**              | Include / exclude / edit memory and compare outcome.   |
| **Tool**                | Rerun with read-only tools or mocked tools.            |
| **Local-only**          | Forbid cloud context or cloud model calls.             |

**A checkpoint is sufficient if and only if** it captures: (1) the prefix of `agent_event` up to the chosen event, (2) the projection state derivable from that prefix, (3) the model route snapshot, (4) the permission snapshot, (5) the memory snapshot (which memories existed and their visibility), (6) the context build referenced by the chosen event.

**Replay output.** A `replay_diff` row that surfaces:

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

## Companion stores

ActantDB does not store every byte directly.

### Artifact Store

Holds screenshots, tool outputs, files, transcripts, reports, audio/video, large model outputs. ActantDB stores `artifact` rows with `uri` and `content_hash`; the bytes live in S3-compatible storage, a local filesystem, or another blob store.

### Secret Vault

Secrets never live in ActantDB. The system stores `secret_ref` only — provider, scope, last-used timestamp. The actual material lives in macOS Keychain, 1Password, HashiCorp Vault, or a cloud KMS.

### Semantic Index

Vector search is a *companion*, not a core subsystem. ActantDB stores `embedding_ref` rows; vectors live in Qdrant, LanceDB, Chroma, FAISS, pgvector, or a SQLite vector extension.

The rule: **vector DBs are indexes, not memory.** ActantDB governs what may be embedded, searched, used, and deleted.

---

## Deployment topologies

### Local personal mode

For personal desktop agents.

```
Mac app / CLI / browser extension
        ↓
local ActantDB node
        ↓
local workers: files, browser, shell, MLX, AppleScript
        ↓
local artifact store + Keychain + semantic index
```

### Local-first multi-device mode

```
laptop ActantDB  ┐
desktop ActantDB ┤── selective sync ── shared low-sensitivity state
phone companion  ┘
```

Selective sync examples:

```
sync workflow status
sync approvals
do not sync private memories
sync artifact hashes but not artifact contents
sync model telemetry without prompts
```

### Team self-hosted mode

```
team apps / dashboards / agents
        ↓
self-hosted ActantDB cluster
        ↓
workers in private network
        ↓
team artifact store / KMS / vector index
```

### Cloud control-plane mode

```
local agents ┐
cloud workers┤
dashboard    ├──→ ActantDB Cloud
mobile app   ┘
```

---

## Architecture diagram

```
                         ┌─────────────────────────────┐
                         │        Actant Studio         │
                         │ dashboard / approvals/replay │
                         └──────────────┬──────────────┘
                                        │
┌─────────────┐  ┌─────────────┐  ┌─────▼─────┐  ┌─────────────┐
│ Agent SDKs  │  │ Human Apps  │  │ CLI / API │  │ Dashboards  │
└──────┬──────┘  └──────┬──────┘  └─────┬─────┘  └──────┬──────┘
       └────────────────┴───────────────┴───────────────┘
                              │
                              ▼
                   ┌──────────────────────┐
                   │   ActantDB Server     │
                   │ commands/subscriptions│
                   └──────────┬───────────┘
                              │
        ┌─────────────────────┼──────────────────────┐
        ▼                     ▼                      ▼
┌──────────────┐      ┌───────────────┐      ┌────────────────┐
│ Command      │      │ Subscription  │      │ Policy / Guard │
│ Engine       │      │ Engine        │      │ Engine         │
└──────┬───────┘      └───────┬───────┘      └───────┬────────┘
       │                      │                      │
       ▼                      ▼                      ▼
┌──────────────┐      ┌───────────────┐      ┌────────────────┐
│ Chronicle    │      │ Projections    │      │ Context Engine │
│ Event Ledger │      │ Live Tables    │      │ Firewall       │
└──────┬───────┘      └───────┬───────┘      └───────┬────────┘
       │                      │                      │
       ▼                      ▼                      ▼
┌──────────────┐      ┌───────────────┐      ┌────────────────┐
│ Replay       │      │ Memory Engine  │      │ Flow Engine    │
│ Engine       │      │ Provenance     │      │ Workflows      │
└──────┬───────┘      └───────┬───────┘      └───────┬────────┘
       │                      │                      │
       └──────────────────────┼──────────────────────┘
                              ▼
                      ┌──────────────┐
                      │ Effect Queue │
                      └──────┬───────┘
                             │
          ┌──────────────────┼──────────────────┐
          ▼                  ▼                  ▼
   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
   │ Model       │    │ Tool        │    │ Workflow    │
   │ Workers     │    │ Workers     │    │ Workers     │
   └─────────────┘    └─────────────┘    └─────────────┘

Companion stores:
  Secret Vault / Keychain
  Artifact Store
  Semantic Index
  Cold Archive
```

---

## Verification

- [ ] Every subsystem here corresponds to one or more tables in `02-data-model.sql`.
- [ ] Every event type listed under Chronicle is emitted by at least one command in `03-command-spec.md`.
- [ ] Every effect type listed under Effect Engine is matched by a worker capability in `04-effect-protocol.md`.
- [ ] Every replay-mode requirement is satisfied by the `replay_checkpoint` columns in `02-data-model.sql`.
- [ ] Every Guard answer listed has a corresponding rule path in `05-security-model.md`.
