# Work package: `actant-storage`

## Context

`actant-storage` is the SQLite-backed storage layer for Phase 1. It owns the connection pool, the migration runner, the `Transaction` wrapper that enforces the command-transaction contract, and typed row mappers for every table in `/specs/02-data-model.sql`.

This crate has no business logic. Policy checks belong in `actant-policy`; command orchestration belongs in `actant-command`. Storage's job is to make those crates' lives easy by providing a safe, typed handle to the database.

## Specs to read first

- `/specs/02-data-model.sql` — every table, every column.
- `/specs/01-architecture.md` §"Command Engine" — transaction boundaries.
- `/specs/04-effect-protocol.md` §2 — the atomic claim semantics required of the `effect` row helper.
- `/specs/05-security-model.md` §2 — invariants the storage layer must structurally support (no raw secrets, etc.).
- `/migrations/README.md` — migration runner contract.

## Scope

### Public API surface

```rust
pub struct StorageConfig {
    pub db_url: String,           // e.g. "sqlite://./actant.dev.sqlite?mode=rwc"
    pub max_connections: u32,
    pub run_migrations_on_start: bool,
}

pub struct Storage { /* wraps sqlx::SqlitePool */ }
impl Storage {
    pub async fn connect(cfg: &StorageConfig) -> Result<Self, StorageError>;
    pub async fn migrate(&self) -> Result<(), StorageError>;
    pub async fn begin(&self) -> Result<Transaction<'_>, StorageError>;
    pub fn pool(&self) -> &sqlx::SqlitePool;
}

// A wrapper around sqlx::Transaction that exposes ONLY the typed helpers below.
// The wrapper enforces that:
//   1. Every command-initiated transaction inserts exactly one command_record.
//   2. Every transaction that emits agent_event rows chains event_hash correctly.
pub struct Transaction<'a> { /* ... */ }

impl<'a> Transaction<'a> {
    // command_record
    pub async fn insert_command_record(&mut self, ...) -> Result<CommandId, StorageError>;
    pub async fn mark_command_committed(&mut self, ...) -> Result<(), StorageError>;
    pub async fn mark_command_rejected(&mut self, ...) -> Result<(), StorageError>;

    // agent_event
    pub async fn append_event(&mut self, event: NewAgentEvent) -> Result<EventId, StorageError>;
    pub async fn latest_event_hash(&self, ws: &WorkspaceId) -> Result<Option<String>, StorageError>;

    // session / message
    pub async fn insert_session(&mut self, ...) -> Result<SessionId, StorageError>;
    pub async fn insert_message(&mut self, ...) -> Result<MessageId, StorageError>;

    // tool_call
    pub async fn insert_tool_call(&mut self, ...) -> Result<ToolCallId, StorageError>;
    pub async fn update_tool_call_status(&mut self, ...) -> Result<(), StorageError>;

    // effect
    pub async fn insert_effect(&mut self, ...) -> Result<EffectId, StorageError>;
    pub async fn claim_pending_effect(&mut self, worker_id: &WorkerId,
                                      effect_types: &[EffectType],
                                      lease_seconds: u32) -> Result<Option<EffectClaim>, StorageError>;

    // approval_request
    pub async fn insert_approval_request(&mut self, ...) -> Result<ApprovalRequestId, StorageError>;
    pub async fn update_approval_request(&mut self, ...) -> Result<(), StorageError>;

    // memory_candidate / memory
    pub async fn insert_memory_candidate(&mut self, ...) -> Result<MemoryCandidateId, StorageError>;
    pub async fn approve_memory_candidate(&mut self, ...) -> Result<MemoryId, StorageError>;
    pub async fn reject_memory_candidate(&mut self, ...) -> Result<(), StorageError>;

    // authority_scope (read in policy checks; write in grant/revoke commands)
    pub async fn list_authority_scopes(&self, ...) -> Result<Vec<AuthorityScope>, StorageError>;
    pub async fn insert_authority_scope(&mut self, ...) -> Result<AuthorityScopeId, StorageError>;
    pub async fn revoke_authority_scope(&mut self, ...) -> Result<(), StorageError>;

    pub async fn commit(self) -> Result<(), StorageError>;
    pub async fn rollback(self) -> Result<(), StorageError>;
}
```

### Internal modules

```
crates/actant-storage/src/
├── lib.rs
├── config.rs
├── pool.rs                  // Storage struct, pool helpers, pragmas (foreign_keys=ON).
├── migrate.rs               // Reads /migrations/, applies in order, records in _schema_migrations.
├── transaction.rs           // Transaction wrapper.
├── error.rs                 // StorageError (mapped to ActantError at the command-engine boundary).
├── models/                  // Row structs (one file per table family).
│   ├── mod.rs
│   ├── workspace.rs
│   ├── actor.rs
│   ├── session.rs
│   ├── message.rs
│   ├── agent_event.rs
│   ├── command_record.rs
│   ├── tool.rs
│   ├── tool_call.rs
│   ├── effect.rs
│   ├── approval_request.rs
│   ├── memory.rs
│   ├── authority_scope.rs
│   ├── artifact.rs
│   ├── worker.rs
│   └── replay.rs
├── queries/                 // Query implementations grouped by table.
└── claim.rs                 // Atomic claim_pending_effect logic.
```

### Tests

- Migration test: apply `0001_initial.sql` to a fresh in-memory DB and verify every table exists with the right columns.
- Transaction test: opening a transaction, inserting a `command_record`, and rolling back leaves no rows.
- Event-chain test: appending two events ties the second's `parent_event_id` and chain hash to the first.
- Claim test: concurrent claim attempts on the same effect row produce exactly one winner; the other returns `None`.
- Idempotency test: `insert_effect` with a duplicate `(workspace_id, idempotency_key)` returns the existing row.

## Acceptance criteria

- [ ] `cargo build -p actant-storage` zero warnings.
- [ ] `cargo test -p actant-storage` passes (including the claim concurrency test).
- [ ] `cargo clippy -p actant-storage -- -D warnings` passes.
- [ ] Migration runner applies `migrations/0001_initial.sql` cleanly to a fresh SQLite database and records the application in `_schema_migrations`.
- [ ] `claim_pending_effect` is atomic under concurrent callers (property test with >=100 iterations).
- [ ] No row mapper exposes `payload_inline` or `body_text` for tables flagged `sensitivity=secret` without an explicit `unsafe_read` helper documented as policy-bypass.

## Do NOT

- Do NOT embed policy checks. The storage layer trusts its caller; policy lives in `actant-policy`.
- Do NOT expose raw `sqlx::Transaction`. Hand back only the typed `Transaction` wrapper.
- Do NOT write to projection tables outside the `Transaction` wrapper.
- Do NOT add a `query_any(sql, params)` escape hatch. Every read/write has a typed method.
- Do NOT use `unsafe`.

## Hand-off

Run `just ci` and ensure green.
