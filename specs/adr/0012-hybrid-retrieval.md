# ADR-0012: Hybrid retrieval (dense + sparse) is the default

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

Dense vectors capture semantic similarity; sparse vectors (BM25, SPLADE-v3, BM42) capture exact-match signals — file names, function names, identifiers, error strings, ticket IDs. Agents query both kinds of content constantly:

- "Why is `kanban_unblock` failing in SwooshBoard?" — dense alone misses the identifiers.
- "What does our latest decision about caching say?" — sparse alone misses the semantic intent.

Mainstream RAG stacks built on dense-only retrieval consistently regress on code, identifiers, logs, and named entities. Modern vector backends (Qdrant, Pinecone, Weaviate) all expose hybrid retrieval as a first-class operation.

## Decision

ActantIndex ships **hybrid retrieval as the default mode** in `actant new` projects beyond the `minimal` template. Every project scaffolded with `--mode default` enables:

- a dense embedder (FastEmbed local default; ADR-0014)
- a sparse encoder (BM25 + SPLADE-v3)
- a reranker (ADR-0013)

The retrieval planner fuses candidate sets, deduplicates, applies capsule/sensitivity policy, reranks, packs into context.

`mode=basic` (dense-only) remains an explicit opt-in for projects that don't want sparse overhead.

## Consequences

### Positive

- Code search, log search, exact-name queries work out of the box.
- Default behavior aligns with how 2026 vector backends are designed.
- The hybrid pipeline is the substrate for the `entity` and `graph` modes; default hybrid is on the path to richer retrieval, not a side-trip.

### Negative

- Slightly larger indexing time (one extra sparse encode per chunk).
- Sparse encoder choice (BM25 vs SPLADE) is opinionated; we pick SPLADE-v3 default with a flag to drop to BM25 for lower-resource environments.

### Neutral / open

- Multivector / late-interaction retrieval (ColBERT-class) ships in Phase 3 as an opt-in `deep` mode, not the default.

## Alternatives considered

- **Dense-only default.** Rejected — proven to fail on the agent-code-search use case.
- **Sparse-only.** Rejected — misses paraphrase queries.
- **Make the developer choose.** Rejected — the default should be the right answer.

## References

- `/specs/15-actant-index.md` §11 (retrieval modes).
- `/agents/actant-index.md`, `/agents/actant-embedders.md`.
- Prior art: Qdrant hybrid, Pinecone hybrid.
