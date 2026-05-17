# actant-embed

Vector-store adapter library for ActantDB. Phase 3.

Owns:

- An `EmbeddingStore` trait covering `upsert`, `delete`, `query`, `count`.
- Adapter implementations behind feature flags: `lance` (default, Phase 3 ships), `qdrant`, `chroma`, `faiss`, `pgvector`, `sqlite-vec`.
- Embedding-model registry — names + dimension + cost — so callers can swap models with `actant-embed`'s help.
- Re-embed jobs that respect `embedding_ref.embedding_model`: atomically migrate per-memory to a new model.

Does **not** own: when to embed (that's `actant-memory`), what to embed against (that's `actant-context`).

See `agents/actant-embed.md`.
