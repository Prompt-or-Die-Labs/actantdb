# 14 — Extended primitives

This document specifies the **new first-class primitives** introduced by the Actant Contract (`specs/13-actant-contract.md`). They extend — they do not replace — the Phase 1 model in specs 00–12.

Each primitive section follows the same shape: **purpose, schema, commands, events, invariants**.

Sections:

1. Intent
2. Observation (structured)
3. Capsule + sensitivity lineage
4. Delegation
5. Budget
6. Regret event
7. Eval case
8. Memory conflict
9. Intervention
10. Trust profile
11. Compensation plan
12. Model route decision
13. Context debt
14. Autonomy drift
15. Effect lease (rich form)
16. Cross-cutting column additions
17. Phase staging

---

## 1. Intent

### Purpose

Separate **desire** from **action**. Before a tool call enters Guard, the agent declares an intent. Guard's evaluation then includes an **intent–action alignment** check: is the proposed effect plausibly within the declared intent's scope?

This is the layer that catches "agent declared `inspect tests`, proposed `rm -rf ~/.ssh`."

### Schema

```sql
CREATE TABLE intent (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    actor_id                    TEXT NOT NULL REFERENCES actor(id),
    session_id                  TEXT REFERENCES session(id),
    workflow_run_id             TEXT REFERENCES workflow_run(id),
    goal                        TEXT NOT NULL,        -- short natural-language description
    proposed_action_class       TEXT NOT NULL,        -- e.g. 'file.inspect' | 'shell.read_only' | 'memory.write'
    resource_targets            TEXT NOT NULL,        -- JSON array of glob patterns
    expected_benefit            TEXT,
    expected_risk               TEXT NOT NULL,        -- 'low' | 'medium' | 'high' | 'critical'
    policy_hint                 TEXT,
    created_from_context_build_id TEXT REFERENCES context_build(id),
    status                      TEXT NOT NULL,        -- 'open' | 'fulfilled' | 'abandoned' | 'mutated'
    created_at                  TEXT NOT NULL,
    closed_at                   TEXT
);

CREATE INDEX idx_intent_session ON intent(session_id, status);
CREATE INDEX idx_intent_actor   ON intent(actor_id, status);
```

### Commands

- `form_intent` — agent declares an intent.
- `fulfill_intent` — link an effect to the intent it satisfies.
- `mutate_intent` — agent revises a still-open intent (creates a new `intent` row with `status='mutated'` on the old).
- `abandon_intent` — mark unsatisfied (records reason).

### Events

```
intent_formed
intent_fulfilled
intent_mutated
intent_abandoned
intent_action_mismatch                  -- emitted by Guard when alignment fails
```

### Invariants

- A `tool_call` produced by an `agent`-kind actor MUST reference an open `intent_id`.
- A Guard decision of `Deny` with reason `intent_mismatch` MUST emit `intent_action_mismatch` and link to the intent + the would-be effect.
- A `fulfill_intent` command can reference at most one open intent per effect; an effect may fulfil zero or one intents.
- See §14 (Autonomy drift) for the scoring that turns repeated mismatches into a drift signal.

---

## 2. Observation (structured)

### Purpose

A tool result is not a string pasted into chat. It is structured evidence. Phase 1 emits `effect_observed` events; this primitive adds a typed projection row that memory, context, and workflows can reference as *evidence*.

### Schema

```sql
CREATE TABLE observation (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    source_effect_id    TEXT NOT NULL REFERENCES effect(id),
    evidence_type       TEXT NOT NULL,    -- 'shell_result' | 'file_content' | 'http_response'
                                          -- | 'browser_snapshot' | 'model_output' | 'human_input'
                                          -- | 'workflow_state' | 'metric'
    summary             TEXT NOT NULL,
    raw_artifact_ref    TEXT,
    confidence          REAL NOT NULL DEFAULT 1.0,
    sensitivity         TEXT NOT NULL,
    capsule_id          TEXT REFERENCES capsule(id),
    created_by_worker_id TEXT REFERENCES worker(id),
    verification_status TEXT NOT NULL,    -- 'unverified' | 'self_verified' | 'cross_verified' | 'disputed'
    created_at          TEXT NOT NULL
);

CREATE INDEX idx_observation_source ON observation(source_effect_id);
```

