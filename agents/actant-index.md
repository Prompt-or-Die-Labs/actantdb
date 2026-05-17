# Work package: `actant-index`

## Context

ActantIndex is the retrieval subsystem. Owns the retrieval planner, hybrid search (dense + sparse + graph), chunker, reranker dispatch, context packer integration, retrieval traces, and retrieval evals. Sits beside `actant-memory` and `actant-context`; consumes `actant-embed` (vector store) and `actant-embedders` (model providers).

## Specs to read first

- `/specs/15-actant-index.md` — full file.
- `/specs/adr/0012-hybrid-retrieval.md`, `/specs/adr/0013-rerank-default.md`.
- `/specs/06-context-and-memory.md` §2 (the context-build pipeline this feeds).

## Scope

### Public API

```rust
pub struct ActantIndex { /* storage + embedders + embed_store */ }

pub struct IndexRequest<'a> {
    pub workspace_id: &'a WorkspaceId,
    pub object_type: &'a str,
    pub object_id: &'a str,
    pub text: &'a str,
    pub sensitivity: Sensitivity,
    pub capsule_id: Option<String>,
    pub chunker: ChunkerKind,
}

pub struct RetrievalRequest<'a> {
    pub workspace_id: &'a WorkspaceId,
    pub query: &'a str,
    pub mode: RetrievalMode,         // Basic | Hybrid | Graph | Deep
    pub policy_id: &'a str,
    pub actor_id: &'a ActorId,
    pub session_id: Option<&'a SessionId>,
    pub model_route: Option<&'a ModelRouteId>,
    pub k: usize,
}

pub struct RetrievalResult { pub trace_id: RetrievalTraceId, pub items: Vec<RetrievedItem> }

impl ActantIndex {
    pub async fn index(&self, req: IndexRequest<'_>) -> Result<Vec<EmbeddingRefId>, IndexError>;
    pub async fn retrieve(&self, req: RetrievalRequest<'_>) -> Result<RetrievalResult, IndexError>;
    pub async fn reindex(&self, scope: ReindexScope, new_model: &str) -> Result<ReindexReport, IndexError>;
}
```

### Internal modules

```
crates/actant-index/src/
├── lib.rs
├── chunk/                       (fixed_token, semantic, ast_code, markdown, pdf_layout, ...)
├── plan.rs                      (retrieval planner; mode dispatch)
├── hybrid.rs                    (dense + sparse fusion)
├── graph.rs                     (entity + relation expansion)
├── rerank.rs                    (calls actant-embedders::Reranker)
├── pack.rs                      (token-budgeted context packer w/ MMR)
├── version.rs                   (embedding-space + version checks)
├── reindex.rs                   (migration runner)
├── trace.rs                     (retrieval_trace + retrieval_candidate writers)
├── eval.rs                      (eval harness; recall@k, MRR, NDCG)
└── error.rs
```

### Tests

- Hybrid retrieval against a fixture corpus produces deterministic top-k for both dense-only and hybrid modes.
- Capsule policy: a `local_only` chunk never enters a cloud-route retrieval result.
- Reranker reason: every included item has a non-empty `reason_selected`.
- Reindex: re-embedding under a new model is atomic per-chunk and updates `embedding_ref.model_version`.
- Trace integrity: a 100-candidate retrieval produces exactly 1 `retrieval_trace` + 100 `retrieval_candidate` rows.

## Acceptance criteria

- [ ] `cargo build -p actant-index` zero warnings.
- [ ] `cargo test -p actant-index` passes.
- [ ] `cargo clippy -p actant-index -- -D warnings` passes.
- [ ] All five invariants in `/specs/15-actant-index.md` §18 have a corresponding test.
- [ ] Default hybrid mode passes a recall@5 ≥ 0.8 against the bundled benchmark fixture.

## Do NOT

- Do NOT cache retrieval results outside `actant-cache`.
- Do NOT bypass the capsule policy filter; sensitivity travels through retrieval.
- Do NOT cross embedding spaces silently.
- Do NOT use `unsafe`.

## Hand-off

`just ci`. Then run `actant index search "<query>"` against a bundled fixture corpus.
