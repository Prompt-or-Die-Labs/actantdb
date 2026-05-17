-- ============================================================================
-- 0002_extended_primitives.sql — Phase 2+ schema delta
--
-- Source of truth: /specs/14-extended-primitives.md
-- See also: /specs/13-actant-contract.md for the framing,
--           /specs/02-data-model.sql for the section-15 mirror.
--
-- This migration introduces 13 new tables and adds cross-cutting columns
-- to several existing tables. All additions are non-destructive. Existing
-- Phase 1 commands continue to work; Phase 2+ commands begin to use the
-- new shape.
-- ============================================================================

-- ----------------------------------------------------------------------------
-- 1. Intent
-- ----------------------------------------------------------------------------

CREATE TABLE intent (
    id                              TEXT PRIMARY KEY,
    workspace_id                    TEXT NOT NULL REFERENCES workspace(id),
    actor_id                        TEXT NOT NULL REFERENCES actor(id),
    session_id                      TEXT REFERENCES session(id),
    workflow_run_id                 TEXT REFERENCES workflow_run(id),
    goal                            TEXT NOT NULL,
    proposed_action_class           TEXT NOT NULL,
    resource_targets                TEXT NOT NULL,
    expected_benefit                TEXT,
    expected_risk                   TEXT NOT NULL,
    policy_hint                     TEXT,
    created_from_context_build_id   TEXT REFERENCES context_build(id),
    status                          TEXT NOT NULL,
    created_at                      TEXT NOT NULL,
    closed_at                       TEXT
);

CREATE INDEX idx_intent_session ON intent(session_id, status);
CREATE INDEX idx_intent_actor   ON intent(actor_id, status);

-- ----------------------------------------------------------------------------
-- 2. Observation (structured)
-- ----------------------------------------------------------------------------

CREATE TABLE observation (
    id                      TEXT PRIMARY KEY,
    workspace_id            TEXT NOT NULL REFERENCES workspace(id),
    source_effect_id        TEXT NOT NULL REFERENCES effect(id),
    evidence_type           TEXT NOT NULL,
    summary                 TEXT NOT NULL,
    raw_artifact_ref        TEXT,
    confidence              REAL NOT NULL DEFAULT 1.0,
    sensitivity             TEXT NOT NULL,
    capsule_id              TEXT,   -- FK to capsule(id) added after capsule table below
    created_by_worker_id    TEXT REFERENCES worker(id),
    verification_status     TEXT NOT NULL,
    created_at              TEXT NOT NULL
);

CREATE INDEX idx_observation_source ON observation(source_effect_id);

-- ----------------------------------------------------------------------------
-- 3. Capsule + capsule_membership
-- ----------------------------------------------------------------------------

CREATE TABLE capsule (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    name                        TEXT NOT NULL,
    created_by_actor_id         TEXT NOT NULL REFERENCES actor(id),
    sensitivity                 TEXT NOT NULL,
    visibility                  TEXT NOT NULL,
    sync_policy                 TEXT NOT NULL,
    retention_policy            TEXT NOT NULL,
    redaction_policy            TEXT,
    cloud_model_allowed         INTEGER NOT NULL,
    memory_allowed              TEXT NOT NULL,
    upgrades_to_sensitivity     TEXT,
    created_at                  TEXT NOT NULL,
    retired_at                  TEXT
);

CREATE TABLE capsule_membership (
    id              TEXT PRIMARY KEY,
    capsule_id      TEXT NOT NULL REFERENCES capsule(id),
    object_type     TEXT NOT NULL,
    object_id       TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    UNIQUE (capsule_id, object_type, object_id)
);

CREATE INDEX idx_capsule_membership_object ON capsule_membership(object_type, object_id);

-- Now that capsule exists, observation.capsule_id can reference it via constraint.
-- SQLite cannot ALTER an existing column to add a foreign key, so we accept the
-- declared FK comment above and rely on the model layer to enforce it. New
-- inserts go through actant-storage which checks referential integrity.

-- ----------------------------------------------------------------------------
-- 4. Delegation
-- ----------------------------------------------------------------------------

CREATE TABLE delegation (
    id                      TEXT PRIMARY KEY,
    workspace_id            TEXT NOT NULL REFERENCES workspace(id),
    parent_actor_id         TEXT NOT NULL REFERENCES actor(id),
    child_actor_id          TEXT NOT NULL REFERENCES actor(id),
    goal                    TEXT NOT NULL,
    allowed_context_refs    TEXT NOT NULL,
    authority_scope_ids     TEXT NOT NULL,
    budget_id               TEXT,
    deadline                TEXT,
    return_channel          TEXT NOT NULL,
    status                  TEXT NOT NULL,
    started_at              TEXT NOT NULL,
    ended_at                TEXT
);

