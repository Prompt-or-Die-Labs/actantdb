# Work package: `actant-replay`

## Context

`actant-replay` is the Replay Engine. Phase 1 ships only **checkpoint creation** — the full replay loops for all seven modes arrive in Phase 5. Implementing checkpoints early is required because Phase 1's command engine creates checkpoints at decision points.

## Specs to read first

- `/specs/07-workflows-and-replay.md` §7 (checkpoint contents) and §8 (running a replay).
- `/specs/02-data-model.sql` — `replay_checkpoint`, `replay_run`, `replay_diff`.
- `/specs/05-security-model.md` §2 invariant 9.

## Scope (Phase 1)

### Public API surface

```rust
pub struct CheckpointWriter { storage: Arc<actant_storage::Storage>, artifacts: ArtifactClient }

impl CheckpointWriter {
    pub fn new(storage: Arc<actant_storage::Storage>, artifacts: ArtifactClient) -> Self;

    pub async fn create(&self, tx: &mut Transaction<'_>, req: CheckpointRequest)
        -> Result<ReplayCheckpointId, ReplayError>;
}

pub struct CheckpointRequest {
    pub event_id: EventId,
    pub session_id: Option<SessionId>,
    pub workflow_run_id: Option<WorkflowRunId>,
    pub context_build_id: Option<ContextBuildId>,
}
```

### Snapshot contents (each is an artifact URI)

- **state_snapshot**: serialized projection state up to `event_id` for the scoped subset (session, workflow_run, or full workspace).
- **model_route_snapshot**: rows of `model_route` + `model_provider` at the time.
- **permission_snapshot**: rows of `authority_scope` + `policy` at the time.
- **memory_snapshot**: rows of `memory` (text, embedding_ref, sensitivity, visibility, scope) at the time.

Each snapshot is a `application/x-ndjson` artifact with a stable column order.

### Internal modules

```
crates/actant-replay/src/
├── lib.rs
├── checkpoint.rs
├── snapshots/
│   ├── mod.rs
│   ├── state.rs
│   ├── model_route.rs
│   ├── permission.rs
│   └── memory.rs
└── error.rs
```

### Tests

- Checkpoint creation produces non-null artifact URIs for all four snapshots.
- A round-trip: produce snapshots, write to artifact store, read back, parse — yields the same row set.
- A checkpoint missing any snapshot (forced via test hook) refuses to commit (precondition_failed).

## Acceptance criteria

- [ ] `cargo build -p actant-replay` zero warnings.
- [ ] `cargo test -p actant-replay` passes.
- [ ] `cargo clippy -p actant-replay -- -D warnings` passes.
- [ ] A checkpoint created at any event_id permits a Phase 5 replay to start (forward-compat test: the checkpoint row's columns and artifacts match the Phase 5 reader contract documented in `/specs/07-workflows-and-replay.md` §7).

## Do NOT

- Do NOT implement the replay loops in Phase 1. That is Phase 5. Phase 1 only writes checkpoints.
- Do NOT compress or encrypt snapshots in Phase 1. Phase 6 considers crypto erasure.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
