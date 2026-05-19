# @actantdb/core

Shared types + local event ledger + capture protocol. Used by every wrapper (`@actantdb/mastra`, future `@actantdb/convex`, etc.) and by `actant-studio`.

## What lives here

- `EventKind` + `ActantEvent` types — the union of events captured during an agent run.
- `Ledger` — append-only local store (SQLite via `better-sqlite3`) under `~/.actant/<project>/events.sqlite`. Subscribe-capable; Studio reads from it.
- `PolicyVerdict` types — the contract `@actantdb/mastra` (and others) hand to Guard logic.
- `CheckpointRef` — the small record that lets replay rerun from a chosen event.
- Hashing + redaction helpers shared across wrappers.

## What doesn't live here

- Framework-specific interception (that lives in `@actantdb/mastra`, `@actantdb/convex`, …).
- UI (that lives in `actant-studio`).
- Remote backend, multi-tenancy, OTel export — out of scope for v0.1.

## Status

Pre-alpha. The Phase 1 milestone is the smallest core that makes `@actantdb/mastra` work end-to-end. Anything beyond that waits.

See [`/PIVOT.md`](../../PIVOT.md), [`/CHANGELOG.md`](../../CHANGELOG.md), [`/CHANGELOG.md`](../../CHANGELOG.md).
