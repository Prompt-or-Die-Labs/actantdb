-- ============================================================================
-- 0001_initial.sql — initial schema migration for ActantDB
--
-- Source of truth: /specs/02-data-model.sql
-- Any change to the schema MUST happen in BOTH places, with a matching new
-- migration file under /migrations/ for the delta.
--
-- Applied by actant-storage's migration runner (sqlx::migrate!). Migrations
-- are run in lexicographic order; numbering uses 4-digit padded prefixes so
-- alphabetic order matches numeric order.
-- ============================================================================

-- ActantDB canonical data model — Phase 0 specification
--
-- Target: SQLite for alpha. Portable subset only.
--   * TEXT for all IDs (ULID/UUIDv7 strings; sortable, opaque).
--   * TEXT for all timestamps (RFC3339 UTC, e.g. '2026-05-17T14:30:00Z').
--   * INTEGER for counts, INTEGER 0/1 for booleans.
--   * No vendor-specific types. JSON payloads are TEXT and validated at the
--     command boundary (see 03-command-spec.md).
--
-- Indices are illustrative; production indices belong in migration files.
--
-- Section map:
--   1.  Core identity
--   2.  Workspaces and policy
--   3.  Sessions and messages
--   4.  Chronicle (events)
--   5.  Commands (command_record)
--   6.  Models, context, model calls
--   7.  Tools and tool calls
--   8.  Effects, workers, approvals
--   9.  Memory
--  10.  Workflows
--  11.  Artifacts
--  12.  Replay
--  13.  Audit and observability
--  14.  Companion store reference rows
-- ============================================================================

-- ----------------------------------------------------------------------------
-- 1. Core identity
-- ----------------------------------------------------------------------------

-- Every meaningful operation in ActantDB is attributed to a workspace.
-- A workspace is the unit of tenancy, policy scope, and audit.

CREATE TABLE workspace (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    archived_at     TEXT
);

-- An actor is any entity capable of action: human, agent, subagent, model,
-- tool, worker, system. The "kind" column drives Guard's authority checks.

CREATE TABLE actor (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    kind            TEXT NOT NULL,      -- 'human' | 'agent' | 'subagent'
                                        -- | 'model' | 'tool' | 'worker'
                                        -- | 'system'
    display_name    TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    disabled_at     TEXT
);

-- Identities (email, OAuth, API key, service principal) bind real-world auth
-- material to an actor. Material itself is in Secret Vault; we store only a
-- reference.
CREATE TABLE actor_identity (
    id              TEXT PRIMARY KEY,
    actor_id        TEXT NOT NULL REFERENCES actor(id),
    provider        TEXT NOT NULL,      -- 'email' | 'github' | 'apple' | ...
    subject         TEXT NOT NULL,      -- provider-specific stable id
    secret_ref      TEXT,               -- pointer into Secret Vault
    created_at      TEXT NOT NULL,
    revoked_at      TEXT,
    UNIQUE (provider, subject)
);

-- ----------------------------------------------------------------------------
-- 2. Workspaces and policy
-- ----------------------------------------------------------------------------

-- A policy is a versioned bundle of rules that Guard evaluates.
-- The bundle itself lives out-of-band (artifact_ref); we keep metadata here.
CREATE TABLE policy (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    name                TEXT NOT NULL,
    version             INTEGER NOT NULL,
    body_ref            TEXT NOT NULL,  -- artifact ref to policy document
    body_hash           TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    activated_at        TEXT,
    retired_at          TEXT,
    UNIQUE (workspace_id, name, version)
);

-- An authority scope grants one actor one permission over one resource
-- pattern up to one sensitivity ceiling. Multiple rows compose.
CREATE TABLE authority_scope (
    id                    TEXT PRIMARY KEY,
    workspace_id          TEXT NOT NULL REFERENCES workspace(id),
    actor_id              TEXT NOT NULL REFERENCES actor(id),
    permission            TEXT NOT NULL,   -- e.g. 'file.read'
    resource_pattern      TEXT,            -- e.g. '~/Projects/**'
    sensitivity_ceiling   TEXT NOT NULL,   -- 'public'|'low'|'medium'|'high'
                                           -- |'secret'|'regulated'
    allowed_actions       TEXT NOT NULL,   -- JSON array
    granted_by_actor_id   TEXT REFERENCES actor(id),
    expires_at            TEXT,
    revoked_at            TEXT,
    created_at            TEXT NOT NULL
);

