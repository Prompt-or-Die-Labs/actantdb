# Work package: `actant-subscribe`

## Context

`actant-subscribe` is the live-row subscription engine. Every committed command emits an `EmittedEvent` set; subscribers filtered against those events (or against derived projection changes) receive incremental updates. Phase 1 ships the in-process changefeed and the tables required by the alpha demo.

## Specs to read first

- `/specs/08-api-spec.md` §5 — subscription semantics.
- `/specs/01-architecture.md` §"Subscription Engine".
- `/specs/05-security-model.md` §10 — visibility filtering at subscribe time.

## Scope (Phase 1)

### Public API surface

```rust
pub struct ChangeFeed { /* broadcaster handle */ }
pub struct Subscriber { /* user-facing async stream of SubscriptionEvent */ }

#[derive(Debug, Clone)]
pub enum SubscriptionEvent<R> {
    Snapshot { rows: Vec<R>, version: u64 },
    Upsert   { row: R, version: u64 },
    Delete   { row_id: String, version: u64 },
    Lag      { lost_versions: u64 },
    SnapshotComplete,
}

pub struct SubscribeRequest {
    pub table: SubscriptionTable,
    pub filter: Filter,
    pub actor_id: ActorId,
    pub workspace_id: WorkspaceId,
    pub buffer_size: usize,
}

pub enum SubscriptionTable {
    Session, Message, AgentEvent, ModelCall, ToolCall, Effect,
    ApprovalRequest, MemoryCandidate, Memory, MemoryUse, ContextBuild,
    ContextItem, WorkflowRun, WorkflowStepRun, AgentTask, AuthorityScope,
    Worker, WorkerHeartbeat, ReplayRun, ReplayDiff,
}

pub struct Filter { /* flat equality and `in` against indexed columns */ }

impl ChangeFeed {
    pub fn subscribe(&self, req: SubscribeRequest)
        -> Result<impl Stream<Item = SubscriptionEvent<serde_json::Value>>, SubscribeError>;

    // Called by the command engine on each successful commit.
    pub fn notify_commit(&self, ws: &WorkspaceId, events: &[EmittedEvent]);
}
```

### Internal modules

```
crates/actant-subscribe/src/
├── lib.rs
├── changefeed.rs           // broadcast hub
├── subscriber.rs           // per-subscription buffer, lag handling
├── filter.rs               // Filter language eval
├── snapshot.rs             // initial-snapshot query per table
├── authority.rs            // visibility filtering (delegate to actant-policy)
└── error.rs
```

### Tests

- Snapshot-then-incremental: a subscriber opened during a sequence of writes sees the snapshot first, then incremental rows in commit order.
- Backpressure: a buffer-full subscriber receives a `Lag` event and a re-snapshot.
- Authority filtering: a subscriber that cannot read a workspace cannot subscribe; if a subscribed actor's authority is later revoked, the stream emits a `Lag` and closes.
- Concurrent commits: every committed event reaches every matching subscriber exactly once until cancellation.

## Acceptance criteria

- [ ] `cargo build -p actant-subscribe` zero warnings.
- [ ] `cargo test -p actant-subscribe` passes.
- [ ] `cargo clippy -p actant-subscribe -- -D warnings` passes.
- [ ] For each Phase 1 subscription table (those used by the alpha demo), the snapshot+incremental round-trip passes a property test (1000 random commits, 10 concurrent subscribers).

## Do NOT

- Do NOT call out over the network. This crate is in-process; `actant-server` adapts it to WebSocket.
- Do NOT skip authority filtering. Subscribers must not see rows they cannot read.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
