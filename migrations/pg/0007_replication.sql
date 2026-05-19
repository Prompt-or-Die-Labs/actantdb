-- ============================================================================
-- pg/0007_replication.sql -- replication-friendly event semantics (Postgres).
--
-- Mirrors migrations/0007_replication.sql so the migrations-parity CI gate
-- (GAPS.md row #22) stays green. See docs/IOS_EMBEDDING.md §4 +
-- docs/SYNC_DESIGN.md for the design.
-- ============================================================================

ALTER TABLE agent_event ADD COLUMN device_id        TEXT   NOT NULL DEFAULT '_legacy_';
ALTER TABLE agent_event ADD COLUMN hlc_physical_ms  BIGINT NOT NULL DEFAULT 0;
ALTER TABLE agent_event ADD COLUMN hlc_logical      BIGINT NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_agent_event_hlc    ON agent_event(hlc_physical_ms, hlc_logical);
CREATE INDEX IF NOT EXISTS idx_agent_event_device ON agent_event(device_id);
