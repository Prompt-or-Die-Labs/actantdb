# actant-storage

SQLite and Postgres storage for ActantDB's substrate.

Owns:

- SQLite connection pool (`sqlx::SqlitePool`) bootstrapped from a config struct.
- Postgres connection pool (`sqlx::PgPool`) bootstrapped from `PgStorage::open`.
- Migration runners that apply files under `/migrations/` and `/migrations/pg/` in order.
- `Transaction` wrapper that encapsulates the command transaction contract from `specs/01-architecture.md` §"Command Engine".
- Backend-neutral helpers for the command substrate: workspace, actor, session,
  agent_event, command_record, idempotency_record, artifact, and session events.
- Postgres mirrors the command-engine helper path. The HTTP server still has
  SQLite-specific route SQL and refuses `ACTANTDB_DATABASE_URL`.
- Atomic claim helper for `effect` (`UPDATE ... RETURNING` semantics emulated for SQLite).

Does **not** own: command dispatch (that's `actant-command`), policy logic (`actant-policy`), HTTP/WS surface (`actant-server`).

See `agents/actant-storage.md` for the work package.