### Commands

- `record_observation` — worker emits a structured observation alongside `complete_effect`.
- `verify_observation` — a second actant marks the observation `cross_verified` (or `disputed`).

### Events

```
observation_recorded
observation_verified
observation_disputed
```

### Invariants

- Every observation references exactly one source effect; deleting an effect's payload tombstones its observation summary but keeps the row skeleton.
- A `memory_candidate` MAY (and SHOULD when possible) reference an `observation_id` in `source_event_ids` to back the candidate with evidence.

---

## 3. Capsule + sensitivity lineage

### Purpose

A **data capsule** bundles content with policy. Anything derived from a capsule inherits the capsule's policy. This is the mechanism behind §8 of `13-actant-contract.md`: *sensitivity travels.*

### Schema

```sql
CREATE TABLE capsule (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    name                        TEXT NOT NULL,
    created_by_actor_id         TEXT NOT NULL REFERENCES actor(id),
    sensitivity                 TEXT NOT NULL,
    visibility                  TEXT NOT NULL,        -- space-separated tags
    sync_policy                 TEXT NOT NULL,        -- 'local_only' | 'metadata_only' | 'team_sync' | 'cloud_sync' | 'hash_only' | 'encrypted_sync' | 'never_sync'
    retention_policy            TEXT NOT NULL,        -- e.g. 'P30D' | 'P1Y' | 'forever'
    redaction_policy            TEXT,
    cloud_model_allowed         INTEGER NOT NULL,     -- 0/1
    memory_allowed              TEXT NOT NULL,        -- 'true' | 'false' | 'review_only'
    upgrades_to_sensitivity     TEXT,                 -- if combined with named features, upgrade to this
    created_at                  TEXT NOT NULL,
    retired_at                  TEXT
);

CREATE TABLE capsule_membership (
    id                  TEXT PRIMARY KEY,
    capsule_id          TEXT NOT NULL REFERENCES capsule(id),
    object_type         TEXT NOT NULL,    -- 'memory' | 'context_item' | 'artifact' | 'observation' | 'message'
    object_id           TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    UNIQUE (capsule_id, object_type, object_id)
);

CREATE INDEX idx_capsule_membership_object ON capsule_membership(object_type, object_id);
```

### Commands

- `create_capsule` — define a capsule with policy.
- `attach_to_capsule` — bind a derived object to its source capsule.
- `retire_capsule` — stop new memberships; existing ones still inherit.

### Events

```
capsule_created
object_attached_to_capsule
capsule_retired
capsule_policy_blocked   -- emitted when an operation was denied because of capsule policy
```

### Invariants (lineage rules)

- When a `context_item` is derived from a source object, the context engine MUST resolve the source's capsule (if any) and copy its policy to the new item. Visibility and sensitivity propagate; the item cannot weaken either.
- When the same context build draws from multiple capsules, the result inherits the **strictest** policy across all parents.
- Sensitivity-upgrade rules in `13-actant-contract.md` §8 are realized as `capsule.upgrades_to_sensitivity` plus a workspace-level rule table (Phase 3+).

---

## 4. Delegation

### Purpose

Explicit authority transfer from a parent actor to a child (subagent). Replaces implicit "the subagent inherits everything I have."

### Schema

```sql
CREATE TABLE delegation (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    parent_actor_id             TEXT NOT NULL REFERENCES actor(id),
    child_actor_id              TEXT NOT NULL REFERENCES actor(id),
    goal                        TEXT NOT NULL,
    allowed_context_refs        TEXT NOT NULL,        -- JSON array of context_item.id
    authority_scope_ids         TEXT NOT NULL,        -- JSON array of authority_scope.id (subset of parent's)
    budget_id                   TEXT REFERENCES budget(id),
    deadline                    TEXT,
    return_channel              TEXT NOT NULL,        -- 'session' | 'workflow' | 'webhook' | 'inbox'
    status                      TEXT NOT NULL,        -- 'active' | 'completed' | 'expired' | 'revoked' | 'failed'
    started_at                  TEXT NOT NULL,
    ended_at                    TEXT
);

CREATE INDEX idx_delegation_child ON delegation(child_actor_id, status);
```

