# 15 — ActantIndex

ActantIndex is the **retrieval subsystem**. Vector embeddings are not memory; they are indexes over governed agent knowledge. ActantIndex makes that distinction structural by combining dense + sparse + entity/graph retrieval, reranking, semantic chunking, context packing, retrieval traces, and retrieval evals under one subsystem governed by ActantDB's policy and memory layers.

The system invariant:

> Every retrieval is permissioned, inspectable, replayable, and bounded by sensitivity policy.

## 1. Design principle

```
Vectors are not memory.
Vectors are indexes over governed agent knowledge.
```

A retrieval result without provenance, sensitivity, and visibility is a leak. A retrieval result without a trace is a black box. ActantIndex prevents both by construction.

## 2. Subsystems

```
ActantIndex
├── Embedder Registry            (actant-embedders)
├── Chunker                      (actant-memory::index::chunk)
├── Sparse Encoder               (actant-embedders sparse)
├── Dense Vector Store           (actant-embed)
├── Sparse Vector Store          (actant-embed sparse)
├── Multivector Store            (actant-embed multivector)
├── Entity Index                 (actant-memory::index::entity)
├── Graph Index                  (actant-memory::index::graph)
├── Memory Index                 (joins actant-memory)
├── Artifact Index               (joins artifact)
├── Retrieval Planner            (actant-memory::index::plan)
├── Hybrid Search Engine         (actant-memory::index::hybrid)
├── Reranker                     (actant-embedders rerank)
├── Context Packer               (actant-memory::index::pack; consumed by actant-context)
├── Policy Filter                (delegates to actant-policy::capsule)
├── Embedding Version Manager    (actant-memory::index::version)
├── Reindexer                    (actant-memory::index::reindex)
├── Retrieval Evaluator          (actant-memory::index::eval + actant-eval)
└── Retrieval Trace Store        (retrieval_trace + retrieval_candidate)
```

## 3. Retrieval pipeline

```
query
→ normalize + intent classify
→ load actor authority
→ apply source policy
→ embed query (provider chosen by route, see §10)
→ run dense search
→ run sparse search
→ run entity / graph expansion
→ merge candidates
→ deduplicate
→ filter by sensitivity + visibility + capsule policy
→ rerank
→ enforce diversity (MMR)
→ pack into token budget
→ emit retrieval_trace
→ hand off to actant-context for the final context manifest
→ model call
```

Each stage emits a span (`/specs/17-observability.md`).

## 4. Indexed object

```sql
indexed_object (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    object_type         TEXT NOT NULL,    -- 'memory'|'message'|'artifact'|'tool_result'
                                          -- |'observation'|'workflow_state'|'event'|'doc'
    object_id           TEXT NOT NULL,
    source_event_ids    TEXT NOT NULL,
    canonical_text_ref  TEXT,
    summary             TEXT,
    sensitivity         TEXT NOT NULL,
    visibility_policy   TEXT NOT NULL,
    sync_policy         TEXT NOT NULL,
    capsule_id          TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT
);
```

Every chunk derived from an indexed object inherits its policy; every embedding derived from a chunk inherits the chunk's. That is sensitivity lineage applied to retrieval.

## 5. Chunking

Modes:

```
fixed_token
semantic
ast_code            (per language: Rust, TS, Python, Swift, Go, ...)
markdown_section
pdf_layout
conversation_turn
tool_result
workflow_step
memory_evidence
multimodal_frame    (video keyframes, audio segments)
```

```sql
index_chunk (
    id              TEXT PRIMARY KEY,
    indexed_object_id TEXT NOT NULL,
    chunk_index     INTEGER NOT NULL,
    chunk_type      TEXT NOT NULL,
    text_ref        TEXT NOT NULL,
    token_count     INTEGER,
    source_hash     TEXT NOT NULL,
    sensitivity    TEXT NOT NULL,
    metadata        TEXT NOT NULL  -- JSON: bbox, line range, speaker, timecode...
);
```

## 6. Embeddings (rich)

The Phase 1 `embedding_ref` is extended; ActantIndex tracks model + provider + dimension + distance metric + space.

```sql
-- Replaces the Phase 1 minimal row. Migration 0003 extends it.
ALTER TABLE embedding_ref ADD COLUMN chunk_id              TEXT;
ALTER TABLE embedding_ref ADD COLUMN provider              TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE embedding_ref ADD COLUMN model                 TEXT;
ALTER TABLE embedding_ref ADD COLUMN model_version         TEXT;
ALTER TABLE embedding_ref ADD COLUMN embedding_space_id    TEXT;
ALTER TABLE embedding_ref ADD COLUMN dimension             INTEGER;
ALTER TABLE embedding_ref ADD COLUMN distance_metric       TEXT;     -- 'cosine'|'dot'|'l2'
ALTER TABLE embedding_ref ADD COLUMN input_type            TEXT;     -- 'query'|'document'|'passage'
ALTER TABLE embedding_ref ADD COLUMN chunker_version       TEXT;
ALTER TABLE embedding_ref ADD COLUMN redaction_version     TEXT;
ALTER TABLE embedding_ref ADD COLUMN source_hash           TEXT;
```

