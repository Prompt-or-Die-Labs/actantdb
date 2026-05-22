# Work package: `actant-embedders`

## Context

Provider registry for embedders, sparse encoders, and rerankers. Used by `actant-memory::index` and `actant-memory`. Ships with FastEmbed (local default), MLX (Apple), and adapters for OpenAI, Voyage, Cohere, Jina, Mixedbread, Nomic, custom OpenAI-compatible. Sparse: BM25 + SPLADE-v3. Rerankers: local BGE-reranker-v2 + cloud (Cohere/Voyage/Jina) + a policy-aware reranker that emits reasons.

## Specs to read first

- `/specs/15-actant-index.md` §10 (registry); §9 (embedding spaces).
- `/specs/adr/0014-local-first-embedders.md`.
- `/specs/adr/0012-hybrid-retrieval.md`, `/specs/adr/0013-rerank-default.md`.

## Scope

```rust
#[async_trait] pub trait Embedder: Send + Sync { /* ... see ActantIndex spec §10 ... */ }
#[async_trait] pub trait Reranker: Send + Sync { /* ... */ }
#[async_trait] pub trait SparseEncoder: Send + Sync { /* ... */ }

pub struct Registry { /* configured providers + capabilities */ }
impl Registry {
    pub fn embedder(&self, name: &str) -> Option<Arc<dyn Embedder>>;
    pub fn reranker(&self, name: &str) -> Option<Arc<dyn Reranker>>;
    pub fn sparse(&self, name: &str) -> Option<Arc<dyn SparseEncoder>>;
}
```

### Provider feature flags

```
default = ["fastembed", "openai-compat"]
fastembed | mlx | openai | voyage | cohere | jina | mixedbread | nomic
sparse-bm25 | sparse-splade | sparse-bm42
rerank-local | rerank-cohere | rerank-voyage | rerank-jina | rerank-policy
```

### Internal modules

```
crates/actant-embedders/src/
├── lib.rs
├── traits.rs
├── registry.rs
├── space.rs                     (embedding_space + compat checks)
├── providers/                   (one file per provider; feature-gated)
└── error.rs
```

### Tests

- Each provider's adapter returns the documented dimension.
- Cross-space query rejected unless `embedding_space` declares compatibility.
- Reranker `reason` field is always populated.

## Acceptance criteria

- [ ] Build/test/clippy green with `--features fastembed,sparse-splade,rerank-local`.
- [ ] `Registry::embedder("fastembed:bge-small-en-v1.5")` returns an embedder that produces 384-dim vectors.
- [ ] An OpenAI adapter does not call out during test (mocked transport).

## Do NOT

- Do NOT depend on any embedder by default. FastEmbed is feature-gated default, not unconditional.
- Do NOT cache vectors here; that's `actant-embed`.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
