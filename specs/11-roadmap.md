# 11 — Roadmap

ActantDB ships in seven phases. Phase 0 (this spec set) is complete when every checklist at the bottom of every spec file is checked. Each subsequent phase ends at a **decision gate** — a small, named milestone whose pass/fail determines whether to start the next phase.

Durations are *target* ranges with a small team (2–4 engineers + 1 designer for Studio). They are not commitments. The bigger risks at each phase are called out under **Risks**.

```
Phase 0 — Specification              (1–2 weeks)
Phase 1 — Core alpha                  (4–6 weeks)
Phase 2 — Effect workers              (4 weeks)
Phase 3 — Context and memory          (4 weeks)
Phase 4 — Workflows                   (4–6 weeks)
Phase 5 — Replay                      (6–8 weeks)
Phase 6 — Cloud / team                (8–12 weeks)
```

Total: roughly 7–10 months of focused work to reach team-grade production.

---

## Phase 0 — Specification

**Duration.** 1–2 weeks.

**Deliverables.**

- All thirteen files under `specs/` complete and cross-referenced.
- README, LICENSE, NOTICE, CONTRIBUTING, CODE_OF_CONDUCT, .gitignore committed.
- Issue templates for spec changes.
- A short ADR log (`specs/adr/`) seeded with the three big design choices: (1) commands-as-mutation, (2) effects-outside-transaction, (3) context-as-manifest.

**Decision gate.**

- Phase 0 passes when one external reviewer (not the author) can read every checklist item in every spec file and check each box, citing the line that satisfies it.

**Risks.**

- Specifying too much: shipping a spec set that locks in choices Phase 1 will want to revisit. Mitigation: every spec carries a "Phase 0 simplifications" note in its tail; revisit lists are explicit.

---

## Phase 1 — Core alpha

**Duration.** 4–6 weeks.

**Goal.** A working `actantdb` binary with the smallest end-to-end surface that runs the §10 alpha demo for a single workspace.

**Build.**

- Rust workspace with crates: `actant-core`, `actant-storage`, `actant-command`, `actant-policy` (minimal), `actant-server`, `actant-cli`.
- SQLite-backed storage with the schema from `02-data-model.sql`.
- Command pipeline supporting the **alpha command set**:

  ```
  create_session
  append_user_message
  append_agent_message
  request_tool_call
  approve_tool_call
  deny_tool_call
  record_tool_result
  propose_memory
  approve_memory
  reject_memory
  ```

  These exercise: sessions, messages, the Chronicle, command_record, tool flow with approval, and the memory candidate/approval flow.

- HTTP `POST /v1/command` with idempotency-key support.
- WebSocket `/v1/subscribe` with snapshot + incremental for the tables the alpha demo touches (`approval_request`, `agent_event`, `tool_call`, `memory_candidate`, `memory`).
- Python SDK (full surface for alpha commands).
- TypeScript SDK (full surface for alpha commands).
- A minimal Studio: chat, Approval Center, Audit Trail, Memory Review.

**Decision gate.**

- The alpha demo from §10 (steps 1–11; replay deferred to Phase 5) can be executed end-to-end on a fresh machine in under 10 minutes by a developer with no prior knowledge.

**Risks.**

- **Schema churn.** Holding the schema stable is hard while filling in commands. Mitigation: every schema change requires a migration file and a corresponding update to `02-data-model.sql`. The schema in the spec is *the* schema.
- **Subscription correctness.** Easy to subtly drop updates. Mitigation: a dedicated property-based test suite for `subscribe` semantics from day one.
- **Studio scope creep.** The dashboard can balloon. Mitigation: Phase 1 Studio is functional, not pretty; visual polish is Phase 6.

---

## Phase 2 — Effect workers

**Duration.** 4 weeks.

**Goal.** Real workers performing real side effects via the effect queue.

**Build.**

- Effect queue: `effect`, `effect_result`, `worker`, `worker_capability`, `worker_heartbeat`, `effect_claim` tables fully wired through commands.
- Worker protocol: `claim_effect`, `heartbeat`, `start_effect`, `stream_observation`, `complete_effect`.
- Reference workers (each a separate small binary or library):
  - `actant-worker-model`: HTTP calls to OpenAI / Anthropic / local OpenAI-compatible endpoints.
  - `actant-worker-shell`: spawn child processes with policy-aware sandboxing.
  - `actant-worker-file`: read/write under approved patterns.
  - `actant-worker-mcp`: bridge to MCP servers for any registered MCP tool.
- Idempotency-key plumbing: stored on `effect`, surfaced to workers, recommended pattern documented for external APIs.
- Worker heartbeat → Studio "Workers" panel.

**Extended primitives landing in Phase 2** (see `/specs/14-extended-primitives.md` §17):