Sparse:

```sql
sparse_ref (
    id              TEXT PRIMARY KEY,
    chunk_id        TEXT NOT NULL,
    encoder         TEXT NOT NULL,    -- 'bm25'|'splade-v3'|'bm42'
    model_version   TEXT,
    sparse_store    TEXT NOT NULL,
    sparse_id       TEXT NOT NULL,
    created_at      TEXT NOT NULL
);
```

Multivector (late-interaction):

```sql
multivector_ref (
    id              TEXT PRIMARY KEY,
    chunk_id        TEXT NOT NULL,
    encoder         TEXT NOT NULL,    -- 'colbert'|'colpali'|'jina-colbert'
    vector_count    INTEGER NOT NULL,
    dimension       INTEGER NOT NULL,
    vector_store    TEXT NOT NULL,
    vector_id       TEXT NOT NULL,
    created_at      TEXT NOT NULL
);
```

## 7. Entity + graph index

Entities (people, projects, repos, companies, files, APIs, errors, tickets) are extracted during indexing and joined into a small graph.

```sql
entity (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL,
    type            TEXT NOT NULL,
    canonical_name  TEXT NOT NULL,
    aliases         TEXT NOT NULL,   -- JSON array
    sensitivity     TEXT NOT NULL,
    source_events   TEXT NOT NULL,
    capsule_id      TEXT,
    created_at      TEXT NOT NULL
);

entity_relation (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL,
    source_entity   TEXT NOT NULL,
    relation_type   TEXT NOT NULL,
    target_entity   TEXT NOT NULL,
    confidence      REAL NOT NULL,
    evidence_events TEXT NOT NULL    -- JSON array
);
```

Retrieval planner uses graph expansion when the query mentions known entities — finds memories / docs / events connected to them and reranks them alongside vector hits.

## 8. Retrieval traces

```sql
retrieval_trace (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    query               TEXT NOT NULL,
    query_actor_id      TEXT NOT NULL,
    session_id          TEXT,
    retrieval_mode      TEXT NOT NULL,   -- 'dense_only'|'hybrid'|'hybrid+graph'|'multivector'
    policy_id           TEXT NOT NULL,
    selected_count      INTEGER NOT NULL,
    blocked_count       INTEGER NOT NULL,
    created_at          TEXT NOT NULL
);

retrieval_candidate (
    id                  TEXT PRIMARY KEY,
    retrieval_trace_id  TEXT NOT NULL,
    source_type         TEXT NOT NULL,
    source_id           TEXT NOT NULL,
    dense_score         REAL,
    sparse_score        REAL,
    graph_score         REAL,
    rerank_score        REAL,
    final_score         REAL,
    included            INTEGER NOT NULL,  -- 0/1
    blocked_reason      TEXT,
    reason_selected     TEXT
);
```

Every retrieval — even auto-generated background ones — produces a trace. The CLI surfaces it: `actant retrieval inspect ret_123`.

## 9. Embedding spaces (asymmetric retrieval)

Models like Voyage 4 share a compatible embedding space across members of their family; query embeddings from `voyage-4-lite` can search documents embedded with `voyage-4-large`. ActantIndex models this explicitly.

```sql
embedding_space (
    id                  TEXT PRIMARY KEY,
    provider            TEXT NOT NULL,
    family              TEXT NOT NULL,
    compatible_models   TEXT NOT NULL,    -- JSON array
    dimension           INTEGER NOT NULL,
    distance_metric     TEXT NOT NULL
);
```

The retrieval planner can choose a fast query embedder while reading against a precise document embedder, if both live in the same space.

## 10. Embedder + reranker registry

Implemented in `actant-embedders`. Two trait shapes:

```rust
#[async_trait]
pub trait Embedder: Send + Sync {
    fn name(&self) -> &str;
    fn dimension(&self) -> usize;
    fn embedding_space(&self) -> Option<&str>;
    async fn embed(&self, batch: Vec<EmbedInput>) -> Result<Vec<Vec<f32>>, EmbedError>;
}

#[async_trait]
pub trait Reranker: Send + Sync {
    fn name(&self) -> &str;
    async fn rerank(&self, query: &str, candidates: Vec<RerankInput>) -> Result<Vec<RerankScore>, RerankError>;
}
```

Default Phase 1 providers:

```
embedders:
  local: fastembed (BGE-small, E5-small, Nomic-small, Jina-small)
  apple: mlx (Phase 2 if Swoosh ships; Phase 3 otherwise)
  cloud: openai, voyage, cohere, jina, mixedbread, nomic, custom-openai-compat

sparse:
  local: bm25 (in-memory or sqlite-fts), splade-v3 (fastembed)
  cloud: bm42 (qdrant)

rerankers:
  local: fastembed cross-encoder (bge-reranker-v2)
  cloud: cohere rerank-v4, voyage rerank, jina rerank
  policy_aware: a local "model-as-judge" reranker that respects capsule policy and emits a reason
```