CREATE INDEX idx_delegation_child ON delegation(child_actor_id, status);

-- ----------------------------------------------------------------------------
-- 5. Budget
-- ----------------------------------------------------------------------------

CREATE TABLE budget (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    actor_id            TEXT REFERENCES actor(id),
    session_id          TEXT REFERENCES session(id),
    workflow_run_id     TEXT REFERENCES workflow_run(id),
    delegation_id       TEXT REFERENCES delegation(id),
    budget_type         TEXT NOT NULL,
    limit_value         REAL NOT NULL,
    used_value          REAL NOT NULL DEFAULT 0,
    reset_policy        TEXT,
    enforcement_action  TEXT NOT NULL,
    last_reset_at       TEXT,
    created_at          TEXT NOT NULL
);

CREATE INDEX idx_budget_actor ON budget(actor_id, budget_type);

-- ----------------------------------------------------------------------------
-- 6. Regret event
-- ----------------------------------------------------------------------------

CREATE TABLE regret_event (
    id                              TEXT PRIMARY KEY,
    workspace_id                    TEXT NOT NULL REFERENCES workspace(id),
    bad_outcome_type                TEXT NOT NULL,
    causal_event_ids                TEXT NOT NULL,
    suspected_failure_mode          TEXT,
    suggested_corrective_action     TEXT,
    severity                        TEXT NOT NULL,
    status                          TEXT NOT NULL,
    created_by_actor_id             TEXT NOT NULL REFERENCES actor(id),
    resolved_by_actor_id            TEXT REFERENCES actor(id),
    created_at                      TEXT NOT NULL,
    resolved_at                     TEXT
);

-- ----------------------------------------------------------------------------
-- 7. Eval case + eval run
-- ----------------------------------------------------------------------------

CREATE TABLE eval_case (
    id                      TEXT PRIMARY KEY,
    workspace_id            TEXT NOT NULL REFERENCES workspace(id),
    name                    TEXT NOT NULL,
    source_replay_run_id    TEXT REFERENCES replay_run(id),
    source_regret_id        TEXT REFERENCES regret_event(id),
    checkpoint_id           TEXT REFERENCES replay_checkpoint(id),
    expected_behavior       TEXT NOT NULL,
    forbidden_behavior      TEXT,
    success_criteria        TEXT NOT NULL,
    policy_constraints      TEXT,
    enabled                 INTEGER NOT NULL DEFAULT 1,
    created_at              TEXT NOT NULL,
    last_run_at             TEXT,
    last_pass               INTEGER
);

CREATE TABLE eval_run (
    id                  TEXT PRIMARY KEY,
    eval_case_id        TEXT NOT NULL REFERENCES eval_case(id),
    replay_run_id       TEXT REFERENCES replay_run(id),
    started_at          TEXT NOT NULL,
    finished_at         TEXT,
    passed              INTEGER,
    failure_detail_ref  TEXT
);

-- ----------------------------------------------------------------------------
-- 8. Memory conflict
-- ----------------------------------------------------------------------------

CREATE TABLE memory_conflict (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    memory_a_id         TEXT NOT NULL REFERENCES memory(id),
    memory_b_id         TEXT NOT NULL REFERENCES memory(id),
    conflict_type       TEXT NOT NULL,
    resolution_policy   TEXT,
    last_resolved_at    TEXT,
    detected_at         TEXT NOT NULL,
    UNIQUE (memory_a_id, memory_b_id)
);

-- ----------------------------------------------------------------------------
-- 9. Intervention
-- ----------------------------------------------------------------------------

CREATE TABLE intervention (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    actor_id                    TEXT NOT NULL REFERENCES actor(id),
    target_session_id           TEXT REFERENCES session(id),
    target_workflow_run_id      TEXT REFERENCES workflow_run(id),
    target_event_id             TEXT REFERENCES agent_event(id),
    intervention_type           TEXT NOT NULL,
    patch_ref                   TEXT,
    reason                      TEXT NOT NULL,
    created_at                  TEXT NOT NULL
);

CREATE INDEX idx_intervention_session ON intervention(target_session_id);

-- ----------------------------------------------------------------------------
-- 10. Trust profile
-- ----------------------------------------------------------------------------

CREATE TABLE trust_profile (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    actor_id            TEXT NOT NULL REFERENCES actor(id),
    capability_area     TEXT NOT NULL,
    score               REAL NOT NULL,
    confidence          REAL NOT NULL,
    sample_size         INTEGER NOT NULL,
    last_updated        TEXT NOT NULL,
    evidence_ref        TEXT,
    UNIQUE (actor_id, capability_area)
);

