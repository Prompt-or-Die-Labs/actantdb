# Work package: `actant-lock`

## Context

Lease-bounded resource locks. Prevents two agents editing the same file, sending duplicate emails, or producing conflicting memory writes.

## Specs to read first

- `/specs/18-reliability-primitives.md` §6.

## Scope

```rust
pub struct LockService { storage: Arc<actant_storage::Storage> }

pub struct LockRequest<'a> { pub workspace_id: &'a WorkspaceId, pub resource_key: &'a str, pub owner: &'a ActorId, pub ttl_seconds: u32 }
pub struct LockHandle { pub id: LockId, pub expires_at: OffsetDateTime }

impl LockService {
    pub async fn acquire(&self, tx: &mut Transaction<'_>, req: LockRequest<'_>) -> Result<LockHandle, LockError>;
    pub async fn extend(&self, tx: &mut Transaction<'_>, lock_id: &LockId, ttl_seconds: u32) -> Result<LockHandle, LockError>;
    pub async fn release(&self, tx: &mut Transaction<'_>, lock_id: &LockId, owner: &ActorId) -> Result<(), LockError>;
    pub async fn force_release(&self, tx: &mut Transaction<'_>, lock_id: &LockId, reason: &str) -> Result<(), LockError>;
}
```

### Key conventions

```
lock:file:<canonical_path>
lock:ticket:<id>
lock:memory:<id>
lock:workflow:<name>
lock:actor:<id>
```

### Internal modules

```
crates/actant-lock/src/
├── lib.rs
├── service.rs
├── reaper.rs                   (expires stale locks)
└── error.rs
```

### Tests

- Acquire is atomic under concurrent callers.
- Expired lock can be re-acquired by a different actor; original owner sees `LockError::Lost` on release.
- `force_release` is audited.

## Acceptance criteria

- [ ] Build/test/clippy green.
- [ ] Property: 100 concurrent acquires on the same `resource_key` produce exactly one winner.

## Do NOT

- Do NOT make locks reentrant by default.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