CREATE INDEX idx_authority_actor   ON authority_scope(actor_id);
CREATE INDEX idx_authority_perm    ON authority_scope(workspace_id, permission);

-- ----------------------------------------------------------------------------
-- 3. Sessions and messages
-- ----------------------------------------------------------------------------

CREATE TABLE session (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    title           TEXT,
    initiator_actor_id TEXT NOT NULL REFERENCES actor(id),
    agent_actor_id  TEXT REFERENCES actor(id),
    status          TEXT NOT NULL,         -- 'active'|'paused'|'closed'
    created_at      TEXT NOT NULL,
    closed_at       TEXT
);

CREATE TABLE message (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL REFERENCES session(id),
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    author_actor_id TEXT NOT NULL REFERENCES actor(id),
    role            TEXT NOT NULL,         -- 'user'|'agent'|'tool'|'system'
    body_ref        TEXT,                  -- artifact ref for large content
    body_text       TEXT,                  -- inline for small content
    body_hash       TEXT NOT NULL,
    parent_message_id TEXT REFERENCES message(id),
    created_at      TEXT NOT NULL
);

CREATE INDEX idx_message_session ON message(session_id, created_at);

-- ----------------------------------------------------------------------------
-- 4. Chronicle (events) — the append-only causal ledger
-- ----------------------------------------------------------------------------

CREATE TABLE agent_event (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    actor_id            TEXT NOT NULL REFERENCES actor(id),
    session_id          TEXT REFERENCES session(id),
    parent_event_id     TEXT REFERENCES agent_event(id),
    event_type          TEXT NOT NULL,
    causality_kind      TEXT NOT NULL,   -- 'observation'|'intent'|'effect'
                                         -- |'control'|'audit'
    sensitivity         TEXT NOT NULL,   -- public|low|medium|high|secret|regulated
    authority_scope_id  TEXT REFERENCES authority_scope(id),
    payload_ref         TEXT,            -- artifact ref if large
    payload_inline      TEXT,            -- inline JSON if small
    payload_hash        TEXT NOT NULL,
    -- Foreign keys to other rows this event references. Any/all may be null.
    model_call_id       TEXT,
    tool_call_id        TEXT,
    workflow_run_id     TEXT,
    memory_id           TEXT,
    artifact_id         TEXT,
    command_id          TEXT,
    effect_id           TEXT,
    -- Tamper-evident chain: SHA-256(parent_hash || payload_hash || metadata).
    event_hash          TEXT NOT NULL,
    created_at          TEXT NOT NULL
);

CREATE INDEX idx_event_workspace_time ON agent_event(workspace_id, created_at);
CREATE INDEX idx_event_session_time   ON agent_event(session_id, created_at);
CREATE INDEX idx_event_parent         ON agent_event(parent_event_id);
CREATE INDEX idx_event_type           ON agent_event(workspace_id, event_type);

-- ----------------------------------------------------------------------------
-- 5. Commands (command_record)
-- ----------------------------------------------------------------------------

CREATE TABLE command_record (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    actor_id        TEXT NOT NULL REFERENCES actor(id),
    session_id      TEXT REFERENCES session(id),
    command_type    TEXT NOT NULL,
    input_ref       TEXT,
    input_inline    TEXT,                -- inline JSON if small
    input_hash      TEXT NOT NULL,
    policy_id       TEXT REFERENCES policy(id),
    status          TEXT NOT NULL,       -- 'received'|'committed'|'rejected'
    error           TEXT,
    created_at      TEXT NOT NULL,
    committed_at    TEXT
);

CREATE INDEX idx_command_workspace_time ON command_record(workspace_id, created_at);

-- ----------------------------------------------------------------------------
-- 6. Models, context, model calls
-- ----------------------------------------------------------------------------

CREATE TABLE model_provider (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    name            TEXT NOT NULL,       -- 'openai'|'anthropic'|'mlx'|...
    base_url        TEXT,
    secret_ref      TEXT,
    created_at      TEXT NOT NULL,
    disabled_at     TEXT
);

