-- ============================================================================
-- 0001_initial.sql — Postgres flavor of the canonical schema.
--
-- Subset matching what actant-command + actant-effects + actant-server use.
-- Full parity with /specs/02-data-model.sql will land in 6.5; this file
-- is enough to boot the alpha-demo flow against Postgres.
--
-- Differences from the SQLite variant:
--   * TIMESTAMPTZ instead of TEXT for created_at fields.
--   * JSONB instead of TEXT for *_inline columns.
--   * BIGINT for counts.
--   * Integer-typed BOOLEAN where the SQLite schema uses INTEGER 0/1.
-- ============================================================================

CREATE TABLE workspace (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL,
    archived_at     TIMESTAMPTZ
);

CREATE TABLE actor (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    kind            TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL,
    disabled_at     TIMESTAMPTZ
);

CREATE TABLE session (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    title           TEXT,
    initiator_actor_id TEXT NOT NULL REFERENCES actor(id),
    agent_actor_id  TEXT REFERENCES actor(id),
    status          TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL,
    closed_at       TIMESTAMPTZ
);

CREATE TABLE message (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL REFERENCES session(id),
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    author_actor_id TEXT NOT NULL REFERENCES actor(id),
    role            TEXT NOT NULL,
    body_ref        TEXT,
    body_text       TEXT,
    body_hash       TEXT NOT NULL,
    parent_message_id TEXT REFERENCES message(id),
    created_at      TIMESTAMPTZ NOT NULL
);

CREATE TABLE agent_event (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    actor_id            TEXT NOT NULL REFERENCES actor(id),
    session_id          TEXT REFERENCES session(id),
    parent_event_id     TEXT REFERENCES agent_event(id),
    event_type          TEXT NOT NULL,
    causality_kind      TEXT NOT NULL,
    sensitivity         TEXT NOT NULL,
    authority_scope_id  TEXT,
    payload_ref         TEXT,
    payload_inline      JSONB,
    payload_hash        TEXT NOT NULL,
    model_call_id       TEXT,
    tool_call_id        TEXT,
    workflow_run_id     TEXT,
    memory_id           TEXT,
    artifact_id         TEXT,
    command_id          TEXT,
    effect_id           TEXT,
    event_hash          TEXT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_event_workspace_time ON agent_event(workspace_id, created_at);
CREATE INDEX idx_event_session_time   ON agent_event(session_id, created_at);

CREATE TABLE command_record (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    actor_id        TEXT NOT NULL REFERENCES actor(id),
    session_id      TEXT REFERENCES session(id),
    command_type    TEXT NOT NULL,
    input_ref       TEXT,
    input_inline    JSONB,
    input_hash      TEXT NOT NULL,
    policy_id       TEXT,
    status          TEXT NOT NULL,
    error           TEXT,
    created_at      TIMESTAMPTZ NOT NULL,
    committed_at    TIMESTAMPTZ
);

CREATE TABLE idempotency_record (
    workspace_id        TEXT NOT NULL,
    idempotency_key     TEXT NOT NULL,
    actor_id            TEXT NOT NULL REFERENCES actor(id),
    command_type        TEXT NOT NULL,
    input_hash          TEXT NOT NULL,
    result_ref          TEXT,
    created_at          TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (workspace_id, idempotency_key)
);
