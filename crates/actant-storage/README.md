# actant-storage

SQLite-backed storage for ActantDB Phase 1.

Owns:

- Connection pool (`sqlx::SqlitePool`) bootstrapped from a config struct.
- Migration runner that applies files under `/migrations/` in order.
- `Transaction` wrapper that encapsulates the command transaction contract from `specs/01-architecture.md` §"Command Engine".
- Typed row mappers for every table in `specs/02-data-model.sql`.
- Insert and read helpers for: `agent_event`, `command_record`, `session`, `message`, `tool_call`, `effect`, `approval_request`, `memory_candidate`, `memory`, and the alpha-set projections.
- Atomic claim helper for `effect` (`UPDATE ... RETURNING` semantics emulated for SQLite).

Does **not** own: command dispatch (that's `actant-command`), policy logic (`actant-policy`), HTTP/WS surface (`actant-server`).

See `agents/actant-storage.md` for the work package.
