# Work package: `actant-embed`

## Context

Vector-store adapter library. Phase 3 ships LanceDB; the trait surface keeps Qdrant / Chroma / FAISS / pgvector / SQLite-vec swappable.

## Specs to read first

- `/specs/06-context-and-memory.md` §8 (embedding policy).
- `/specs/02-data-model.sql` — `embedding_ref`.

## Scope

```rust
#[async_trait]
pub trait EmbeddingStore: Send + Sync {
    async fn upsert(&self, ref_: &EmbeddingRef, vector: &[f32]) -> Result<(), EmbedError>;
    async fn delete(&self, ref_id: &str) -> Result<(), EmbedError>;
    async fn query(&self, q: EmbeddingQuery) -> Result<Vec<EmbeddingHit>, EmbedError>;
    async fn count(&self, scope: &EmbeddingScope) -> Result<u64, EmbedError>;
}

pub struct EmbeddingQuery {
    pub vector: Vec<f32>,
    pub k: usize,
    pub scope: EmbeddingScope,        // workspace + optional object_type filter
    pub min_score: Option<f32>,
}

pub struct EmbeddingHit { pub embedding_ref_id: String, pub object_type: String, pub object_id: String, pub score: f32, pub sensitivity: Sensitivity }
```

Phase 3 implementation: `lance::LanceStore`. Feature flags: `qdrant`, `chroma`, `faiss`, `pgvector`, `sqlite-vec`.

### Internal modules

```
crates/actant-embed/src/
├── lib.rs
├── trait_.rs
├── ref_.rs                 // EmbeddingRef + serde
├── lance/                  // default
└── (others behind feature flags, file stubs OK)
```

### Tests

- Round-trip: upsert / query / delete.
- Sensitivity filter: a query whose route ceiling is `medium` does not return `high` or `secret` results.
- Model swap: re-embedding all memories under a new model is atomic per-memory (no half-state).

## Acceptance criteria

- [ ] `cargo build -p actant-embed` zero warnings.
- [ ] `cargo test -p actant-embed --no-default-features --features lance` passes.
- [ ] Sensitivity-filter property test passes.

## Do NOT

- Do NOT bundle multiple stores in the default feature set.
- Do NOT cache vectors outside the store (the store is authoritative).
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