- **Intent + alignment + drift.** `intent` table, `form_intent` command, intent–action alignment check in Guard, `drift_signal` recording. Workers cannot claim effects whose proposal failed alignment unless an intervention overrode.
- **Observation (structured).** `observation` table; workers emit observations alongside `complete_effect`. Verification status flows through.
- **Budget + delegation.** `budget` and `delegation` tables; effect requests verify budgets. `delegate` and `consume_budget` commands.
- **Compensation plans.** Tools with `undo_capability != 'irreversible'` produce a `compensation_plan` row before the effect commits.
- **Session phase + intervention.** `session.phase` column populated by command engine; `intervene` command enters the causal graph.
- **Rich effect lease.** New columns on `effect_claim` for `input_hash`, `permission_scope_ref`, `sandbox_policy_ref`, `max_attempts`. Workers refuse to execute if input hash drifts.
- **Causal DAG.** `agent_event.causal_parent_ids` populated by commands; replay loop traverses both single-parent and DAG forms.

**Decision gate.**

- The alpha demo's shell + file steps are performed by the reference workers, not by stubbed in-process code.
- Killing a worker mid-effect produces a clean lease-loss → retry path that the next worker picks up.
- A drift scenario (agent proposes a resource outside its declared intent) triggers an `intent_action_mismatch` event and either an approval or a deny — visible in Studio.

**Risks.**

- **Worker security.** Workers run real code with real authority. Mitigation: workers carry their own actor identity, are subject to Guard, and run with OS-level isolation where possible.
- **Backpressure under load.** A burst of effects can overwhelm workers. Mitigation: per-effect-type concurrency limits on the worker side; circuit breakers in policy.
- **Intent–action alignment false positives.** A simple alignment check will block legitimate work. Mitigation: ship a "log-only" alignment mode for the first two weeks of any deployment; promote to enforcing once base rate is understood.

---

## Phase 3 — Context and memory

**Duration.** 4 weeks.

**Goal.** Context manifests, the context firewall, and the full memory lifecycle described in `06-context-and-memory.md`.

**Build.**

- `context_build`, `context_item` tables fully populated by `build_context`.
- The four-stage build pipeline: gather, score, firewall, redact, truncate.
- Default scorer (weighted recency + semantic similarity + pin signal).
- Default redaction passes (secret patterns, basic PII).
- Memory commands: `propose_memory`, `approve_memory`, `reject_memory`, `edit_memory`, `record_memory_use`, `restrict_memory`, `expire_memory`, `revoke_memory`, `delete_memory`.
- Embedding workflow: a `memory.embed` effect, an `embedding_ref` row, integration with at least one vector store (Phase 3 ships LanceDB; Qdrant adapter follows in Phase 6).
- Studio: Context Inspector, Memory Review with provenance graph.

**Decision gate.**

- A context build for the alpha demo runs end-to-end, the `~/.ssh/config` block path is visible in the Context Inspector, and the memory candidate from §11 of the demo lands correctly.
- Memory deletion zeroes the embedding and the `memory.text` column.

**Risks.**

- **Scoring quality.** A weak default scorer means bad memories get into prompts. Mitigation: ship a small benchmark task set with the SDK; track recall@k over time.
- **PII redaction false positives.** Aggressive redaction hurts quality. Mitigation: redaction is opt-in per workspace; default-off for personal mode.

---

## Phase 4 — Workflows

**Duration.** 4–6 weeks.

**Goal.** Durable workflows that survive restarts and gate on approval.

**Build.**

- `workflow`, `workflow_node`, `workflow_edge`, `workflow_run`, `workflow_step_run`, `trigger` tables and commands.
- Workflow definition format (YAML or DSL; Phase 4 picks one — proposal: a small DSL embedded in YAML).
- Node types from `07-workflows-and-replay.md` §2: `agent_task`, `model_call`, `tool_call`, `approval_gate`, `human_task`, `condition`, `parallel_group`, `memory_write`, `file_operation`, `delay`, `subworkflow`. (`browser_action`, `external_webhook` follow in Phase 6.)
- Retry policies per node; timeout policies per node.
- Studio: Workflow Board with per-run timeline.
- The second demo from §14 of the alpha demo (daily digest) runs end-to-end.

**Decision gate.**

- Daily digest workflow runs unattended for one week against a real inbox + calendar (test account), produces correct summaries, and pauses correctly at the approval gate.

**Risks.**

- **State-machine bugs.** Workflows are stateful and long-running; off-by-one errors here are nasty. Mitigation: model-based tests of every node-type transition.

---

## Phase 5 — Replay

**Duration.** 6–8 weeks.

**Goal.** All replay modes from `07-workflows-and-replay.md` §6.

**Build.**

