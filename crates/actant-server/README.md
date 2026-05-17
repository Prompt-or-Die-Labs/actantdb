# actant-server

The HTTP + WebSocket front door for ActantDB.

Owns:

- `POST /v1/command` and `POST /v1/command/batch` (deferred to Phase 2).
- WebSocket `GET /v1/subscribe`.
- Worker API: `POST /v1/effects/claim`, `/heartbeat`, `/start`, `/observe`.
- Artifact upload/download endpoints.
- Replay API: `POST /v1/replay`, `GET /v1/replay/{id}`, `POST /v1/checkpoints`.
- Health, version, metadata endpoints.
- Bearer + mTLS auth (Phase 1 ships bearer only; mTLS in Phase 6).
- Per-actor + per-workspace rate limiting.

Binary: `actantdb-server` (`src/bin/server.rs`).

See `agents/actant-server.md` for the work package.