### Commands

- `delegate` — create a delegation. Validates that every `authority_scope_id` is owned by `parent_actor_id` and that the child has `actor.kind in ('subagent', 'agent')`.
- `revoke_delegation` — parent (or admin) revokes; the child loses delegated scopes immediately.
- `complete_delegation` — child reports outcome via `return_channel`.

### Events

```
delegation_started
delegation_revoked
delegation_completed
delegation_expired
delegation_authority_overrun       -- emitted when child tried to use authority outside the delegation
```

### Invariants

- A child actor's authority while a delegation is `active` is restricted to the union of its own scopes AND the delegated scopes.
- Closing a delegation MUST emit `delegation_completed` (with outcome) or `delegation_expired` (with deadline reached).
- A child cannot delegate further unless the parent granted `delegation.may_subdelegate=true` (Phase 3 column).

---

## 5. Budget

### Purpose

Agents spend more than tokens. A budget bounds **autonomy**: token, cost, time, tool-call, risk, approval, memory-write, file-write, external-message budgets. When the bound is reached, the actor halts or escalates.

### Schema

```sql
CREATE TABLE budget (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    actor_id            TEXT REFERENCES actor(id),
    session_id          TEXT REFERENCES session(id),
    workflow_run_id     TEXT REFERENCES workflow_run(id),
    delegation_id       TEXT REFERENCES delegation(id),
    budget_type         TEXT NOT NULL,    -- 'tokens' | 'cost_usd' | 'time_seconds' | 'tool_calls'
                                          -- | 'risk_score' | 'approvals' | 'memory_writes'
                                          -- | 'file_writes' | 'external_messages'
    limit_value         REAL NOT NULL,
    used_value          REAL NOT NULL DEFAULT 0,
    reset_policy        TEXT,             -- 'session' | 'daily' | 'manual' | 'never'
    enforcement_action  TEXT NOT NULL,    -- 'halt' | 'escalate' | 'warn'
    last_reset_at       TEXT,
    created_at          TEXT NOT NULL
);

CREATE INDEX idx_budget_actor ON budget(actor_id, budget_type);
```

### Commands

- `set_budget` — create or update a budget.
- `consume_budget` — atomic increment of `used_value`. Returns the remaining; raises if exceeded and `enforcement_action='halt'`.
- `reset_budget` — manual reset, or scheduled by `reset_policy`.

### Events

```
budget_set
budget_consumed                         -- emitted only for significant consumption (configurable threshold)
budget_warning                          -- crossed soft threshold (e.g. 80%)
budget_exceeded                         -- with enforcement_action
budget_reset
```

### Invariants

- Every command that requests an effect MUST verify required budgets before enqueuing. A budget overrun with `enforcement_action='halt'` produces `policy_blocked` with `decision_reason='budget_exceeded'`.
- Budget rows are write-once for `limit_value`; changing a limit creates a new `budget` row that supersedes the old.

---

## 6. Regret event

### Purpose

Capture bad outcomes as structured records that feed back into memory, policy, and evals. The system turns failure into improvement.

### Schema

```sql
CREATE TABLE regret_event (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    bad_outcome_type            TEXT NOT NULL,    -- 'user_correction' | 'tool_failure' | 'approval_denied'
                                                  -- | 'workflow_aborted' | 'memory_corrected'
                                                  -- | 'test_regression' | 'policy_violation'
                                                  -- | 'unintended_effect'
    causal_event_ids            TEXT NOT NULL,    -- JSON array
    suspected_failure_mode      TEXT,
    suggested_corrective_action TEXT,             -- JSON: { type: 'demote_memory'|'add_policy'|'update_tool_schema'|'create_eval', ... }
    severity                    TEXT NOT NULL,    -- 'low' | 'medium' | 'high' | 'critical'
    status                      TEXT NOT NULL,    -- 'open' | 'acknowledged' | 'resolved' | 'wontfix'
    created_by_actor_id         TEXT NOT NULL REFERENCES actor(id),
    resolved_by_actor_id        TEXT REFERENCES actor(id),
    created_at                  TEXT NOT NULL,
    resolved_at                 TEXT
);
```

