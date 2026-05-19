-- ============================================================================
-- 0005_substrate_parity.sql -- Postgres parity migration for the SQLite-only
-- substrate tables (GAPS.md row #5).
--
-- Brings the Postgres backend's CREATE TABLE set to full parity with
-- /migrations/[0-9]*.sql so the `migrations-parity` CI job stops reporting
-- a gap. 35 tables in total: every CREATE TABLE present in the SQLite
-- migrations but missing from the earlier Postgres files lands here.
--
-- Conventions (match pg/0002-0004, not pg/0001):
--   * TEXT for IDs and timestamps (RFC3339 strings). Keeps the bound types
--     identical between SQLite and Postgres so a single Rust `&str` works.
--   * INTEGER for boolean 0/1 columns. Matches both the SQLite source and
--     the pg/0002+ precedent; the existing pg_repo binders all bind raw
--     integers without a Postgres-specific BOOLEAN conversion.
--   * REAL stays as REAL on the wire; pg accepts it as DOUBLE PRECISION's
--     synonym (`real` is a Postgres type alias for 4-byte float, but the
--     existing pg files use the same shorthand and sqlx handles both).
--   * `CREATE TABLE foo` (bare identifier) -- the parity-check sed regex in
--     .github/workflows/ci.yml does not tolerate `IF NOT EXISTS` or quoting,
--     so every CREATE TABLE here is the bare form.
--
-- Runtime coverage note:
--   The 13 SQLite repo methods in crates/actant-storage/src/repo.rs all
--   match a counterpart in crates/actant-storage/src/pg_repo.rs. None of
--   the 35 tables added in this migration are touched by an existing
--   SQLite repo method, so no PgStorage methods need to be added now --
--   schema parity (this file) is what closes GAPS row #5.
--
--   Specific deferrals (worker / background populated, no repo method on
--   either side yet -- schema present, repo lands when the matching SQLite
--   repo method does):
--     - worker, worker_capability, worker_heartbeat
--     - effect, effect_claim, effect_result
--     - replay_checkpoint, replay_run, replay_diff
--     - workflow, workflow_node, workflow_edge, workflow_run,
--       workflow_step_run, agent_task
--     - approval_request, audit_event, trigger
--     - model_provider, model_route, model_call,
--       context_build, context_item
--     - memory, memory_candidate, memory_use, embedding_ref
--     - tool, tool_call, tool_schema_version
--     - policy, authority_scope, actor_identity, secret_ref, artifact
--
--   FK targets that exist in SQLite via 0001-0003 but not in pg/0001 are
--   declared here. References to tables defined earlier in pg/0001 (e.g.
--   workspace, actor, session, command_record, agent_event) keep their
--   FKs; references to tables that pg/0001 omits but pg/0005 introduces
--   are wired forward-only within this file.
-- ============================================================================

-- ----------------------------------------------------------------------------
-- 1. Identity / policy / authority
-- ----------------------------------------------------------------------------

CREATE TABLE actor_identity (
    id              TEXT PRIMARY KEY,
    actor_id        TEXT NOT NULL REFERENCES actor(id),
    provider        TEXT NOT NULL,
    subject         TEXT NOT NULL,
    secret_ref      TEXT,
    created_at      TEXT NOT NULL,
    revoked_at      TEXT,
    UNIQUE (provider, subject)
);

CREATE TABLE policy (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    name                TEXT NOT NULL,
    version             INTEGER NOT NULL,
    body_ref            TEXT NOT NULL,
    body_hash           TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    activated_at        TEXT,
    retired_at          TEXT,
    UNIQUE (workspace_id, name, version)
);

CREATE TABLE authority_scope (
    id                    TEXT PRIMARY KEY,
    workspace_id          TEXT NOT NULL REFERENCES workspace(id),
    actor_id              TEXT NOT NULL REFERENCES actor(id),
    permission            TEXT NOT NULL,
    resource_pattern      TEXT,
    sensitivity_ceiling   TEXT NOT NULL,
    allowed_actions       TEXT NOT NULL,
    granted_by_actor_id   TEXT REFERENCES actor(id),
    expires_at            TEXT,
    revoked_at            TEXT,
    created_at            TEXT NOT NULL
);

CREATE INDEX idx_authority_actor   ON authority_scope(actor_id);
CREATE INDEX idx_authority_perm    ON authority_scope(workspace_id, permission);

-- ----------------------------------------------------------------------------
-- 2. Models, context, model calls
-- ----------------------------------------------------------------------------

CREATE TABLE model_provider (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    name            TEXT NOT NULL,
    base_url        TEXT,
    secret_ref      TEXT,
    created_at      TEXT NOT NULL,
    disabled_at     TEXT
);

CREATE TABLE model_route (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    name                TEXT NOT NULL,
    provider_id         TEXT NOT NULL REFERENCES model_provider(id),
    model_name          TEXT NOT NULL,
    visibility_required TEXT NOT NULL,
    cost_per_input_1k   REAL,
    cost_per_output_1k  REAL,
    created_at          TEXT NOT NULL,
    retired_at          TEXT
);

CREATE TABLE context_build (
    id                    TEXT PRIMARY KEY,
    workspace_id          TEXT NOT NULL REFERENCES workspace(id),
    session_id            TEXT REFERENCES session(id),
    model_call_id         TEXT,
    policy_id             TEXT NOT NULL REFERENCES policy(id),
    purpose               TEXT NOT NULL,
    token_budget          INTEGER NOT NULL,
    final_prompt_ref      TEXT,
    final_prompt_hash     TEXT,
    redaction_summary     TEXT,
    blocked_item_count    INTEGER NOT NULL DEFAULT 0,
    included_item_count   INTEGER NOT NULL DEFAULT 0,
    created_at            TEXT NOT NULL
);

CREATE TABLE context_item (
    id                    TEXT PRIMARY KEY,
    context_build_id      TEXT NOT NULL REFERENCES context_build(id),
    source_type           TEXT NOT NULL,
    source_id             TEXT NOT NULL,
    included              INTEGER NOT NULL,
    blocked_reason        TEXT,
    sensitivity           TEXT NOT NULL,
    token_count           INTEGER,
    rank_score            REAL,
    reason_included       TEXT,
    visibility            TEXT NOT NULL,
    content_hash          TEXT
);

CREATE INDEX idx_ctx_item_build ON context_item(context_build_id);

CREATE TABLE model_call (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    session_id          TEXT REFERENCES session(id),
    actor_id            TEXT NOT NULL REFERENCES actor(id),
    route_id            TEXT NOT NULL REFERENCES model_route(id),
    context_build_id    TEXT NOT NULL REFERENCES context_build(id),
    purpose             TEXT NOT NULL,
    status              TEXT NOT NULL,
    request_ref         TEXT,
    response_ref        TEXT,
    request_hash        TEXT,
    response_hash       TEXT,
    input_tokens        INTEGER,
    output_tokens       INTEGER,
    cost_usd            REAL,
    latency_ms          INTEGER,
    error               TEXT,
    created_at          TEXT NOT NULL,
    completed_at        TEXT
);

-- ----------------------------------------------------------------------------
-- 3. Tools and tool calls
-- ----------------------------------------------------------------------------

CREATE TABLE tool (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    name                TEXT NOT NULL,
    kind                TEXT NOT NULL,
    required_permission TEXT NOT NULL,
    default_risk_level  TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    retired_at          TEXT,
    UNIQUE (workspace_id, name)
);

CREATE TABLE tool_schema_version (
    id                TEXT PRIMARY KEY,
    tool_id           TEXT NOT NULL REFERENCES tool(id),
    version           INTEGER NOT NULL,
    input_schema_ref  TEXT NOT NULL,
    output_schema_ref TEXT,
    created_at        TEXT NOT NULL,
    UNIQUE (tool_id, version)
);

CREATE TABLE tool_call (
    id                    TEXT PRIMARY KEY,
    workspace_id          TEXT NOT NULL REFERENCES workspace(id),
    session_id            TEXT REFERENCES session(id),
    requested_by_actor_id TEXT NOT NULL REFERENCES actor(id),
    tool_id               TEXT NOT NULL REFERENCES tool(id),
    schema_version        INTEGER NOT NULL,
    arguments_ref         TEXT,
    arguments_inline      TEXT,
    arguments_hash        TEXT NOT NULL,
    status                TEXT NOT NULL,
    risk_level            TEXT NOT NULL,
    approval_request_id   TEXT,
    effect_id             TEXT,
    result_ref            TEXT,
    result_hash           TEXT,
    error                 TEXT,
    created_at            TEXT NOT NULL,
    completed_at          TEXT
);

-- ----------------------------------------------------------------------------
-- 4. Workers (forward-declared so effect can FK to worker(id))
-- ----------------------------------------------------------------------------

CREATE TABLE worker (
    id                TEXT PRIMARY KEY,
    workspace_id      TEXT NOT NULL REFERENCES workspace(id),
    actor_id          TEXT NOT NULL REFERENCES actor(id),
    name              TEXT NOT NULL,
    host              TEXT,
    version           TEXT,
    status            TEXT NOT NULL,
    last_heartbeat_at TEXT,
    created_at        TEXT NOT NULL,
    disabled_at       TEXT
);

CREATE TABLE worker_capability (
    id              TEXT PRIMARY KEY,
    worker_id       TEXT NOT NULL REFERENCES worker(id),
    effect_type     TEXT NOT NULL,
    UNIQUE (worker_id, effect_type)
);

CREATE TABLE worker_heartbeat (
    id              TEXT PRIMARY KEY,
    worker_id       TEXT NOT NULL REFERENCES worker(id),
    at              TEXT NOT NULL,
    in_flight_count INTEGER NOT NULL,
    cpu_pct         REAL,
    mem_mb          REAL
);

-- ----------------------------------------------------------------------------
-- 5. Effects, claims, approvals
-- ----------------------------------------------------------------------------

CREATE TABLE effect (
    id                      TEXT PRIMARY KEY,
    workspace_id            TEXT NOT NULL REFERENCES workspace(id),
    command_id              TEXT NOT NULL REFERENCES command_record(id),
    requested_by_actor_id   TEXT NOT NULL REFERENCES actor(id),
    effect_type             TEXT NOT NULL,
    status                  TEXT NOT NULL,
    required_permission     TEXT,
    risk_level              TEXT NOT NULL,
    idempotency_key         TEXT,
    input_ref               TEXT,
    input_inline            TEXT,
    input_hash              TEXT NOT NULL,
    assigned_worker_id      TEXT REFERENCES worker(id),
    attempt_count           INTEGER NOT NULL DEFAULT 0,
    max_attempts            INTEGER NOT NULL DEFAULT 3,
    next_attempt_at         TEXT,
    started_at              TEXT,
    finished_at             TEXT,
    result_ref              TEXT,
    result_hash             TEXT,
    error                   TEXT,
    created_at              TEXT NOT NULL,
    UNIQUE (workspace_id, idempotency_key)
);

CREATE INDEX idx_effect_pending ON effect(workspace_id, status, next_attempt_at);

CREATE TABLE effect_result (
    id              TEXT PRIMARY KEY,
    effect_id       TEXT NOT NULL REFERENCES effect(id),
    attempt_number  INTEGER NOT NULL,
    succeeded       INTEGER NOT NULL,
    output_ref      TEXT,
    output_hash     TEXT,
    error           TEXT,
    started_at      TEXT NOT NULL,
    finished_at     TEXT NOT NULL
);

-- Mirrors SQLite 0001 plus the four columns added in SQLite 0002 (effect
-- lease richness). The SQLite migration uses ALTER TABLE statements; we
-- inline the columns here since pg/0005 is the first migration to define
-- effect_claim for Postgres.
CREATE TABLE effect_claim (
    id                      TEXT PRIMARY KEY,
    effect_id               TEXT NOT NULL REFERENCES effect(id),
    worker_id               TEXT NOT NULL REFERENCES worker(id),
    claimed_at              TEXT NOT NULL,
    expires_at              TEXT NOT NULL,
    released_at             TEXT,
    input_hash              TEXT,
    permission_scope_ref    TEXT,
    sandbox_policy_ref      TEXT,
    max_attempts            INTEGER,
    UNIQUE (effect_id, worker_id, claimed_at)
);

CREATE TABLE approval_request (
    id                    TEXT PRIMARY KEY,
    workspace_id          TEXT NOT NULL REFERENCES workspace(id),
    effect_id             TEXT REFERENCES effect(id),
    tool_call_id          TEXT REFERENCES tool_call(id),
    requested_by_actor_id TEXT NOT NULL REFERENCES actor(id),
    risk_level            TEXT NOT NULL,
    required_permission   TEXT NOT NULL,
    summary               TEXT NOT NULL,
    redacted_input_ref    TEXT,
    policy_snapshot_ref   TEXT,
    status                TEXT NOT NULL,
    created_at            TEXT NOT NULL,
    expires_at            TEXT,
    approved_by_actor_id  TEXT REFERENCES actor(id),
    approved_at           TEXT,
    denied_reason         TEXT,
    scope_granted         TEXT
);

-- ----------------------------------------------------------------------------
-- 6. Memory
-- ----------------------------------------------------------------------------

-- memory_candidate is defined before memory because memory.source_candidate_id
-- references memory_candidate. Mirrors SQLite 0001 plus the capsule_id column
-- added in SQLite 0002 (inlined here for parity).
CREATE TABLE memory_candidate (
    id                    TEXT PRIMARY KEY,
    workspace_id          TEXT NOT NULL REFERENCES workspace(id),
    proposed_by_actor_id  TEXT NOT NULL REFERENCES actor(id),
    source_event_ids      TEXT NOT NULL,
    text                  TEXT NOT NULL,
    category              TEXT NOT NULL,
    confidence            REAL NOT NULL,
    sensitivity           TEXT NOT NULL,
    status                TEXT NOT NULL,
    review_reason         TEXT,
    created_at            TEXT NOT NULL,
    capsule_id            TEXT
);

-- memory inlines SQLite 0001's columns plus the four added in SQLite 0002
-- (allowed_contexts, forbidden_contexts, last_verified_at, capsule_id).
CREATE TABLE memory (
    id                    TEXT PRIMARY KEY,
    workspace_id          TEXT NOT NULL REFERENCES workspace(id),
    text                  TEXT NOT NULL,
    category              TEXT NOT NULL,
    sensitivity           TEXT NOT NULL,
    confidence            REAL,
    scope                 TEXT NOT NULL,
    source_candidate_id   TEXT REFERENCES memory_candidate(id),
    source_event_ids      TEXT NOT NULL,
    embedding_ref_id      TEXT,
    usage_count           INTEGER NOT NULL DEFAULT 0,
    last_used_at          TEXT,
    expires_at            TEXT,
    revoked_at            TEXT,
    deleted_at            TEXT,
    created_at            TEXT NOT NULL,
    allowed_contexts      TEXT,
    forbidden_contexts    TEXT,
    last_verified_at      TEXT,
    capsule_id            TEXT
);

CREATE INDEX idx_memory_active ON memory(workspace_id, revoked_at, deleted_at);

CREATE TABLE memory_use (
    id                    TEXT PRIMARY KEY,
    memory_id             TEXT NOT NULL REFERENCES memory(id),
    context_build_id      TEXT NOT NULL REFERENCES context_build(id),
    model_call_id         TEXT REFERENCES model_call(id),
    used_at               TEXT NOT NULL,
    outcome               TEXT,
    user_feedback         TEXT
);

-- ----------------------------------------------------------------------------
-- 7. Workflows + triggers + agent tasks
-- ----------------------------------------------------------------------------

CREATE TABLE workflow (
    id                TEXT PRIMARY KEY,
    workspace_id      TEXT NOT NULL REFERENCES workspace(id),
    name              TEXT NOT NULL,
    owner_actor_id    TEXT NOT NULL REFERENCES actor(id),
    version           INTEGER NOT NULL,
    status            TEXT NOT NULL,
    policy_id         TEXT REFERENCES policy(id),
    definition_ref    TEXT NOT NULL,
    definition_hash   TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    UNIQUE (workspace_id, name, version)
);

CREATE TABLE workflow_node (
    id                    TEXT PRIMARY KEY,
    workflow_id           TEXT NOT NULL REFERENCES workflow(id),
    node_key              TEXT NOT NULL,
    node_type             TEXT NOT NULL,
    config_ref            TEXT,
    required_permissions  TEXT,
    retry_policy          TEXT,
    timeout_policy        TEXT,
    UNIQUE (workflow_id, node_key)
);

CREATE TABLE workflow_edge (
    id              TEXT PRIMARY KEY,
    workflow_id     TEXT NOT NULL REFERENCES workflow(id),
    from_node_id    TEXT NOT NULL REFERENCES workflow_node(id),
    to_node_id      TEXT NOT NULL REFERENCES workflow_node(id),
    condition_ref   TEXT,
    order_index     INTEGER
);

CREATE TABLE workflow_run (
    id                  TEXT PRIMARY KEY,
    workflow_id         TEXT NOT NULL REFERENCES workflow(id),
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    trigger_event_id    TEXT REFERENCES agent_event(id),
    status              TEXT NOT NULL,
    current_node_ids    TEXT,
    summary             TEXT,
    started_at          TEXT NOT NULL,
    finished_at         TEXT
);

CREATE TABLE workflow_step_run (
    id                  TEXT PRIMARY KEY,
    workflow_run_id     TEXT NOT NULL REFERENCES workflow_run(id),
    node_id             TEXT NOT NULL REFERENCES workflow_node(id),
    status              TEXT NOT NULL,
    attempt_number      INTEGER NOT NULL DEFAULT 1,
    effect_id           TEXT REFERENCES effect(id),
    approval_request_id TEXT REFERENCES approval_request(id),
    started_at          TEXT,
    finished_at         TEXT,
    output_ref          TEXT,
    output_hash         TEXT,
    error               TEXT
);

-- `trigger` is classified "non-reserved (cannot be function or type)" in
-- PostgreSQL, which means it parses cleanly as a table identifier without
-- quoting. The parity-check CI regex requires the bare form.
CREATE TABLE trigger (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    workflow_id     TEXT NOT NULL REFERENCES workflow(id),
    kind            TEXT NOT NULL,
    config_ref      TEXT NOT NULL,
    enabled         INTEGER NOT NULL,
    created_at      TEXT NOT NULL
);

CREATE TABLE agent_task (
    id                   TEXT PRIMARY KEY,
    workspace_id         TEXT NOT NULL REFERENCES workspace(id),
    workflow_run_id      TEXT REFERENCES workflow_run(id),
    assigned_to_actor_id TEXT REFERENCES actor(id),
    title                TEXT NOT NULL,
    description_ref      TEXT,
    status               TEXT NOT NULL,
    priority             INTEGER NOT NULL DEFAULT 0,
    due_at               TEXT,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL
);

-- ----------------------------------------------------------------------------
-- 8. Artifacts
-- ----------------------------------------------------------------------------

-- artifact inlines SQLite 0001's columns plus the capsule_id column added
-- in SQLite 0002.
CREATE TABLE artifact (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    kind                TEXT NOT NULL,
    uri                 TEXT NOT NULL,
    content_hash        TEXT NOT NULL,
    bytes               INTEGER,
    sensitivity         TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL REFERENCES actor(id),
    created_at          TEXT NOT NULL,
    deleted_at          TEXT,
    capsule_id          TEXT
);

-- ----------------------------------------------------------------------------
-- 9. Replay
-- ----------------------------------------------------------------------------

CREATE TABLE replay_checkpoint (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    event_id                    TEXT NOT NULL REFERENCES agent_event(id),
    session_id                  TEXT REFERENCES session(id),
    workflow_run_id             TEXT REFERENCES workflow_run(id),
    context_build_id            TEXT REFERENCES context_build(id),
    state_snapshot_ref          TEXT NOT NULL,
    model_route_snapshot_ref    TEXT NOT NULL,
    permission_snapshot_ref     TEXT NOT NULL,
    memory_snapshot_ref         TEXT NOT NULL,
    created_at                  TEXT NOT NULL
);

CREATE TABLE replay_run (
    id                    TEXT PRIMARY KEY,
    workspace_id          TEXT NOT NULL REFERENCES workspace(id),
    checkpoint_id         TEXT NOT NULL REFERENCES replay_checkpoint(id),
    mode                  TEXT NOT NULL,
    overrides_ref         TEXT,
    requested_by_actor_id TEXT NOT NULL REFERENCES actor(id),
    status                TEXT NOT NULL,
    started_at            TEXT NOT NULL,
    finished_at           TEXT,
    summary_ref           TEXT,
    error                 TEXT
);

CREATE TABLE replay_diff (
    id                  TEXT PRIMARY KEY,
    replay_run_id       TEXT NOT NULL REFERENCES replay_run(id),
    original_event_id   TEXT REFERENCES agent_event(id),
    replay_event_id     TEXT,
    kind                TEXT NOT NULL,
    diff_summary        TEXT,
    detail_ref          TEXT
);

-- ----------------------------------------------------------------------------
-- 10. Audit and observability
-- ----------------------------------------------------------------------------

CREATE TABLE audit_event (
    id              TEXT PRIMARY KEY,
    agent_event_id  TEXT NOT NULL REFERENCES agent_event(id),
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    actor_id        TEXT NOT NULL REFERENCES actor(id),
    event_type      TEXT NOT NULL,
    sensitivity     TEXT NOT NULL,
    decision        TEXT,
    decision_reason TEXT,
    created_at      TEXT NOT NULL
);

CREATE INDEX idx_audit_ws_time ON audit_event(workspace_id, created_at);

-- ----------------------------------------------------------------------------
-- 11. Companion store reference rows
-- ----------------------------------------------------------------------------

-- embedding_ref inlines SQLite 0001's columns plus the eleven columns added
-- in SQLite 0003 (provider/model/embedding_space/etc.). Putting them in the
-- table definition is cleaner than emitting eleven ALTER TABLEs the way
-- pg/0003 does (whose ALTERs would also fail today because embedding_ref
-- didn't exist before this file).
CREATE TABLE embedding_ref (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    object_type         TEXT NOT NULL,
    object_id           TEXT NOT NULL,
    embedding_model     TEXT NOT NULL,
    vector_store        TEXT NOT NULL,
    vector_id           TEXT NOT NULL,
    sensitivity         TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    expires_at          TEXT,
    deleted_at          TEXT,
    chunk_id            TEXT,
    provider            TEXT NOT NULL DEFAULT 'unknown',
    model               TEXT,
    model_version       TEXT,
    embedding_space_id  TEXT,
    dimension           INTEGER,
    distance_metric     TEXT,
    input_type          TEXT,
    chunker_version     TEXT,
    redaction_version   TEXT,
    source_hash         TEXT
);

CREATE INDEX idx_embedding_object ON embedding_ref(object_type, object_id);

CREATE TABLE secret_ref (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    provider        TEXT NOT NULL,
    handle          TEXT NOT NULL,
    scope           TEXT NOT NULL,
    description     TEXT,
    created_at      TEXT NOT NULL,
    last_used_at    TEXT,
    revoked_at      TEXT
);