CREATE TABLE model_route (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    name                TEXT NOT NULL,   -- 'planner'|'executor'|'critic'|...
    provider_id         TEXT NOT NULL REFERENCES model_provider(id),
    model_name          TEXT NOT NULL,
    visibility_required TEXT NOT NULL,   -- minimum visibility tag a context
                                         -- item must carry to be sent here
                                         -- ('local_model_allowed' or stricter)
    cost_per_input_1k   REAL,
    cost_per_output_1k  REAL,
    created_at          TEXT NOT NULL,
    retired_at          TEXT
);

CREATE TABLE context_build (
    id                    TEXT PRIMARY KEY,
    workspace_id          TEXT NOT NULL REFERENCES workspace(id),
    session_id            TEXT REFERENCES session(id),
    model_call_id         TEXT,                  -- backref filled after call
    policy_id             TEXT NOT NULL REFERENCES policy(id),
    purpose               TEXT NOT NULL,         -- 'planner'|'executor'|...
    token_budget          INTEGER NOT NULL,
    final_prompt_ref      TEXT,                  -- artifact ref
    final_prompt_hash     TEXT,
    redaction_summary     TEXT,
    blocked_item_count    INTEGER NOT NULL DEFAULT 0,
    included_item_count   INTEGER NOT NULL DEFAULT 0,
    created_at            TEXT NOT NULL
);

