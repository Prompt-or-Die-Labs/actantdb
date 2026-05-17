# ADR-0014: Local-first embedders by default

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

Embeddings are a hot path: every indexed chunk, every retrieval query, every memory candidate produces an embedding. Cloud embedding APIs (OpenAI, Voyage, Cohere) work well but introduce:

- per-call cost,
- network latency,
- data residency concerns,
- vendor lock-in,
- failure modes during outages.

For personal-agent products (Swoosh on Mac), private memories should never leave the device — which makes cloud embedders a non-starter for the default path.

FastEmbed runs ONNX-based dense/sparse/multivector embedding models locally on CPU with millisecond latency for the small/base class. MLX runs efficient ML on Apple silicon and exposes MLX Swift bindings for Mac-first apps.

## Decision

ActantIndex defaults to **local embedders**:

- `fastembed:bge-small-en-v1.5` (dense) — Phase 1 default.
- `fastembed:splade-v3-distilbert` (sparse) — Phase 1 default.
- `fastembed:bge-reranker-v2-base` (rerank) — Phase 1 default.
- `mlx:gte-small-mlx` on Apple silicon — Phase 2 if Swoosh ships; Phase 3 otherwise.

Cloud embedders (OpenAI, Voyage, Cohere, Jina, Mixedbread, Nomic, custom OpenAI-compatible) ship in the same registry and are explicit opt-ins via `actant.yaml`:

```yaml
index:
  default_embedder:
    provider: voyage
    model: voyage-4-large
```

Capsule policy can override per-content:

- `local_only` capsules cannot use cloud embedders.
- `cloud_model_allowed=false` content cannot enter a cloud embedding API even if it's allowed in cloud model context.

## Consequences

### Positive

- `actant new` → `actant dev` works offline.
- No API key required for the first hour of developer experience.
- Sensitive content stays local by construction.
- Cost predictable.
- Apple-first products (Swoosh) get a first-class path.

### Negative

- Local model quality varies. We standardize on BGE-small for English; multilingual ships separately.
- Disk usage: ~100MB per local model. Acceptable for a developer tool; documented.
- Cloud embedders are still essential for some workloads (long-form, multilingual edge cases, frontier accuracy). The registry pattern keeps them first-class, just not default.

### Neutral / open

- MLX vs llama.cpp on Apple silicon: MLX wins for embedding-specific workloads; we default to MLX once it's available in the project.

## Alternatives considered

- **Cloud default with local fallback.** Rejected — defaults the developer into paying immediately, leaks privacy by default.
- **No bundled model.** Rejected — `actant dev` MUST work offline.
- **Bundle multiple local models.** Rejected — bundle the proven default; let `actant index reembed` swap.

## References

- `/specs/15-actant-index.md` §10 (embedder registry), §17 (replay).
- `/agents/actant-embedders.md`.
- FastEmbed, MLX, MLX Swift.
