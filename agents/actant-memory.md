# Work package: `actant-memory`

## Context

`actant-memory` owns the memory lifecycle, the candidate/approval flow, and the provenance traversal helpers. It also owns the retrieval index module because vectors are indexes over governed memory, not an independent runtime surface. Phase 1 implements the candidate → approve/reject path that the alpha demo exercises. Restrict / expire / revoke / delete and embedding integration arrive in Phase 3.

## Specs to read first

- `/specs/06-context-and-memory.md` §5 (lifecycle), §6 (categories), §7 (provenance).
- `/specs/02-data-model.sql` — `memory_candidate`, `memory`, `memory_use`.
- `/specs/03-command-spec.md` — `propose_memory`, `approve_memory`, `reject_memory`, `record_memory_use`.

## Scope (Phase 1)

### Public API surface

```rust
pub struct MemoryService { /* Storage handle */ }

impl MemoryService {
    pub async fn propose(&self, tx: &mut Transaction<'_>, candidate: NewMemoryCandidate)
        -> Result<MemoryCandidateId, MemoryError>;

    pub async fn approve(&self, tx: &mut Transaction<'_>, candidate_id: &MemoryCandidateId,
                         scope: MemoryScope, expires_at: Option<OffsetDateTime>)
        -> Result<MemoryId, MemoryError>;

    pub async fn reject(&self, tx: &mut Transaction<'_>, candidate_id: &MemoryCandidateId,
                        reason: String) -> Result<(), MemoryError>;

    pub async fn record_use(&self, tx: &mut Transaction<'_>, use_: NewMemoryUse)
        -> Result<(), MemoryError>;

    pub async fn provenance(&self, memory_id: &MemoryId) -> Result<Provenance, MemoryError>;
}

pub enum MemoryScope { Global, Session(SessionId), Project(String) }
pub struct Provenance { pub memory: Memory, pub candidate: MemoryCandidate, pub events: Vec<AgentEvent> }
```

### Internal modules

```
crates/actant-memory/src/
├── lib.rs
├── service.rs
├── candidate.rs               // auto-approve threshold + sensitivity gate
├── provenance.rs              // memory → candidate → events traversal
└── error.rs
```

### Tests

- Approve gate: `sensitivity in {high, secret, regulated}` always goes to `pending_review` regardless of confidence.
- Approve gate: `sensitivity in {public, low, medium}` + `confidence >= threshold` auto-approves.
- Approve cascade: approving a candidate creates exactly one `memory` row with matching `source_candidate_id`.
- Reject path: rejected candidates do not produce a memory.
- Provenance: traversal returns events in `source_event_ids` order.

## Acceptance criteria

- [ ] `cargo build -p actant-memory` zero warnings.
- [ ] `cargo test -p actant-memory` passes.
- [ ] `cargo clippy -p actant-memory -- -D warnings` passes.
- [ ] Provenance traversal lists every event in `memory.source_event_ids`.
- [ ] An approved memory has non-null `source_candidate_id` and `source_event_ids` matching the candidate.

## Do NOT

- Do NOT compute embeddings here in Phase 1. That ships in Phase 3.
- Do NOT delete or revoke. Phase 3.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