## 11. Default retrieval modes

```
basic       dense only
hybrid      dense + sparse, rerank
graph       hybrid + entity/graph expansion
deep        hybrid + graph + multivector
```

Project scaffold asks at `actant new`; default for `coding-agent` template is `hybrid`.

## 12. Memory-aware retrieval

Memory retrieval is not generic vector search. Score is:

```
score =
   semantic_similarity
 + recency_weight
 + confidence_weight
 + explicit_user_approval_weight
 + task_scope_match
 + historical_usefulness
 - stale_penalty
 - conflict_penalty
 - sensitivity_penalty
```

Where:

- `confidence_weight` reads `memory.confidence`
- `historical_usefulness` reads `memory_use.outcome` aggregates
- `conflict_penalty` reads `memory_conflict` rows
- `stale_penalty` reads `memory.last_verified_at` decay
- `sensitivity_penalty` is the firewall rule from `/specs/06-context-and-memory.md`

## 13. Context packer

The packer takes ranked candidates + a token budget and produces the prompt fragment:

- MMR diversity (configurable λ).
- Recency bias for time-anchored sources.
- Memory + tool-result priority slots.
- Source balancing (don't let one file dominate).
- Sensitivity filtering (last-mile check before pack).
- Model-specific formatting (XML tags for Claude, JSON for OpenAI structured, plain for local).
- Long-context compression when the candidate set exceeds the model's window — Phase 4 enhancement.

Output is consumed by `actant-context` which emits the final `context_build` + `context_item` rows.

## 14. CLI

```
actant index init
actant index status
actant index sources
actant index add ./docs
actant index add ./src --type code
actant index add-memory mem_123
actant index build
actant index search "<query>" [--hybrid] [--rerank] [--mode basic|hybrid|graph|deep]
actant index inspect idx_123
actant index reembed [--model PROV:MODEL]
actant index migrate --from <old> --to <new>
actant index eval run
actant retrieval inspect ret_123
actant retrieval trace --session sess_123
```

## 15. Project config

```yaml
index:
  enabled: true
  default_embedder:
    provider: local
    model: fastembed:bge-small-en-v1.5
  sparse:
    enabled: true
    encoder: fastembed:splade-v3
  reranker:
    enabled: true
    provider: local
    model: fastembed:bge-reranker-v2
  vector_store:
    provider: local            # lance default; qdrant/pgvector/chroma/faiss in adapters
  policy:
    cloud_context_max_sensitivity: low
    memory_requires_approval: true
    index_high_sensitivity: local_only
  evals:
    enabled: true
```

## 16. Phase staging

| Phase | Index deliverables                                                  |
| ----- | ------------------------------------------------------------------- |
| 1     | local FastEmbed + BM25 sparse + hybrid retrieval + `retrieval_trace` + minimal context packer + `actant index` CLI. |
| 2     | MCP resource indexing, MLX (if Swoosh), entity extraction, retrieval-aware drift signal. |
| 3     | Full graph index, multivector (ColBERT-class), reranker policy reasons, embedding-space migration tooling, retrieval evals. |
| 4     | Workflow-state indexing, retrieval-aware model routing, automatic eval-from-failure generation. |
| 5     | Replay against alternate embedders/rerankers/retrieval modes.       |
| 6     | Adapters: Qdrant, Pinecone, Pgvector, Chroma, Weaviate, Milvus, OpenSearch hybrid. |

## 17. Replay integration

Every model call's `context_build` references its `retrieval_trace_id`. Replay can therefore run:

```
actant replay run rep_123 --embedder local:bge
actant replay run rep_123 --reranker cohere:rerank-v4
actant replay run rep_123 --without-memory mem_9
actant replay run rep_123 --retrieval-mode graph
```

The diff answers: did retrieval cause the failure? Did reranking fix it?

## 18. Invariants

1. **No retrieval without a trace.** Every retrieval emits `retrieval_trace` + N `retrieval_candidate` rows.
2. **No embed without policy.** Indexing an object requires its capsule policy to be readable. A `local_only` capsule never produces a cloud-store embedding.
3. **Sensitivity travels.** A retrieval result whose chunk sensitivity exceeds the model route's ceiling is dropped at pack time with `blocked_reason='sensitivity'`.
4. **Versioned migration.** Re-embedding under a new model is a planned operation, not destructive. `embedding_ref.model_version` records the transition.
5. **No silent fallback across embedding spaces.** Cross-space queries are rejected unless the spaces are recorded as compatible.

## Verification

- [ ] Every table in §4–§8 has a CREATE TABLE in `/migrations/0003_ai_native_and_reliability.sql`.
- [ ] Every retrieval mode in §11 has a code path in `actant-memory::index::plan`.
- [ ] Every provider in §10 has a registry entry in `actant-embedders`.
- [ ] Every CLI command in §14 is implemented in `actant-cli`.
- [ ] Every invariant in §18 is enforced by a structural check + a property test.
