-- ============================================================================
-- 0007_replication.sql -- replication-friendly event semantics.
--
-- Adds device_id + HLC (Hybrid Logical Clock) columns to agent_event so the
-- ledger can be merged across devices without coordination. Backstop for the
-- iOS embedded mode + CloudKit sync path. See docs/IOS_EMBEDDING.md §4 +
-- docs/SYNC_DESIGN.md.
--
-- Existing rows get device_id='_legacy_' / hlc_physical_ms=0 / hlc_logical=0.
-- New writes (via ingest_events / the FFI path) populate real values; legacy
-- append_event keeps writing without these columns -- the defaults apply.
--
-- Mirrored in migrations/pg/0007_replication.sql for parity-gate green.
-- ============================================================================

ALTER TABLE agent_event ADD COLUMN device_id        TEXT    NOT NULL DEFAULT '_legacy_';
ALTER TABLE agent_event ADD COLUMN hlc_physical_ms  INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_event ADD COLUMN hlc_logical      INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_agent_event_hlc    ON agent_event(hlc_physical_ms, hlc_logical);
CREATE INDEX IF NOT EXISTS idx_agent_event_device ON agent_event(device_id);