CREATE TABLE context_item (
    id                    TEXT PRIMARY KEY,
    context_build_id      TEXT NOT NULL REFERENCES context_build(id),
    source_type           TEXT NOT NULL,   -- 'memory'|'message'|'file'
                                           -- |'artifact'|'tool_result'|...
    source_id             TEXT NOT NULL,
    included              INTEGER NOT NULL,   -- 0/1
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
    actor_id            TEXT NOT NULL REFERENCES actor(id),  -- requesting agent
    route_id            TEXT NOT NULL REFERENCES model_route(id),
    context_build_id    TEXT NOT NULL REFERENCES context_build(id),
    purpose             TEXT NOT NULL,
    status              TEXT NOT NULL,   -- requested|running|completed|failed|cancelled
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
-- 7. Tools and tool calls
-- ----------------------------------------------------------------------------

CREATE TABLE tool (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    name            TEXT NOT NULL,         -- 'shell.run'|'github.create_issue'|...
    kind            TEXT NOT NULL,         -- 'shell'|'file'|'browser'|'http'
                                           -- |'mcp'|'app'|...
    required_permission TEXT NOT NULL,
    default_risk_level  TEXT NOT NULL,     -- 'low'|'medium'|'high'|'critical'
    created_at      TEXT NOT NULL,
    retired_at      TEXT,
    UNIQUE (workspace_id, name)
);

CREATE TABLE tool_schema_version (
    id              TEXT PRIMARY KEY,
    tool_id         TEXT NOT NULL REFERENCES tool(id),
    version         INTEGER NOT NULL,
    input_schema_ref  TEXT NOT NULL,
    output_schema_ref TEXT,
    created_at      TEXT NOT NULL,
    UNIQUE (tool_id, version)
);

CREATE TABLE tool_call (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    session_id          TEXT REFERENCES session(id),
    requested_by_actor_id TEXT NOT NULL REFERENCES actor(id),
    tool_id             TEXT NOT NULL REFERENCES tool(id),
    schema_version      INTEGER NOT NULL,
    arguments_ref       TEXT,
    arguments_inline    TEXT,
    arguments_hash      TEXT NOT NULL,
    status              TEXT NOT NULL,    -- requested|pending_approval
                                          -- |approved|denied|running
                                          -- |completed|failed|cancelled
    risk_level          TEXT NOT NULL,
    approval_request_id TEXT,
    effect_id           TEXT,
    result_ref          TEXT,
    result_hash         TEXT,
    error               TEXT,
    created_at          TEXT NOT NULL,
    completed_at        TEXT
);

-- ----------------------------------------------------------------------------
-- 8. Effects, workers, approvals
-- ----------------------------------------------------------------------------

CREATE TABLE effect (
    id                      TEXT PRIMARY KEY,
    workspace_id            TEXT NOT NULL REFERENCES workspace(id),
    command_id              TEXT NOT NULL REFERENCES command_record(id),
    requested_by_actor_id   TEXT NOT NULL REFERENCES actor(id),
    effect_type             TEXT NOT NULL,   -- see 04-effect-protocol.md
    status                  TEXT NOT NULL,   -- pending|claimed|running
                                             -- |succeeded|failed|cancelled
                                             -- |awaiting_approval
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

CREATE TABLE worker (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    actor_id        TEXT NOT NULL REFERENCES actor(id),
    name            TEXT NOT NULL,
    host            TEXT,
    version         TEXT,
    status          TEXT NOT NULL,   -- 'online'|'draining'|'offline'
    last_heartbeat_at TEXT,
    created_at      TEXT NOT NULL,
    disabled_at     TEXT
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

CREATE TABLE effect_claim (
    id              TEXT PRIMARY KEY,
    effect_id       TEXT NOT NULL REFERENCES effect(id),
    worker_id       TEXT NOT NULL REFERENCES worker(id),
    claimed_at      TEXT NOT NULL,
    expires_at      TEXT NOT NULL,
    released_at     TEXT,
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
    status                TEXT NOT NULL,  -- pending|approved|denied|expired
                                          -- |escalated|cancelled
    created_at            TEXT NOT NULL,
    expires_at            TEXT,
    approved_by_actor_id  TEXT REFERENCES actor(id),
    approved_at           TEXT,
    denied_reason         TEXT,
    scope_granted         TEXT             -- 'once'|'session'|'scope'|'forever'
);

-- ----------------------------------------------------------------------------
-- 9. Memory
-- ----------------------------------------------------------------------------

CREATE TABLE memory_candidate (
    id                    TEXT PRIMARY KEY,
    workspace_id          TEXT NOT NULL REFERENCES workspace(id),
    proposed_by_actor_id  TEXT NOT NULL REFERENCES actor(id),
    source_event_ids      TEXT NOT NULL,   -- JSON array of agent_event.id
    text                  TEXT NOT NULL,
    category              TEXT NOT NULL,   -- 'preference'|'fact'|'goal'
                                           -- |'pattern'|'relationship'|...
    confidence            REAL NOT NULL,
    sensitivity           TEXT NOT NULL,
    status                TEXT NOT NULL,   -- 'proposed'|'pending_review'
                                           -- |'approved'|'rejected'|'edited'
    review_reason         TEXT,
    created_at            TEXT NOT NULL
);

CREATE TABLE memory (
    id                    TEXT PRIMARY KEY,
    workspace_id          TEXT NOT NULL REFERENCES workspace(id),
    text                  TEXT NOT NULL,
    category              TEXT NOT NULL,
    sensitivity           TEXT NOT NULL,
    confidence            REAL,
    scope                 TEXT NOT NULL,   -- 'global'|'session'|'project'|...
    source_candidate_id   TEXT REFERENCES memory_candidate(id),
    source_event_ids      TEXT NOT NULL,
    embedding_ref_id      TEXT,            -- pointer to embedding_ref row
    usage_count           INTEGER NOT NULL DEFAULT 0,
    last_used_at          TEXT,
    expires_at            TEXT,
    revoked_at            TEXT,
    deleted_at            TEXT,
    created_at            TEXT NOT NULL
);

CREATE INDEX idx_memory_active ON memory(workspace_id, revoked_at, deleted_at);

CREATE TABLE memory_use (
    id                    TEXT PRIMARY KEY,
    memory_id             TEXT NOT NULL REFERENCES memory(id),
    context_build_id      TEXT NOT NULL REFERENCES context_build(id),
    model_call_id         TEXT REFERENCES model_call(id),
    used_at               TEXT NOT NULL,
    outcome               TEXT,            -- 'used'|'ignored'|'rejected'
    user_feedback         TEXT
);

-- ----------------------------------------------------------------------------
-- 10. Workflows
-- ----------------------------------------------------------------------------

CREATE TABLE workflow (
    id                TEXT PRIMARY KEY,
    workspace_id      TEXT NOT NULL REFERENCES workspace(id),
    name              TEXT NOT NULL,
    owner_actor_id    TEXT NOT NULL REFERENCES actor(id),
    version           INTEGER NOT NULL,
    status            TEXT NOT NULL,       -- 'draft'|'active'|'retired'
    policy_id         TEXT REFERENCES policy(id),
    definition_ref    TEXT NOT NULL,       -- artifact ref to DAG document
    definition_hash   TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    UNIQUE (workspace_id, name, version)
);

CREATE TABLE workflow_node (
    id                    TEXT PRIMARY KEY,
    workflow_id            TEXT NOT NULL REFERENCES workflow(id),
    node_key               TEXT NOT NULL,   -- stable identifier inside the DAG
    node_type              TEXT NOT NULL,
    config_ref             TEXT,
    required_permissions   TEXT,            -- JSON array
    retry_policy           TEXT,            -- JSON object
    timeout_policy         TEXT,            -- JSON object
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
    id                    TEXT PRIMARY KEY,
    workflow_id            TEXT NOT NULL REFERENCES workflow(id),
    workspace_id           TEXT NOT NULL REFERENCES workspace(id),
    trigger_event_id       TEXT REFERENCES agent_event(id),
    status                 TEXT NOT NULL,  -- created|running|paused
                                           -- |waiting_human|waiting_effect
                                           -- |completed|failed|cancelled
    current_node_ids       TEXT,           -- JSON array of workflow_node.id
    summary                TEXT,
    started_at             TEXT NOT NULL,
    finished_at            TEXT
);

CREATE TABLE workflow_step_run (
    id                  TEXT PRIMARY KEY,
    workflow_run_id     TEXT NOT NULL REFERENCES workflow_run(id),
    node_id             TEXT NOT NULL REFERENCES workflow_node(id),
    status              TEXT NOT NULL,    -- pending|running|succeeded|failed|skipped
    attempt_number      INTEGER NOT NULL DEFAULT 1,
    effect_id           TEXT REFERENCES effect(id),
    approval_request_id TEXT REFERENCES approval_request(id),
    started_at          TEXT,
    finished_at         TEXT,
    output_ref          TEXT,
    output_hash         TEXT,
    error               TEXT
);

CREATE TABLE trigger (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    workflow_id     TEXT NOT NULL REFERENCES workflow(id),
    kind            TEXT NOT NULL,       -- 'cron'|'event'|'webhook'|'manual'
    config_ref      TEXT NOT NULL,
    enabled         INTEGER NOT NULL,
    created_at      TEXT NOT NULL
);

CREATE TABLE agent_task (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    workflow_run_id     TEXT REFERENCES workflow_run(id),
    assigned_to_actor_id TEXT REFERENCES actor(id),
    title               TEXT NOT NULL,
    description_ref     TEXT,
    status              TEXT NOT NULL,    -- open|in_progress|blocked|done|cancelled
    priority            INTEGER NOT NULL DEFAULT 0,
    due_at              TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

-- ----------------------------------------------------------------------------
-- 11. Artifacts
-- ----------------------------------------------------------------------------

CREATE TABLE artifact (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    kind            TEXT NOT NULL,     -- 'screenshot'|'file'|'tool_output'
                                       -- |'transcript'|'report'|'model_output'
                                       -- |'audio'|'video'|...
    uri             TEXT NOT NULL,     -- e.g. 'file:///...', 's3://...'
    content_hash    TEXT NOT NULL,
    bytes           INTEGER,
    sensitivity     TEXT NOT NULL,
    created_by_actor_id TEXT NOT NULL REFERENCES actor(id),
    created_at      TEXT NOT NULL,
    deleted_at      TEXT
);

-- ----------------------------------------------------------------------------
-- 12. Replay
-- ----------------------------------------------------------------------------

CREATE TABLE replay_checkpoint (
    id                          TEXT PRIMARY KEY,
    workspace_id                 TEXT NOT NULL REFERENCES workspace(id),
    event_id                     TEXT NOT NULL REFERENCES agent_event(id),
    session_id                   TEXT REFERENCES session(id),
    workflow_run_id              TEXT REFERENCES workflow_run(id),
    context_build_id             TEXT REFERENCES context_build(id),
    state_snapshot_ref           TEXT NOT NULL,   -- artifact ref
    model_route_snapshot_ref     TEXT NOT NULL,
    permission_snapshot_ref      TEXT NOT NULL,
    memory_snapshot_ref          TEXT NOT NULL,
    created_at                   TEXT NOT NULL
);

CREATE TABLE replay_run (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    checkpoint_id       TEXT NOT NULL REFERENCES replay_checkpoint(id),
    mode                TEXT NOT NULL,   -- 'recorded'|'experimental'|'policy'
                                         -- |'model'|'memory'|'tool'|'local_only'
    overrides_ref       TEXT,            -- artifact ref to override doc
    requested_by_actor_id TEXT NOT NULL REFERENCES actor(id),
    status              TEXT NOT NULL,   -- 'pending'|'running'|'completed'|'failed'
    started_at          TEXT NOT NULL,
    finished_at         TEXT,
    summary_ref         TEXT,
    error               TEXT
);

CREATE TABLE replay_diff (
    id                  TEXT PRIMARY KEY,
    replay_run_id       TEXT NOT NULL REFERENCES replay_run(id),
    original_event_id   TEXT REFERENCES agent_event(id),
    replay_event_id     TEXT,            -- synthetic event in replay scope
    kind                TEXT NOT NULL,   -- 'identical'|'changed'|'missing'|'extra'
    diff_summary        TEXT,
    detail_ref          TEXT
);

-- ----------------------------------------------------------------------------
-- 13. Audit and observability
-- ----------------------------------------------------------------------------

-- audit_event is a denormalized projection over agent_event for fast querying
-- by auditors and dashboards. The agent_event ledger remains the source of
-- truth; audit_event is rebuildable.
CREATE TABLE audit_event (
    id                  TEXT PRIMARY KEY,
    agent_event_id      TEXT NOT NULL REFERENCES agent_event(id),
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    actor_id            TEXT NOT NULL REFERENCES actor(id),
    event_type          TEXT NOT NULL,
    sensitivity         TEXT NOT NULL,
    decision            TEXT,            -- 'allow'|'allow_with_approval'|'deny'
    decision_reason     TEXT,
    created_at          TEXT NOT NULL
);

CREATE INDEX idx_audit_ws_time ON audit_event(workspace_id, created_at);

-- ----------------------------------------------------------------------------
-- 14. Companion store reference rows
-- ----------------------------------------------------------------------------

CREATE TABLE embedding_ref (
    id                TEXT PRIMARY KEY,
    workspace_id      TEXT NOT NULL REFERENCES workspace(id),
    object_type       TEXT NOT NULL,    -- 'memory'|'message'|'artifact'|...
    object_id         TEXT NOT NULL,
    embedding_model   TEXT NOT NULL,
    vector_store      TEXT NOT NULL,    -- 'qdrant'|'lance'|'chroma'|...
    vector_id         TEXT NOT NULL,    -- id within the vector store
    sensitivity       TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    expires_at        TEXT,
    deleted_at        TEXT
);

CREATE INDEX idx_embedding_object ON embedding_ref(object_type, object_id);

-- secret_ref. The vault holds the material; we hold only a pointer.
CREATE TABLE secret_ref (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    provider        TEXT NOT NULL,     -- 'keychain'|'vault'|'kms'|'1password'
    handle          TEXT NOT NULL,     -- provider-specific identifier
    scope           TEXT NOT NULL,
    description     TEXT,
    created_at      TEXT NOT NULL,
    last_used_at    TEXT,
    revoked_at      TEXT
);

-- ----------------------------------------------------------------------------
-- Verification (mirrors the checklist at the end of 01-architecture.md)
-- ----------------------------------------------------------------------------
--   * Every subsystem in 01 has at least one table here.
--   * Every command in 03 writes only tables in this file.
--   * Every effect_type in 04 is matched by a worker_capability row.
--   * replay_checkpoint columns suffice to reconstruct context for every
--     replay mode in 07.
--   * No table stores raw secret material; secrets are referenced via
--     secret_ref only.
