# Work package: `actant-sync`

## Context

Selective-sync engine. Phase 6. Replicates Chronicle slices + projection rows between ActantDB nodes (laptop + phone, team + cloud) respecting capsule policy.

## Specs to read first

- `/specs/01-architecture.md` §"Deployment topologies".
- `/specs/14-extended-primitives.md` §3 (capsule sync_policy).
- ADR-0011 (sync conflict resolution; will be authored in Phase 6).

## Scope

```rust
#[async_trait]
pub trait SyncDestination: Send + Sync {
    async fn push(&self, batch: SyncBatch) -> Result<SyncReceipt, SyncError>;
    async fn pull(&self, cursor: SyncCursor) -> Result<SyncBatch, SyncError>;
}

pub struct SyncEngine { storage: Arc<actant_storage::Storage>, policy: Arc<actant_policy::Guard>, dest: Box<dyn SyncDestination> }

impl SyncEngine {
    pub async fn run_loop(self) -> Result<(), SyncError>;       // long-running; reads change-feed
    pub async fn one_shot(&self) -> Result<SyncStats, SyncError>;
}

pub struct SyncBatch { pub events: Vec<AgentEvent>, pub projections: Vec<ProjectionRow> }
```

### Decisions

- Chronicle is append-only → sync is conflict-free for events.
- Projection rows: last-write-wins per ADR-0011 (which will be authored when Phase 6 starts). The sync engine emits `sync_conflict_detected` if two writes arrive with overlapping `version` from different sources.
- Per-row policy resolution: the engine asks `actant-policy::capsule` for the policy. Rows whose policy is `local_only` or `never_sync` are skipped silently (no metadata leak via "we skipped a row").
- `metadata_only` strips payloads (replaces `body_ref` / `input_ref` with the hash only).
- `encrypted_sync` requires a per-destination KMS key (Phase 6 supports cloud-KMS + on-device Keychain key delegation).

### Internal modules

```
crates/actant-sync/src/
├── lib.rs
├── engine.rs
├── batch.rs               // SyncBatch builders
├── destinations/
│   ├── mod.rs
│   ├── http.rs            // HTTP push/pull to a peer ActantDB
│   └── object_store.rs    // S3-compatible for one-way export
├── policy.rs              // capsule policy → field redaction / drop
├── conflict.rs
└── error.rs
```

### Tests

- A pair of in-memory nodes converges on Chronicle after one push round.
- A `local_only` capsule's content does not appear on the destination.
- A `metadata_only` capsule's content is replaced by hash; restoring is impossible from the destination.
- Conflicting projection writes produce one `sync_conflict_detected` per conflict, not silent loss.

## Acceptance criteria

- [ ] Build / test / clippy green.
- [ ] Property test: any sequence of writes on two nodes converges on Chronicle.
- [ ] No private capsule content leaks across the sync boundary in a 10k-row fixture.

## Do NOT

- Do NOT bypass capsule policy. Every row's policy is consulted.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
