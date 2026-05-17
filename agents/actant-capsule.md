# Work package: `actant-capsule`

## Context

Capsule resolution and strictest-wins composition. Sensitivity travels through derivations via this crate.

## Specs to read first

- `/specs/13-actant-contract.md` §8 (sensitivity travels).
- `/specs/14-extended-primitives.md` §3 (capsule + capsule_membership).
- `/specs/adr/0005-data-capsules.md`.

## Scope

```rust
pub struct Capsule { /* mirrors the row */ }
pub struct CapsulePolicy {
    pub sensitivity: Sensitivity,
    pub visibility: VisibilitySet,
    pub sync_policy: SyncPolicy,
    pub retention_policy: RetentionPolicy,
    pub cloud_model_allowed: bool,
    pub memory_allowed: MemoryAllowed,
}

pub struct CapsuleService { storage: Arc<actant_storage::Storage> }

impl CapsuleService {
    pub async fn resolve(&self, object_type: &str, object_id: &str) -> Result<Vec<Capsule>, CapsuleError>;
    pub fn compose_strictest(capsules: &[Capsule]) -> CapsulePolicy;
    pub async fn attach(&self, tx: &mut Transaction<'_>, capsule_id: &str, object_type: &str, object_id: &str) -> Result<(), CapsuleError>;
}
```

Upgrade rules (Phase 3 simple form): a workspace-level rules table holds `{ trigger_pattern, target_sensitivity }`. When a derivation gathers content matching a trigger, the composed policy's sensitivity is bumped accordingly.

### Internal modules

```
crates/actant-capsule/src/
├── lib.rs
├── service.rs
├── compose.rs
├── attach.rs
├── rules.rs               // workspace upgrade rules
└── error.rs
```

### Tests

- Strictest-wins: composing 3 capsules of sensitivity `low, medium, high` yields `high`.
- Visibility intersection: `cloud_model_allowed ∩ local_model_allowed` yields `local_model_allowed` only.
- Attach is idempotent: attaching the same `(capsule, object)` twice is a no-op.
- Resolve traverses all memberships for the object id, ordered by `created_at`.

## Acceptance criteria

- [ ] Build / test / clippy green.
- [ ] Property test: any random vector of capsules composes to the strictest in O(n).
- [ ] Resolve does not panic on a missing object_id; returns empty `Vec`.

## Do NOT

- Do NOT weaken policy on compose. Strictest only.
- Do NOT cache resolutions across writes; capsule membership can change.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
