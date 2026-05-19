-- ============================================================================
-- 0006_pubsub.sql — Generic named-topic pub/sub broker.
--
-- DEVX_GAPS.md row #X93. Adds a persistent broker alongside the in-memory
-- SubscribeHub. Every publish writes a row here; subscribers that connect
-- with a `since` cursor replay rows strictly greater than the cursor before
-- attaching to the live broadcast.
--
-- The broker is intentionally distinct from `agent_event`: pubsub messages
-- are application-layer fan-out, not ledger entries, and they do not chain.
-- Workspace isolation is enforced at write time (the broker API takes a
-- WorkspaceId and stamps it onto the row).
-- ============================================================================

CREATE TABLE pubsub_message (
    id              TEXT PRIMARY KEY,    -- ULID (lexicographically sortable)
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    topic           TEXT NOT NULL,
    payload         TEXT NOT NULL,       -- canonical JSON
    created_at      TEXT NOT NULL        -- RFC3339
);

CREATE INDEX idx_pubsub_ws_topic_id ON pubsub_message(workspace_id, topic, id);
CREATE INDEX idx_pubsub_ws_id       ON pubsub_message(workspace_id, id);
