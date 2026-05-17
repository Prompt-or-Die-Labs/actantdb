# migrations/

Schema migrations for ActantDB.

## Runner contract

- Files are named `NNNN_short_name.sql` where `NNNN` is a zero-padded sequence number.
- Migrations are applied in **lexicographic** order (which equals numeric order under zero-padding).
- A migration is applied exactly once per database. The runner keeps a `_schema_migrations` table (name and applied_at).
- Migrations are **forward-only** in Phase 1. There is no automatic `down`. To revert, write a new `NNNN_revert_*.sql` migration that reverses the change.
- Each migration is wrapped in a single transaction. A failure rolls back.
- Migrations MUST be idempotent at the table level (`CREATE TABLE IF NOT EXISTS` is not used — instead, a failed migration must leave no half-state; the runner aborts on first error).

## Adding a new migration

1. Update `/specs/02-data-model.sql` first. That file is the source of truth.
2. Create a new migration file in this directory with the next sequence number.
3. The migration MUST contain only the **delta** from the prior schema, not the full schema.
4. Add the migration to the test fixtures in `crates/actant-storage/tests/`.
5. Run `cargo test -p actant-storage --test migrations` to verify the chain still applies cleanly.

## Conventions

- All IDs are `TEXT` (ULID/UUIDv7 strings).
- All timestamps are `TEXT` in RFC3339 UTC.
- All booleans are `INTEGER 0/1`.
- Foreign keys reference by name. `PRAGMA foreign_keys=ON` is enabled at connection time by the storage layer.

## Phase boundaries

| File                    | Phase | Notes                                                |
| ----------------------- | ----- | ---------------------------------------------------- |
| `0001_initial.sql`      | 1     | Mirrors `/specs/02-data-model.sql` for SQLite alpha. |
| `0002_*` ... `0099_*`   | 1–2   | Schema fixes discovered during alpha implementation. |
| `0100_postgres_prep.sql`| 6     | Reserve for Postgres backend introduction.           |
