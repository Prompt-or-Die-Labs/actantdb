# Work package: `actant-server`

## Context

`actant-server` is the HTTP + WebSocket front door. It composes `actant-storage`, `actant-command`, `actant-policy`, `actant-effects`, `actant-context`, `actant-memory`, `actant-subscribe`, `actant-replay`, and `actant-flow` behind a stable wire API.

## Specs to read first

- `/specs/08-api-spec.md` — complete wire API.
- `/specs/05-security-model.md` §10 — cross-actor isolation, especially the subscription-time authority check.
- `/specs/04-effect-protocol.md` §2 — worker API endpoint shapes.

## Scope (Phase 1)

### Endpoints

- `POST /v1/command` — single command dispatch. Idempotency-Key header honored.
- `WS   /v1/subscribe` — open-subscribe / unsubscribe / per-subscription buffer.
- `POST /v1/effects/claim`, `/v1/effects/{id}/heartbeat`, `/v1/effects/{id}/start`, `/v1/effects/{id}/observe` — worker API.
- `POST /v1/artifacts`, `GET /v1/artifacts/{id}` — artifact upload/download.
- `GET  /v1/health`, `GET /v1/version`, `GET /v1/metadata/commands`, `GET /v1/metadata/tables`.

`POST /v1/replay`, `GET /v1/replay/{id}`, `POST /v1/checkpoints` are wired but `POST /v1/replay` returns `not_implemented` in Phase 1 — replay execution lands in Phase 5. Checkpoint creation is supported.

### Auth

- Bearer token (Phase 1).
- mTLS scaffold (no enforcement; Phase 6 turns it on).
- Authenticated principal → `actor` row lookup → `AuthenticatedActor`.

### Rate limiting

Per-actor and per-workspace token buckets as in `/specs/08-api-spec.md` §10.

### Internal modules

```
crates/actant-server/src/
├── lib.rs
├── app.rs                     // axum::Router construction
├── auth.rs                    // bearer + mTLS scaffold
├── error.rs                   // wire-error mapping; ActantError -> ApiError
├── handlers/
│   ├── mod.rs
│   ├── command.rs
│   ├── subscribe.rs
│   ├── effects.rs
│   ├── artifacts.rs
│   ├── replay.rs
│   └── metadata.rs
├── ratelimit.rs
└── bin/
    └── server.rs              // main()
```

### Tests

- `POST /v1/command` happy path for every alpha command produces the correct response shape per `/specs/08-api-spec.md` §4.
- Rejection paths produce JSON errors with the right `code` and `request_id`.
- WebSocket subscribe → initial snapshot → incremental → unsubscribe flow.
- Worker API: claim → heartbeat → complete round-trip.
- Auth: missing token returns 401; invalid token returns 401.
- Rate limit: exceeded bucket returns 429 with `Retry-After`.

## Acceptance criteria

- [ ] `cargo build -p actant-server` zero warnings.
- [ ] `cargo test -p actant-server` passes.
- [ ] `cargo clippy -p actant-server -- -D warnings` passes.
- [ ] `actantdb-server` boots, listens on the configured port, serves `/v1/health`, and runs the alpha demo from `/specs/10-alpha-demo.md` end-to-end against a localhost client.
- [ ] `GET /v1/metadata/commands` returns exactly the alpha command set with valid JSON Schema for each input.

## Do NOT

- Do NOT introduce a general `POST /v1/query` endpoint. Reads come through subscriptions and metadata.
- Do NOT skip auth on any endpoint except `/v1/health` and `/v1/version`.
- Do NOT use `unsafe`.

## Hand-off

`just ci`. Then run `just serve` and the alpha demo from `/specs/10-alpha-demo.md`.
