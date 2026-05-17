# Work package: `actant-kernel`

## Context

`actant-kernel` is the hot-path coordinator. It is the only crate that runs in the synchronous command path. It composes `actant-command`, `actant-policy`, `actant-storage`, `actant-effects`, and `actant-subscribe` under a discipline (ADR-0018):

> The synchronous path validates authority, commits intent, updates hot projections, and enqueues effects. Nothing else.

Everything expensive — embeddings, model calls, reranking, workflow advancement, compliance generation, OTel export — runs as **lane consumers** of the chronicle. Lanes are documented in `/planning/lane-catalog.md`.

## Specs to read first

- `/specs/19-performance-architecture.md` — full file.
- `/specs/adr/0018-hot-kernel-async-lanes.md`, `0019-progressive-enrichment.md`, `0020-deployment-modes.md`.
- `/planning/lane-catalog.md`.
- `/planning/performance-budgets.md`.

## Scope

### Public API

```rust
pub struct Kernel {
    storage:    Arc<actant_storage::Storage>,
    policy:     Arc<actant_policy::Guard>,
    effects:    Arc<actant_effects::EffectQueue>,
    subscribe:  Arc<actant_subscribe::ChangeFeed>,
    dispatch:   Arc<DispatchTable>,
    hot:        Arc<HotState>,
}

pub struct DispatchTable {
    /// command_type → (validator fn, policy requirement, transaction fn, emitted events, projection updates)
    entries: HashMap<&'static str, DispatchEntry>,
}

pub struct HotState {
    /// in-memory L0 cache: capability tokens, rate-limit/budget counters,
    /// pending approvals, workflow state, worker heartbeats, hot memory shortlists
}

impl Kernel {
    pub async fn dispatch(&self, raw: RawCommand, authn: AuthenticatedActor)
        -> Result<DispatchResult, KernelError>;

    pub async fn refresh_capability_token(&self, actor: &ActorId) -> Result<CapabilityToken, KernelError>;
    pub async fn invalidate_caches(&self, event: &EmittedEvent);
}
```

### Hot-path lifecycle (exactly the steps in `/specs/19` §4)

```
1. authenticate actor                 (token lookup; L0)
2. validate command schema            (compiled; in-process)
3. compiled policy check              (decision DAG; L0)
4. lightweight budget/rate check      (in-memory counters; L0)
5. append event                       (L1 WAL via actant-storage)
6. update hot projection              (L0 + L2 row via actant-storage::Transaction)
7. enqueue effect (if needed)         (L0 + L2 effect_queue_entry)
8. notify subscribers                 (in-process changefeed broadcast)
```

### Internal modules

```
crates/actant-kernel/src/
├── lib.rs
├── kernel.rs
├── dispatch.rs                  (DispatchTable + compiled command lookups)
├── tokens.rs                    (CapabilityToken compile + refresh)
├── hot_state.rs                 (L0 cache; sync.Mutex/parking_lot, no async locks inside the hot path)
├── admission.rs                 (refuses expensive work on the hot path; redirects to lanes)
├── budget_counter.rs            (in-memory token bucket; periodic snapshot)
├── policy_dag.rs                (compiled decision graph; array-indexed lookup)
├── invalidate.rs                (changefeed subscribers that flush hot caches)
└── error.rs
```

### Tests

- Dispatch table coverage: every alpha command has an entry.
- Capability-token round-trip: a session-start compile produces a token; an `authority_scope` revoke invalidates it.
- Compiled policy DAG: lookup is O(1) by indexed arrays; no string match on the hot path (architectural test using `grep`).
- Hot projection consistency: a hot read after a commit reflects the committed row; lane work does not affect it.
- Admission control: a command that would trigger graph expansion in its sync path is rejected by the kernel.
- Bench harness: every operation in `/specs/19` §2 meets p50/p99 on a developer laptop (M-series Mac or 4-core Linux x86).

## Acceptance criteria

- [ ] `cargo build -p actant-kernel` zero warnings.
- [ ] `cargo test -p actant-kernel` passes.
- [ ] `cargo clippy -p actant-kernel -- -D warnings` passes.
- [ ] `bench/` harness runs and meets the latency budgets in `/specs/19-performance-architecture.md` §2.
- [ ] Architectural grep test: no HTTP calls, no process spawn, no model SDK calls, no vector-store queries inside any `Transaction<'_>` block across the workspace.
- [ ] All six invariants in `/specs/19` §16 are enforced by named tests.

## Do NOT

- Do NOT add a lane to the hot path. Lanes are consumers of the changefeed; they never block a command.
- Do NOT do heap allocations in the policy DAG lookup. Preallocate; reuse.
- Do NOT use string lookups on the hot path. Intern + index.
- Do NOT call out to any other process (model worker, embedding service) from inside the kernel.
- Do NOT use `unsafe` (except for documented `repr(C)` interop with pre-allocated arenas, with safety comments).

## Hand-off

`just ci` + `cargo run -p bench -- --suite hot-path` shows green budgets.
