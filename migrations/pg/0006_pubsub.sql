-- ============================================================================
-- 0006_pubsub.sql — Postgres flavor of the named-topic pub/sub broker.
--
-- See migrations/0006_pubsub.sql for the design notes. PG variant uses
-- TIMESTAMPTZ for created_at and JSONB for payload.
-- ============================================================================

CREATE TABLE pubsub_message (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    topic           TEXT NOT NULL,
    payload         JSONB NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_pubsub_ws_topic_id ON pubsub_message(workspace_id, topic, id);
CREATE INDEX idx_pubsub_ws_id       ON pubsub_message(workspace_id, id);