### Commands

- `file_regret` — anyone (user, agent, evaluator) can file.
- `acknowledge_regret` — a responsible actor accepts ownership.
- `resolve_regret` — links to the action taken (memory demotion, policy change, eval creation).

### Events

```
regret_filed
regret_acknowledged
regret_resolved
regret_wontfix
```

### Invariants

- A regret's `causal_event_ids` MUST all exist and share `workspace_id`.
- A `resolve_regret` MUST reference at least one concrete corrective action (a new `policy` row, a `memory_candidate` rejection, an `eval_case`, etc.).

---

## 7. Eval case

### Purpose

Evals derived from real production traces. When a replay shows a failure mode, the system can crystallize it into an eval that runs forever after.

### Schema

```sql
CREATE TABLE eval_case (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    name                TEXT NOT NULL,
    source_replay_run_id TEXT REFERENCES replay_run(id),
    source_regret_id    TEXT REFERENCES regret_event(id),
    checkpoint_id       TEXT REFERENCES replay_checkpoint(id),
    expected_behavior   TEXT NOT NULL,
    forbidden_behavior  TEXT,
    success_criteria    TEXT NOT NULL,    -- JSON DSL (Phase 2 spec)
    policy_constraints  TEXT,
    enabled             INTEGER NOT NULL DEFAULT 1,
    created_at          TEXT NOT NULL,
    last_run_at         TEXT,
    last_pass           INTEGER             -- 0/1/NULL
);

CREATE TABLE eval_run (
    id              TEXT PRIMARY KEY,
    eval_case_id    TEXT NOT NULL REFERENCES eval_case(id),
    replay_run_id   TEXT REFERENCES replay_run(id),
    started_at      TEXT NOT NULL,
    finished_at     TEXT,
    passed          INTEGER,                -- 0/1
    failure_detail_ref TEXT
);
```

### Commands

