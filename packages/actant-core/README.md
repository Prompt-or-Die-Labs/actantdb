# @actantdb/core

Embedded backend state for ActantDB's TypeScript adapters: local ledger,
approval queue, replay checkpoints, context manifests, and capture helpers.
Used by the framework adapters and Studio.

## What lives here

- `EventKind` + `ActantEvent` types — the union of events captured during an agent run.
- `Ledger` — append-only local store under `~/.actantdb/<project>/events.sqlite`. It uses `node:sqlite` on Node and `bun:sqlite` on Bun.
- Approval records and capture helpers used by `@actantdb/mastra`, `@actantdb/langgraph`, `@actantdb/elizaos`, and the other adapters.
- `CheckpointRef` — the small record that lets replay rerun from a chosen event.
- Hashing + redaction helpers shared across wrappers.

## What doesn't live here

- Framework-specific interception (that lives in `@actantdb/mastra`, `@actantdb/convex`, and sibling adapters).
- UI (that lives in `actant-studio`).
- Rust server storage, multi-tenancy, auth, sync, and OTel export.

## Status

Pre-1.0. This package is the embedded Node/Bun backend path. It is not an
agent runtime and does not replace your framework; it records the backend state
your framework produces.

See [`/PIVOT.md`](../../PIVOT.md), [`/CHANGELOG.md`](../../CHANGELOG.md), and [`/docs/RUNTIME_GUIDANCE.md`](../../docs/RUNTIME_GUIDANCE.md).
