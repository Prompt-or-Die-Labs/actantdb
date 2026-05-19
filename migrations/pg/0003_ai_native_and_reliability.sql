-- ============================================================================
-- 0003_ai_native_and_reliability.sql -- Postgres flavor of the AI-native +
-- reliability primitives delta.
--
-- 1:1 parity with /migrations/0003_ai_native_and_reliability.sql (SQLite).
-- TEXT for timestamps, INTEGER for booleans -- conventions at top of
-- pg/0001_initial.sql.
-- ============================================================================

-- ----------------------------------------------------------------------------
-- ActantIndex
-- ----------------------------------------------------------------------------

CREATE TABLE indexed_object (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    object_type         TEXT NOT NULL,
    object_id           TEXT NOT NULL,
    source_event_ids    TEXT NOT NULL,
    canonical_text_ref  TEXT,
    summary             TEXT,
    sensitivity         TEXT NOT NULL,
    visibility_policy   TEXT NOT NULL,
    sync_policy         TEXT NOT NULL,
    capsule_id          TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT
);

CREATE INDEX idx_indexed_object_ws_type ON indexed_object(workspace_id, object_type);

CREATE TABLE index_chunk (
    id                  TEXT PRIMARY KEY,
    indexed_object_id   TEXT NOT NULL REFERENCES indexed_object(id),
    chunk_index         INTEGER NOT NULL,
    chunk_type          TEXT NOT NULL,
    text_ref            TEXT NOT NULL,
    token_count         INTEGER,
    source_hash         TEXT NOT NULL,
    sensitivity         TEXT NOT NULL,
    metadata            TEXT NOT NULL
);

CREATE INDEX idx_chunk_obj ON index_chunk(indexed_object_id);

