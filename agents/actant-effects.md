# Work package: `actant-effects`

## Context

`actant-effects` owns the effect queue, the worker claim protocol, retries, and dead-lettering. The protocol is exhaustively specified — implement it strictly to that spec.

## Specs to read first

- `/specs/04-effect-protocol.md` — full file.
- `/specs/02-data-model.sql` — `effect`, `effect_result`, `effect_claim`, `worker`, `worker_capability`, `worker_heartbeat`.
- `/specs/01-architecture.md` §"Effect Engine".

## Scope (Phase 1)

### Public API surface

```rust
pub struct EffectQueue { storage: Arc<actant_storage::Storage> }

pub struct ClaimRequest<'a> {
    pub worker_id: &'a WorkerId,
    pub effect_types: &'a [EffectType],
    pub lease_seconds: u32,
    pub max_count: u32,
}

pub struct Lease {
    pub effect_id: EffectId,
    pub effect_type: EffectType,
    pub input_ref: Option<String>,
    pub input_hash: String,
    pub workspace_id: WorkspaceId,
    pub expires_at: OffsetDateTime,
    pub idempotency_key: Option<String>,
    pub attempt_number: u32,
}

impl EffectQueue {
    pub fn new(storage: Arc<actant_storage::Storage>) -> Self;

    pub async fn enqueue(&self, tx: &mut Transaction<'_>, new_effect: NewEffect)
        -> Result<EffectId, EffectError>;

    pub async fn claim(&self, req: ClaimRequest<'_>) -> Result<Vec<Lease>, EffectError>;
    pub async fn heartbeat(&self, effect_id: &EffectId, worker_id: &WorkerId,
                           extend_seconds: u32) -> Result<(), EffectError>;
    pub async fn start(&self, effect_id: &EffectId, worker_id: &WorkerId)
        -> Result<(), EffectError>;
    pub async fn complete(&self, effect_id: &EffectId, worker_id: &WorkerId,
                          result: EffectCompletion) -> Result<(), EffectError>;
    pub async fn observe(&self, effect_id: &EffectId, worker_id: &WorkerId,
                         observation_ref: &str) -> Result<EventId, EffectError>;

    // Maintenance — called by a background task.
    pub async fn reap_expired_leases(&self) -> Result<u32, EffectError>;
}
```

### Internal modules

```
crates/actant-effects/src/
├── lib.rs
├── queue.rs
├── claim.rs                  // atomic SELECT-then-UPDATE under SQLite
├── lease.rs                  // reaper task
├── backoff.rs                // 2s → 8s → 30s → 2m → 10m + ±25% jitter
└── error.rs
```

### Tests

- Idempotency: enqueue twice with same `(workspace_id, idempotency_key)` returns the same `effect_id`.
- Claim atomicity: 50 concurrent claimers, 1 effect → exactly one winner.
- Lease loss: a missed heartbeat returns the effect to `pending` with `attempt_count += 1`.
- Retry backoff schedule matches the spec table (with jitter window).
- Dead-letter: exhausting `max_attempts` transitions to `dead_letter` and emits the event.
- Approval gate: an effect in `awaiting_approval` is NOT returned by `claim`.

## Acceptance criteria

- [ ] `cargo build -p actant-effects` zero warnings.
- [ ] `cargo test -p actant-effects` passes (including concurrency tests).
- [ ] `cargo clippy -p actant-effects -- -D warnings` passes.
- [ ] Property test: 200 concurrent claim attempts produce at most one winner per effect row.
- [ ] All 10 invariants in `/specs/04-effect-protocol.md` §8 have a corresponding test.

## Do NOT

- Do NOT execute side effects. This crate is the QUEUE; workers (separate binaries) perform side effects.
- Do NOT bypass the approval gate. An effect's status flow is the spec; do not invent shortcuts.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
