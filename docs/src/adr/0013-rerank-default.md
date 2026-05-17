# ADR-0013: Reranking is part of the default stack

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

Embedding retrieval produces *candidates*. Without a reranker, the top-k chosen for a model prompt is noisy — relevance is approximated by cosine similarity, which is correlated with but not equal to actual usefulness. Modern stacks pair retrieval with a cross-encoder reranker that scores each candidate against the query directly.

Cohere, Voyage, Jina, and FastEmbed all expose rerank models for this purpose. Local cross-encoder rerankers (BGE-reranker-v2) run on CPU in milliseconds.

## Decision

ActantIndex ships **reranking enabled by default** in `actant new` projects beyond `minimal`. The pipeline:

```
retrieve top 100 (hybrid) → rerank top 100 → select top 8–20 for context
```

The default reranker is local (FastEmbed `bge-reranker-v2`). Cloud rerankers (Cohere, Voyage, Jina) are available via the `actant-embedders` registry.

A reranker MUST emit a per-candidate **reason** alongside its score. The Retrieval Inspector in Studio shows the reason; auditors can see *why* a candidate made the cut.

## Consequences

### Positive

- Context quality improves materially on real queries.
- Reasons are part of the audit trail; relevance is no longer a black box.
- Policy-aware rerankers (a special class) can refuse candidates that conflict with capsule policy with a reason, instead of letting the firewall drop them silently.

### Negative

- Adds latency (typical: ~50ms for 100 candidates with a local cross-encoder).
- Adds an extra model dependency. Mitigated by bundling FastEmbed in the default install.

### Neutral / open

- Whether to run reranker on every retrieval or only when the candidate count exceeds a threshold is a per-route policy. Phase 1 default: always rerank.

## Alternatives considered

- **No reranking.** Rejected — proven to leave significant quality on the table.
- **Reranker only on opt-in.** Rejected — defaults shape the developer's mental model; the default must be the right answer.

## References

- `/specs/15-actant-index.md` §13 (context packer), §10 (reranker registry).
- `/agents/actant-embedders.md`.
- Cohere Rerank docs, FastEmbed reranking, Voyage rerank.
