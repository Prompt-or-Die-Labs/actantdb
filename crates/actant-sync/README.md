# actant-sync

Selective-sync engine for local-first multi-device and team replication. Phase 6.

Owns:

- A `SyncDestination` trait (push/pull/peer).
- Per-row, per-capsule sync-policy resolution: `local_only`, `metadata_only`, `team_sync`, `cloud_sync`, `hash_only`, `encrypted_sync`, `never_sync`.
- Replication of Chronicle slices (append-only — Chronicle is conflict-free by construction).
- Projection-row sync (last-write-wins per ADR-0011; mid-Phase 6 picks the rule).
- Conflict telemetry: emit `sync_conflict_detected` when projection writes diverge so operators can review.

Does **not** own: transport (uses the same HTTP API as clients), authentication (uses standard bearer/mTLS), encryption of artifacts (capsule + secret_ref handle that).

See `agents/actant-sync.md`.