-- ----------------------------------------------------------------------------
-- 11. Compensation plan
-- ----------------------------------------------------------------------------

CREATE TABLE compensation_plan (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    effect_id                   TEXT NOT NULL REFERENCES effect(id),
    undo_capability             TEXT NOT NULL,
    compensation_effect_type    TEXT,
    pre_state_artifact_ref      TEXT,
    created_at                  TEXT NOT NULL,
    consumed_at                 TEXT
);

CREATE INDEX idx_compensation_effect ON compensation_plan(effect_id);

-- ----------------------------------------------------------------------------
-- 12. Model route decision
-- ----------------------------------------------------------------------------

CREATE TABLE model_route_decision (
    id                      TEXT PRIMARY KEY,
    workspace_id            TEXT NOT NULL REFERENCES workspace(id),
    model_call_id           TEXT NOT NULL REFERENCES model_call(id),
    purpose                 TEXT NOT NULL,
    candidate_route_ids     TEXT NOT NULL,
    selected_route_id       TEXT NOT NULL REFERENCES model_route(id),
    selection_reason        TEXT NOT NULL,
    privacy_constraints     TEXT,
    cost_estimate_usd       REAL,
    latency_estimate_ms     INTEGER,
    fallbacks               TEXT,
    created_at              TEXT NOT NULL
);

-- ----------------------------------------------------------------------------
-- 13. Context debt
-- ----------------------------------------------------------------------------

CREATE TABLE context_debt (
    id                      TEXT PRIMARY KEY,
    context_build_id        TEXT NOT NULL REFERENCES context_build(id),
    score                   REAL NOT NULL,
    age_factor              REAL NOT NULL,
    confidence_factor       REAL NOT NULL,
    sensitivity_factor      REAL NOT NULL,
    summarization_factor    REAL NOT NULL,
    provenance_factor       REAL NOT NULL,
    review_factor           REAL NOT NULL,
    notes                   TEXT,
    created_at              TEXT NOT NULL
);

-- ----------------------------------------------------------------------------
-- 14. Autonomy drift
-- ----------------------------------------------------------------------------

CREATE TABLE drift_signal (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    session_id                  TEXT REFERENCES session(id),
    workflow_run_id             TEXT REFERENCES workflow_run(id),
    actor_id                    TEXT NOT NULL REFERENCES actor(id),
    score                       REAL NOT NULL,
    components                  TEXT NOT NULL,
    triggered_intervention_id   TEXT REFERENCES intervention(id),
    created_at                  TEXT NOT NULL
);

-- ----------------------------------------------------------------------------
-- 15. Effect lease (rich form) — extend effect_claim
-- ----------------------------------------------------------------------------

ALTER TABLE effect_claim ADD COLUMN input_hash             TEXT;
ALTER TABLE effect_claim ADD COLUMN permission_scope_ref   TEXT;
ALTER TABLE effect_claim ADD COLUMN sandbox_policy_ref     TEXT;
ALTER TABLE effect_claim ADD COLUMN max_attempts           INTEGER;

-- ----------------------------------------------------------------------------
-- 16. Cross-cutting column additions
-- ----------------------------------------------------------------------------

ALTER TABLE agent_event       ADD COLUMN causal_parent_ids    TEXT;
ALTER TABLE session           ADD COLUMN phase                TEXT NOT NULL DEFAULT 'idle';

ALTER TABLE tool              ADD COLUMN undo_capability      TEXT NOT NULL DEFAULT 'irreversible';
ALTER TABLE tool              ADD COLUMN risk_classifier_ref  TEXT;

ALTER TABLE memory            ADD COLUMN allowed_contexts     TEXT;
ALTER TABLE memory            ADD COLUMN forbidden_contexts   TEXT;
ALTER TABLE memory            ADD COLUMN last_verified_at     TEXT;

ALTER TABLE context_item      ADD COLUMN capsule_id           TEXT;
ALTER TABLE artifact          ADD COLUMN capsule_id           TEXT;
ALTER TABLE memory            ADD COLUMN capsule_id           TEXT;
ALTER TABLE memory_candidate  ADD COLUMN capsule_id           TEXT;

ALTER TABLE workspace         ADD COLUMN drift_threshold      REAL NOT NULL DEFAULT 0.7;

-- ----------------------------------------------------------------------------
-- Verification
-- ----------------------------------------------------------------------------
--  * Every table in /specs/14-extended-primitives.md is present here.
--  * Every ALTER TABLE here is referenced in section 16 of that file.
--  * No table is dropped or renamed; Phase 1 schema is unchanged.
--  * Capsule lineage FKs are documented but unconstrained at the SQLite layer
--    (SQLite cannot add a FK to an existing column without table rebuild);
--    enforcement lives in actant-storage's model layer.