- `create_eval_from_replay` — given a replay_run and a description, mint an eval_case.
- `run_eval` — re-execute the eval (kicks off a fresh replay under the eval's constraints).
- `disable_eval` / `enable_eval`.

### Events

```
eval_case_created
eval_run_started
eval_run_passed
eval_run_failed
```

### Invariants

- An eval whose `checkpoint_id` no longer resolves (snapshots purged) MUST be disabled and emit `eval_case_orphaned`.

---

## 8. Memory conflict

### Purpose

Two approved memories that contradict each other are first-class — not silently picked-one-of-them at retrieval time.

### Schema

```sql
CREATE TABLE memory_conflict (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    memory_a_id                 TEXT NOT NULL REFERENCES memory(id),
    memory_b_id                 TEXT NOT NULL REFERENCES memory(id),
    conflict_type               TEXT NOT NULL,    -- 'contradiction' | 'overlap' | 'supersedes' | 'context_dependent'
    resolution_policy           TEXT,             -- 'newer_wins' | 'human_review_required'
                                                  -- | 'context_specific' | 'model_decides_with_explanation'
    last_resolved_at            TEXT,
    detected_at                 TEXT NOT NULL,
    UNIQUE (memory_a_id, memory_b_id)
);
```

### Commands

- `detect_memory_conflict` — invoked by the memory engine (Phase 3+) or by a reviewer.
- `set_conflict_resolution` — choose how the engine picks between the two when both match a query.
- `resolve_conflict` — supersede one with the other.

### Events

```
memory_conflict_detected
memory_conflict_resolved
```

### Invariants

- The context engine MUST consult `memory_conflict` rows when more than one of the conflicting pair would otherwise enter a context build, and apply the `resolution_policy`.

---

## 9. Intervention

### Purpose

Human steering is a first-class command, not a side channel. Interventions enter the causal graph.

### Schema

```sql
CREATE TABLE intervention (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    actor_id            TEXT NOT NULL REFERENCES actor(id),    -- the human doing it
    target_session_id   TEXT REFERENCES session(id),
    target_workflow_run_id TEXT REFERENCES workflow_run(id),
    target_event_id     TEXT REFERENCES agent_event(id),
    intervention_type   TEXT NOT NULL,    -- 'pause' | 'cancel' | 'redirect' | 'edit_context'
                                          -- | 'deny_tool' | 'change_model' | 'add_instruction'
                                          -- | 'restrict_permission' | 'fork_replay' | 'take_over_node'
    patch_ref           TEXT,             -- artifact ref to the change payload
    reason              TEXT NOT NULL,
    created_at          TEXT NOT NULL
);
```

### Commands

- `intervene` — single entry-point for all intervention types. The handler dispatches by `intervention_type`.

### Events

```
intervention_applied
intervention_failed
```

### Invariants

- Every intervention MUST be replayable: the patch and the target are captured exactly. A replay can re-execute the run with or without the intervention.
- Interventions are subject to authority — only actors with `intervention.<type>` permission may invoke them.

---

## 10. Trust profile

### Purpose

Authority is calibrated by behavior. Trust profiles aggregate operational signals so policy can adapt.

### Schema

```sql
CREATE TABLE trust_profile (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    actor_id            TEXT NOT NULL REFERENCES actor(id),
    capability_area     TEXT NOT NULL,    -- 'shell.run' | 'file.write' | 'memory.write' | ...
    score               REAL NOT NULL,    -- 0.0 .. 1.0
    confidence          REAL NOT NULL,    -- 0.0 .. 1.0
    sample_size         INTEGER NOT NULL,
    last_updated        TEXT NOT NULL,
    evidence_ref        TEXT,             -- artifact ref to evidence summary
    UNIQUE (actor_id, capability_area)
);
```

### Commands

- `recalculate_trust` — system command, runs periodically.
- `pin_trust` — admin override (recorded as an audit event so changes are visible).

### Events

```
trust_recalculated
trust_pinned
trust_downgrade            -- crossed a threshold downward
trust_upgrade              -- crossed a threshold upward
```

### Invariants

- Guard MAY consult `trust_profile` to escalate `risk_level` on a request from an actor with low trust in the relevant area; this must be visible in `decision_reason`.

---

## 11. Compensation plan

### Purpose

Approvals should show reversibility. Some tools can be undone or compensated; others cannot. Plans are first-class so the UI and the audit story can both reason about them.

### Schema

```sql
CREATE TABLE compensation_plan (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    effect_id                   TEXT NOT NULL REFERENCES effect(id),
    undo_capability             TEXT NOT NULL,    -- 'reversible' | 'partial' | 'irreversible'
    compensation_effect_type    TEXT,             -- e.g. 'file.write' to restore prior content
    pre_state_artifact_ref      TEXT,             -- captured before the effect; required if 'reversible'
    created_at                  TEXT NOT NULL,
    consumed_at                 TEXT              -- non-null once compensation was used
);
```

### Commands

- `attach_compensation_plan` — created automatically by the command engine for tools with `undo_capability != 'irreversible'`.
- `compensate_effect` — execute the compensation effect; consumes the plan.

### Events

```
compensation_attached
compensation_consumed
compensation_failed
```

### Invariants

- Tools with `undo_capability='reversible'` MUST produce a `pre_state_artifact_ref` before the effect commits.
- A compensation plan can be consumed at most once; the row's `consumed_at` is set inside the consume transaction.

---

## 12. Model route decision

### Purpose

Choosing a model is itself an agentic decision with provenance. Already-running auditors need to know why a route was chosen on a per-call basis.

### Schema

```sql
CREATE TABLE model_route_decision (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    model_call_id       TEXT NOT NULL REFERENCES model_call(id),
    purpose             TEXT NOT NULL,
    candidate_route_ids TEXT NOT NULL,    -- JSON array
    selected_route_id   TEXT NOT NULL REFERENCES model_route(id),
    selection_reason    TEXT NOT NULL,
    privacy_constraints TEXT,
    cost_estimate_usd   REAL,
    latency_estimate_ms INTEGER,
    fallbacks           TEXT,             -- JSON array of route_ids in order
    created_at          TEXT NOT NULL
);
```

### Commands

Created implicitly by `request_model_call` once the route selector finishes. There is no user-facing command in Phase 2; Phase 4 may add `propose_route` for human override.

### Events

```
model_route_selected
model_route_fallback_used
```

### Invariants

- Every `model_call` has exactly one `model_route_decision`. The selector cannot run without producing one.

---

## 13. Context debt

### Purpose

An operational metric: how much hidden, stale, or low-confidence context the system is relying on. Surfaces as a number on a context build, with diagnostics.

### Schema

```sql
CREATE TABLE context_debt (
    id                      TEXT PRIMARY KEY,
    context_build_id        TEXT NOT NULL REFERENCES context_build(id),
    score                   REAL NOT NULL,            -- 0.0 .. 1.0, higher = more debt
    age_factor              REAL NOT NULL,
    confidence_factor       REAL NOT NULL,
    sensitivity_factor      REAL NOT NULL,
    summarization_factor    REAL NOT NULL,
    provenance_factor       REAL NOT NULL,
    review_factor           REAL NOT NULL,
    notes                   TEXT,
    created_at              TEXT NOT NULL
);
```

### Commands

Created implicitly by the context engine at the end of `build_context`.

### Events

```
context_debt_recorded
context_debt_warning           -- crossed threshold for the workspace
```

### Invariants

- Context-debt warnings are advisory in Phase 2. Phase 4 may gate model calls above a debt threshold to require human acknowledgement.

---

## 14. Autonomy drift

### Purpose

Compute a drift score from `(declared intent vs actual effects, resources touched, risk escalation, unusual tool sequence)` and react when it crosses a threshold.

### Schema

```sql
CREATE TABLE drift_signal (
    id                      TEXT PRIMARY KEY,
    workspace_id            TEXT NOT NULL REFERENCES workspace(id),
    session_id              TEXT REFERENCES session(id),
    workflow_run_id         TEXT REFERENCES workflow_run(id),
    actor_id                TEXT NOT NULL REFERENCES actor(id),
    score                   REAL NOT NULL,            -- 0.0 .. 1.0
    components              TEXT NOT NULL,            -- JSON: { intent_mismatch, resource_mismatch, risk_escalation, sequence_unusual }
    triggered_intervention_id TEXT REFERENCES intervention(id),
    created_at              TEXT NOT NULL
);
```

### Commands

- `recompute_drift` — system command; runs after each tool_call or effect.
- `enforce_drift_threshold` — Guard helper invoked when a drift_signal crosses the workspace threshold; may inject an `intervention` of type `pause` and emit `intent_action_mismatch`.

### Events

```
drift_signal_recorded
drift_threshold_crossed
drift_intervention_applied
```

### Invariants

- A drift signal at or above `workspace.drift_threshold` MUST result in either a recorded intervention or an explicit human acknowledgement.

---

## 15. Effect lease (rich form)

### Purpose

Phase 1's `effect_claim` is the minimum lease. Phase 2 extends it to bind a worker mandate explicitly.

### Schema additions

```sql
ALTER TABLE effect_claim ADD COLUMN input_hash             TEXT;
ALTER TABLE effect_claim ADD COLUMN permission_scope_ref   TEXT;  -- snapshot of authority that justified the claim
ALTER TABLE effect_claim ADD COLUMN sandbox_policy_ref     TEXT;  -- e.g. "docker:read-only", "macos:project-only"
ALTER TABLE effect_claim ADD COLUMN max_attempts           INTEGER;
```

### Invariants

- A worker's effect execution MUST hash its actual inputs and refuse to proceed if the hash differs from `effect_claim.input_hash` — preventing argument tampering between scheduling and execution.

---

## 16. Cross-cutting column additions

These are tiny but pervasive.

```sql
-- Chronicle becomes a causal DAG, not a chain.
ALTER TABLE agent_event   ADD COLUMN causal_parent_ids TEXT;          -- JSON array of agent_event.id
-- An explicit phase column on session enables agent state-machine UIs.
ALTER TABLE session       ADD COLUMN phase             TEXT NOT NULL DEFAULT 'idle';
-- Tools declare their reversibility and risk classifier.
ALTER TABLE tool          ADD COLUMN undo_capability   TEXT NOT NULL DEFAULT 'irreversible';
ALTER TABLE tool          ADD COLUMN risk_classifier_ref TEXT;        -- artifact ref to the classifier definition
-- Memory carries scope/visibility/expiry constraints used by the context firewall.
ALTER TABLE memory        ADD COLUMN allowed_contexts  TEXT;          -- JSON array
ALTER TABLE memory        ADD COLUMN forbidden_contexts TEXT;
ALTER TABLE memory        ADD COLUMN last_verified_at  TEXT;
-- Capsule lineage for derived items.
ALTER TABLE context_item  ADD COLUMN capsule_id        TEXT REFERENCES capsule(id);
ALTER TABLE artifact      ADD COLUMN capsule_id        TEXT REFERENCES capsule(id);
ALTER TABLE memory        ADD COLUMN capsule_id        TEXT REFERENCES capsule(id);
ALTER TABLE memory_candidate ADD COLUMN capsule_id     TEXT REFERENCES capsule(id);
-- Workspace-level drift threshold.
ALTER TABLE workspace     ADD COLUMN drift_threshold   REAL NOT NULL DEFAULT 0.7;
```

### Session phase enum

```
idle              awaiting_approval
planning          executing_tool
building_context  observing
calling_model     reflecting
                  writing_memory
                  blocked
                  completed
                  failed
                  cancelled
```

Stored as snake_case strings in `session.phase`. The set is closed; new phases require a migration.

---

## 17. Phase staging

The extended primitives ship across Phases 2–4. Phase 2 lands the core; Phase 3 lands lineage and self-improvement; Phase 4 lands fleet-level controls.

| Phase | Primitive                                                       | Why this phase                                                  |
| ----- | --------------------------------------------------------------- | --------------------------------------------------------------- |
| 2     | Intent + intent-action alignment + autonomy drift               | Lands with the effect-workers; needs alignment before risky tools execute. |
| 2     | Observation (structured)                                        | Lands with workers; workers emit them.                          |
| 2     | Budget + delegation                                             | Required for safe Phase 2 worker fleets.                        |
| 2     | Compensation plan                                               | Drops in alongside the file/shell workers.                      |
| 2     | Session phase column + intervention                             | Needed the moment Studio shows live sessions.                   |
| 3     | Capsule + sensitivity lineage                                   | Requires the context engine of Phase 3.                         |
| 3     | Memory conflict + memory `allowed_contexts` / `forbidden_contexts` / `last_verified_at` | Requires the memory engine of Phase 3. |
| 3     | Trust profile (initial scoring)                                 | Needs a corpus of behavior; comes when telemetry exists.        |
| 4     | Regret event + eval case                                        | Requires Flow Engine + Replay Engine, both Phase 4/5.           |
| 5     | Drift threshold gating                                          | Refined once replay is in place.                                |
| 4     | Model route decision                                            | When real routing exists (multiple cloud routes).               |
| 4     | Context debt scoring                                            | Tuned with real prompts in flight.                              |

---

## Verification

- [ ] Every table introduced here has a column-by-column counterpart in `/migrations/0002_extended_primitives.sql`.
- [ ] Every new command appears either in a Phase 2/3/4 work package or in `agents/<crate>.md`'s "expanded scope" section.
- [ ] Every new event name follows the snake_case convention of `01-architecture.md` §1.
- [ ] Every cross-cutting `ALTER TABLE` is reflected in `/specs/02-data-model.sql`.
- [ ] No primitive in this file violates any invariant in `05-security-model.md` §2 (extends only).
- [ ] Every primitive is referenced from `13-actant-contract.md` (the framing remains the parent).
