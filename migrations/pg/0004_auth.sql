-- ============================================================================
-- 0004_auth.sql -- Postgres flavor of the UI auth + linking-code flow.
--
-- 1:1 parity with /migrations/0004_auth.sql (SQLite). TEXT timestamps; see
-- UI_AUTH_DESIGN.md for the threat model.
-- ============================================================================

CREATE TABLE workspace_owner (
    workspace_id        TEXT PRIMARY KEY REFERENCES workspace(id),
    owner_actor_id      TEXT NOT NULL REFERENCES actor(id),
    password_hash       TEXT,
    password_set_at     TEXT,
    created_at          TEXT NOT NULL
);

CREATE TABLE link_code (
    code_hash               TEXT PRIMARY KEY,
    workspace_id            TEXT NOT NULL REFERENCES workspace(id),
    expires_at              TEXT NOT NULL,
    claimed_at              TEXT,
    claimed_by_actor_id     TEXT REFERENCES actor(id),
    created_at              TEXT NOT NULL
);

CREATE INDEX idx_link_code_expires ON link_code(expires_at);
CREATE INDEX idx_link_code_workspace ON link_code(workspace_id);

CREATE TABLE session_token (
    token_hash          TEXT PRIMARY KEY,
    owner_actor_id      TEXT NOT NULL REFERENCES actor(id),
    workspace_id        TEXT NOT NULL REFERENCES workspace(id),
    csrf_secret         TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    expires_at          TEXT NOT NULL,
    revoked_at          TEXT
);

CREATE INDEX idx_session_token_workspace ON session_token(workspace_id);
CREATE INDEX idx_session_token_expires ON session_token(expires_at);