- `replay_checkpoint`, `replay_run`, `replay_diff` tables and commands.
- Automatic checkpoint creation at the points named in `07` §7.
- The replay event loop with each mode implemented and tested:
  - `recorded` (reuses outputs).
  - `experimental` (re-invokes via sandboxed workers).
  - `policy` (alternate policy_id).
  - `model` (alternate model_route_id).
  - `memory` (excluded / edited memory set).
  - `tool` (mocked tools).
  - `local_only` (cloud routes forbidden).
- Studio: Replay Lab with mode picker, diff viewer, summary stats.
- Replay-scoped synthetic event storage in artifacts (Phase 5) — moves to dedicated tables in Phase 6 if needed for performance.

**Decision gate.**

- Three named replay scenarios from §12 of the alpha demo (memory, policy, experimental) run reproducibly and produce diffs that the diff viewer renders correctly.

**Risks.**

- **Snapshot size.** Memory and state snapshots grow. Mitigation: snapshots reference prior snapshots' deltas after Phase 5.
- **Determinism limits.** `mode=experimental` is inherently non-deterministic; users may misread the diff. Mitigation: explicit "this mode re-invokes the model" callouts in the UI; diffs grouped by event_type.

---

## Phase 6 — Cloud / team

**Duration.** 8–12 weeks.

**Goal.** Hosted multi-workspace deployment with the features a small team or enterprise needs.

**Build.**

- Multi-workspace and multi-tenant boundary across the server.
- SSO (OIDC) for human actors; service accounts for agents and workers.
- Team-level permissions: roles, groups, approval pools.
- Selective sync engine for local-first multi-device mode (see `01-architecture.md` §"Deployment topologies").
- Audit exports: nightly JSONL of Chronicle slices.
- Retention policies: workspace-level deletion windows.
- Studio: workspace management, team approvals, audit explorer, exports.
- Hosted deployment image (Docker, Helm chart).
- A self-hosted enterprise installation guide.

**Decision gate.**

- A self-hosted Phase 6 stack runs a multi-tenant smoke with at least 100 commands per workspace, per-workspace quotas, and no cross-tenant reads.
- Audit exports satisfy the repo's SOC 2 evidence-flow checklist.

**Risks.**

- **Operational toil.** Multi-tenant systems require care. Mitigation: ship per-workspace observability and quotas from day one of Phase 6.

---

## Cross-phase tracks

Three tracks run continuously through Phases 1–5 rather than as discrete phases.

### Documentation

Every command, table, and concept has a `docs/` page derived from `specs/` plus code comments. Phase 1 ships docs scraped automatically from `GET /v1/metadata/*`.

### Performance and capacity

A `bench/` package measures: command latency at the 50/99th percentile, subscription delivery latency, effect throughput, and replay reconstruction time. These run in CI per PR.

### Migrations

The SQLite schema needs a migration runner from Phase 1 day one. Every schema change is a numbered migration file. Phase 6 adds a Postgres backend with the same migration system.

---

## Risk register (consolidated)

| Risk                              | Likelihood | Impact | Mitigation                                                                 |
| --------------------------------- | ---------- | ------ | -------------------------------------------------------------------------- |
| Schema churn in Phase 1           | high       | medium | Spec is the schema; migrations required for every change.                  |
| Subscription correctness          | medium     | high   | Property-based tests from day one.                                         |
| Worker security                   | medium     | high   | Workers are Guard-checked actors with OS isolation.                        |
| Scoring quality (memory)          | high       | medium | Benchmark task set; recall@k tracked.                                      |
| Replay determinism                | high       | medium | UI calls out non-deterministic modes; diffs grouped by type.               |
| Multi-tenant ops in Phase 6       | medium     | high   | Per-workspace observability and quotas from start.                         |
| LLM provider drift                | high       | low    | Workers behind an abstraction; provider changes localized to one crate.    |
| Specification drift from code     | medium     | medium | Specs include verification checklists; PRs link the section they satisfy.  |

---

## Open questions deferred past Phase 0

These are explicitly *not* decided in Phase 0 and tracked as ADRs to write during Phase 1:

1. Workflow definition format: YAML+DSL vs JSON vs a Rust-typed builder. (`ADR-004`)
2. Inline vs out-of-band payload threshold (when does a `payload_inline` become a `payload_ref` artifact?). (`ADR-005`)
3. Phase 1 vector store default: LanceDB vs SQLite vector ext vs pgvector. (`ADR-006`)
4. Whether `agent_event` rows should be partitioned by workspace from day one. (`ADR-007`)
5. Cloud control-plane multi-region story. (`ADR-008`)

---

## Verification

- [ ] Every phase ends at a decision gate with a binary pass/fail criterion.
- [ ] Every risk in the consolidated register has a named mitigation.
- [ ] No phase depends on work from a later phase.
- [ ] Cross-phase tracks (docs, performance, migrations) appear in every phase that produces code.
