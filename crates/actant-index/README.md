# actant-index

Retrieval subsystem (ActantIndex). Owns the retrieval planner, hybrid search engine (dense + sparse + entity/graph), chunker, reranker dispatch, context packer (with budget + MMR + diversity), retrieval traces, and retrieval evals. Depends on `actant-embed` for vector storage and `actant-embedders` for model providers. Specs: `/specs/15-actant-index.md`.

See `agents/actant-index.md`.
