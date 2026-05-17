# Work package: `actant-cache`

## Context

Sensitivity-aware caches: semantic, model-response, retrieval, reranker, tool-result, web-extraction, artifact-summary, context-build, policy-decision. Caches honor capsule policy; secret-class is never cached, high-sensitivity is local-only, cross-user reuse is forbidden.

## Specs to read first

- `/specs/18-reliability-primitives.md` §9 (Defaults), §10 (integration).
- `/specs/05-security-model.md` §3, §4 (sensitivity + visibility).
- `/specs/06-context-and-memory.md` §3 (firewall — cache invalidation must respect this).

## Scope

```rust
pub struct CacheService { storage: Arc<actant_storage::Storage> }

pub enum CacheType { Embedding, Retrieval, Reranker, ModelResponse, ToolResult, WebExtraction, ArtifactSummary, ContextBuild, PolicyDecision }

pub struct CacheKey { pub cache_type: CacheType, pub key_hash: String }

impl CacheService {
    pub async fn get(&self, key: &CacheKey, actor: &ActorId, sensitivity_ceiling: Sensitivity) -> Result<Option<String>, CacheError>;
    pub async fn put(&self, tx: &mut Transaction<'_>, key: CacheKey, value_ref: String, sensitivity: Sensitivity, policy_id: &str, expires_at: Option<OffsetDateTime>) -> Result<(), CacheError>;
    pub async fn invalidate_for_memory(&self, memory_id: &MemoryId) -> Result<u32, CacheError>;
    pub async fn invalidate_for_policy(&self, policy_id: &str) -> Result<u32, CacheError>;
}
```

### Internal modules

```
crates/actant-cache/src/
├── lib.rs
├── service.rs
├── key.rs                       (canonical hashing per cache_type)
├── policy.rs                    (sensitivity + visibility rules)
├── invalidate.rs                (cascade on memory/permission change)
└── error.rs
```

### Tests

- A `secret`-sensitivity put is rejected.
- A `high`-sensitivity put is cached but never returned to a cloud route.
- Cross-actor read isolation: actor A cannot fetch actor B's cached value even with the same key.
- `invalidate_for_memory` removes cache entries derived from that memory.

## Acceptance criteria

- [ ] Build/test/clippy green.
- [ ] Property test: 1000 random `(actor, key, sensitivity)` puts followed by random gets never leak across actors.
- [ ] Cache hits emit a `cache.hit` metric (consumed by `actant-trace`).

## Do NOT

- Do NOT cache anything labeled `secret` or `regulated`.
- Do NOT serve a cached value to a model route whose ceiling is lower than the cached value's sensitivity.

## Hand-off

`just ci`.
