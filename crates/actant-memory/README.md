# actant-memory

The Memory Engine — candidate/approved memory lifecycle, provenance, use tracking.

Owns:

- Memory state-machine helpers covering the lifecycle from `specs/06-context-and-memory.md` §5.
- Auto-approve threshold logic per workspace policy.
- Provenance traversal helpers (`memory → candidate → events`).
- `MemoryUse::record(memory_id, context_build_id, ...)`.
- Restrict/expire/revoke/delete semantics with embedding-ref cascade.

Phase 1: candidate/approve/reject/use. Phase 3 adds restrict/expire/revoke/delete and embedding integration.

See `agents/actant-memory.md` for the work package.