CREATE TABLE sparse_ref (
    id              TEXT PRIMARY KEY,
    chunk_id        TEXT NOT NULL REFERENCES index_chunk(id),
    encoder         TEXT NOT NULL,
    model_version   TEXT,
    sparse_store    TEXT NOT NULL,
    sparse_id       TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

CREATE TABLE multivector_ref (
    id              TEXT PRIMARY KEY,
    chunk_id        TEXT NOT NULL REFERENCES index_chunk(id),
    encoder         TEXT NOT NULL,
    vector_count    INTEGER NOT NULL,
    dimension       INTEGER NOT NULL,
    vector_store    TEXT NOT NULL,
    vector_id       TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

CREATE TABLE embedding_space (
    id                  TEXT PRIMARY KEY,
    provider            TEXT NOT NULL,
    family              TEXT NOT NULL,
    compatible_models   TEXT NOT NULL,
    dimension           INTEGER NOT NULL,
    distance_metric     TEXT NOT NULL
);

-- Extend embedding_ref for AI-native richness.
ALTER TABLE embedding_ref ADD COLUMN chunk_id              TEXT;
ALTER TABLE embedding_ref ADD COLUMN provider              TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE embedding_ref ADD COLUMN model                 TEXT;
ALTER TABLE embedding_ref ADD COLUMN model_version         TEXT;
ALTER TABLE embedding_ref ADD COLUMN embedding_space_id    TEXT;
ALTER TABLE embedding_ref ADD COLUMN dimension             INTEGER;
ALTER TABLE embedding_ref ADD COLUMN distance_metric       TEXT;
ALTER TABLE embedding_ref ADD COLUMN input_type            TEXT;
ALTER TABLE embedding_ref ADD COLUMN chunker_version       TEXT;
ALTER TABLE embedding_ref ADD COLUMN redaction_version     TEXT;
ALTER TABLE embedding_ref ADD COLUMN source_hash           TEXT;

CREATE TABLE entity (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    type            TEXT NOT NULL,
    canonical_name  TEXT NOT NULL,
    aliases         TEXT NOT NULL,
    sensitivity     TEXT NOT NULL,
    source_events   TEXT NOT NULL,
    capsule_id      TEXT,
    created_at      TEXT NOT NULL
);

CREATE INDEX idx_entity_ws_type ON entity(workspace_id, type);

CREATE TABLE entity_relation (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    source_entity   TEXT NOT NULL REFERENCES entity(id),
    relation_type   TEXT NOT NULL,
    target_entity   TEXT NOT NULL REFERENCES entity(id),
    confidence      REAL NOT NULL,
    evidence_events TEXT NOT NULL
);

CREATE INDEX idx_entity_rel_src ON entity_relation(source_entity);
CREATE INDEX idx_entity_rel_tgt ON entity_relation(target_entity);

CREATE TABLE retrieval_trace (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    query               TEXT NOT NULL,
    query_actor_id      TEXT NOT NULL REFERENCES actor(id),
    session_id          TEXT REFERENCES session(id),
    retrieval_mode      TEXT NOT NULL,
    policy_id           TEXT NOT NULL,
    selected_count      INTEGER NOT NULL,
    blocked_count       INTEGER NOT NULL,
    created_at          TEXT NOT NULL
);

CREATE INDEX idx_retrieval_trace_ws ON retrieval_trace(workspace_id, created_at);

CREATE TABLE retrieval_candidate (
    id                  TEXT PRIMARY KEY,
    retrieval_trace_id  TEXT NOT NULL REFERENCES retrieval_trace(id),
    source_type         TEXT NOT NULL,
    source_id           TEXT NOT NULL,
    dense_score         REAL,
    sparse_score        REAL,
    graph_score         REAL,
    rerank_score        REAL,
    final_score         REAL,
    included            INTEGER NOT NULL,
    blocked_reason      TEXT,
    reason_selected     TEXT
);

CREATE INDEX idx_rcand_trace ON retrieval_candidate(retrieval_trace_id);

-- ----------------------------------------------------------------------------
-- Prompt + tool-schema registry
-- ----------------------------------------------------------------------------

CREATE TABLE prompt_template (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    name            TEXT NOT NULL,
    description     TEXT,
    created_at      TEXT NOT NULL,
    UNIQUE (workspace_id, name)
);

CREATE TABLE prompt_version (
    id                  TEXT PRIMARY KEY,
    prompt_template_id  TEXT NOT NULL REFERENCES prompt_template(id),
    version             INTEGER NOT NULL,
    body_ref            TEXT NOT NULL,
    schema_ref          TEXT,
    eval_case_id        TEXT REFERENCES eval_case(id),
    created_at          TEXT NOT NULL,
    UNIQUE (prompt_template_id, version)
);

-- ----------------------------------------------------------------------------
-- Model registry
-- ----------------------------------------------------------------------------

CREATE TABLE model_registry (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    provider                    TEXT NOT NULL,
    name                        TEXT NOT NULL,
    context_window              INTEGER,
    tool_support                INTEGER NOT NULL,
    json_reliability            REAL,
    vision_support              INTEGER NOT NULL,
    audio_support               INTEGER NOT NULL,
    embedding_support           INTEGER NOT NULL,
    rerank_support              INTEGER NOT NULL,
    cost_per_input_1k           REAL,
    cost_per_output_1k          REAL,
    latency_p50_ms              INTEGER,
    privacy_class               TEXT NOT NULL,
    locality                    TEXT NOT NULL,
    capabilities                TEXT NOT NULL,
    created_at                  TEXT NOT NULL,
    retired_at                  TEXT,
    UNIQUE (workspace_id, provider, name)
);

-- ----------------------------------------------------------------------------
-- Semantic cache
-- ----------------------------------------------------------------------------

CREATE TABLE cache_entry (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    cache_type      TEXT NOT NULL,
    key_hash        TEXT NOT NULL,
    value_ref       TEXT NOT NULL,
    sensitivity     TEXT NOT NULL,
    policy_id       TEXT NOT NULL,
    expires_at      TEXT,
    created_at      TEXT NOT NULL,
    UNIQUE (workspace_id, cache_type, key_hash)
);

-- ----------------------------------------------------------------------------
-- Protocols: MCP, A2A, AP2
-- ----------------------------------------------------------------------------

CREATE TABLE mcp_server (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    name            TEXT NOT NULL,
    transport       TEXT NOT NULL,
    uri             TEXT NOT NULL,
    auth_ref        TEXT,
    capabilities    TEXT NOT NULL,
    registered_at   TEXT NOT NULL,
    retired_at      TEXT
);

CREATE TABLE mcp_resource (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    server_id       TEXT NOT NULL REFERENCES mcp_server(id),
    uri             TEXT NOT NULL,
    name            TEXT,
    mime_type       TEXT,
    sensitivity     TEXT NOT NULL,
    capsule_id      TEXT,
    last_read_at    TEXT,
    UNIQUE (server_id, uri)
);

CREATE TABLE mcp_prompt (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    server_id       TEXT NOT NULL REFERENCES mcp_server(id),
    name            TEXT NOT NULL,
    schema_ref      TEXT NOT NULL,
    version         INTEGER NOT NULL,
    UNIQUE (server_id, name, version)
);

CREATE TABLE a2a_card (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    peer_name       TEXT NOT NULL,
    endpoint        TEXT NOT NULL,
    capabilities    TEXT NOT NULL,
    auth_ref        TEXT,
    trust_ref       TEXT REFERENCES trust_profile(id),
    discovered_at   TEXT NOT NULL,
    retired_at      TEXT
);

CREATE TABLE a2a_interaction (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    peer_card_id    TEXT NOT NULL REFERENCES a2a_card(id),
    direction       TEXT NOT NULL,
    task_id         TEXT NOT NULL,
    intent_id       TEXT REFERENCES intent(id),
    delegation_id   TEXT REFERENCES delegation(id),
    state           TEXT NOT NULL,
    payload_ref     TEXT,
    signature       TEXT,
    created_at      TEXT NOT NULL,
    finished_at     TEXT
);

CREATE TABLE ap2_mandate (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL REFERENCES workspace(id),
    granted_by_actor_id         TEXT NOT NULL REFERENCES actor(id),
    holder_actor_id             TEXT NOT NULL REFERENCES actor(id),
    purpose                     TEXT NOT NULL,
    spend_limit_usd             REAL NOT NULL,
    spend_used_usd              REAL NOT NULL DEFAULT 0,
    cryptographic_proof_ref     TEXT NOT NULL,
    expires_at                  TEXT NOT NULL,
    revoked_at                  TEXT,
    created_at                  TEXT NOT NULL
);

CREATE TABLE ap2_intent (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    mandate_id      TEXT NOT NULL REFERENCES ap2_mandate(id),
    purpose         TEXT NOT NULL,
    amount_usd      REAL NOT NULL,
    payee           TEXT NOT NULL,
    signed_payload  TEXT NOT NULL,
    status          TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    decided_at      TEXT
);

CREATE TABLE ap2_transaction (
    id              TEXT PRIMARY KEY,
    intent_id       TEXT NOT NULL REFERENCES ap2_intent(id),
    processor       TEXT NOT NULL,
    external_ref    TEXT NOT NULL,
    amount_usd      REAL NOT NULL,
    status          TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

-- ----------------------------------------------------------------------------
-- Observability -- chronicle correlation with OTel
-- ----------------------------------------------------------------------------

ALTER TABLE agent_event ADD COLUMN otel_trace_id TEXT;
ALTER TABLE agent_event ADD COLUMN otel_span_id  TEXT;

-- ----------------------------------------------------------------------------
-- Reliability primitives
-- ----------------------------------------------------------------------------

CREATE TABLE rate_limit_policy (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    name                TEXT NOT NULL,
    scope_type          TEXT NOT NULL,
    scope_pattern       TEXT NOT NULL,
    algorithm           TEXT NOT NULL,
    limit_value         INTEGER NOT NULL,
    refill_rate         REAL,
    window_seconds      INTEGER,
    burst_size          INTEGER,
    priority_weight    REAL,
    fairness_key        TEXT,
    created_at          TEXT NOT NULL
);

CREATE TABLE rate_limit_state (
    id                  TEXT PRIMARY KEY,
    policy_id           TEXT NOT NULL REFERENCES rate_limit_policy(id),
    scope_key           TEXT NOT NULL,
    tokens_available    REAL,
    window_start        TEXT,
    used_count          INTEGER,
    reset_at            TEXT,
    updated_at          TEXT NOT NULL,
    UNIQUE (policy_id, scope_key)
);

CREATE TABLE effect_queue_entry (
    id              TEXT PRIMARY KEY,
    effect_id       TEXT NOT NULL REFERENCES effect(id),
    queue_name      TEXT NOT NULL,
    priority        INTEGER NOT NULL,
    fairness_key    TEXT,
    available_at    TEXT NOT NULL,
    deadline_at     TEXT,
    attempts        INTEGER NOT NULL,
    status          TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

CREATE INDEX idx_eq_queue_status ON effect_queue_entry(queue_name, status, available_at);

CREATE TABLE retry_policy (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    name                TEXT NOT NULL,
    max_attempts        INTEGER NOT NULL,
    backoff_type        TEXT NOT NULL,
    initial_delay_ms    INTEGER NOT NULL,
    max_delay_ms        INTEGER NOT NULL,
    jitter              INTEGER NOT NULL,
    retry_on            TEXT NOT NULL,
    do_not_retry_on     TEXT NOT NULL,
    UNIQUE (workspace_id, name)
);

CREATE TABLE circuit_state (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    dependency_key      TEXT NOT NULL,
    state               TEXT NOT NULL,
    failure_count       INTEGER NOT NULL,
    success_count       INTEGER NOT NULL,
    opened_at           TEXT,
    half_open_at        TEXT,
    reason              TEXT,
    updated_at          TEXT NOT NULL,
    UNIQUE (workspace_id, dependency_key)
);

CREATE TABLE dead_letter_item (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    original_effect_id  TEXT REFERENCES effect(id),
    workflow_run_id     TEXT REFERENCES workflow_run(id),
    failure_type        TEXT NOT NULL,
    failure_summary     TEXT NOT NULL,
    attempts            INTEGER NOT NULL,
    last_error          TEXT,
    created_at          TEXT NOT NULL
);

-- `lock` is a Postgres reserved word in some legacy contexts (and conflicts
-- with the LOCK statement keyword); we keep the table name unquoted for parity
-- with SQLite since Postgres only treats it as reserved in specific SQL-99
-- contexts (LOCK TABLE), not as an identifier. Tests confirm CREATE TABLE lock
-- parses cleanly. If any DDL syntax issue surfaces, add quoting around the
-- identifier here and in the matching repo SQL.
CREATE TABLE lock (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    resource_key        TEXT NOT NULL,
    owner_actor_id      TEXT NOT NULL REFERENCES actor(id),
    lease_id            TEXT,
    expires_at          TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    UNIQUE (workspace_id, resource_key)
);

CREATE TABLE ingress_event (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    source              TEXT NOT NULL,
    event_type          TEXT NOT NULL,
    payload_ref         TEXT NOT NULL,
    signature_valid     INTEGER,
    dedupe_key          TEXT,
    received_at         TEXT NOT NULL,
    UNIQUE (workspace_id, source, dedupe_key)
);

CREATE INDEX idx_ingress_ws_recv ON ingress_event(workspace_id, received_at);

CREATE TABLE idempotency_record (
    workspace_id        TEXT NOT NULL,
    idempotency_key     TEXT NOT NULL,
    actor_id            TEXT NOT NULL REFERENCES actor(id),
    command_type        TEXT NOT NULL,
    input_hash          TEXT NOT NULL,
    result_ref          TEXT,
    created_at          TEXT NOT NULL,
    PRIMARY KEY (workspace_id, idempotency_key)
);
