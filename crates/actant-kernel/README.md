# actant-kernel

The hot-path coordinator. Composes `actant-command`, `actant-policy`, `actant-storage`, `actant-effects`, and `actant-subscribe` under a discipline:

> The synchronous path validates authority, commits intent, updates hot projections, and enqueues effects. Nothing else.

Owns:

- **Dispatch table** — compiled command-name → (validator, policy requirement, transaction function, emitted events, projection updates) lookups. No dynamic plugin resolution on the hot path.
- **Capability tokens** — at session/workflow start, compile an actor's authority into a compact token; commands check the token in microseconds.
- **Compiled policy decision DAG** — policy bundles compile into a graph evaluated against (effect, actor, resource, sensitivity, workflow mode) without runtime string evaluation.
- **Hot projection tier** — an in-memory L0 cache above the SQLite/Postgres projections, holding active sessions, pending approvals, leases, budgets, worker health, hot memory shortlists. WAL-backed; reconstructable from event log.
- **Hot rate-limit + budget counters** — in-memory token-bucket state per `(scope, key)`, with periodic durable snapshots.
- **Admission control** — refuses to add expensive work (graph expansion, deep retrieval, compliance trace) to the hot path; routes it to async lanes.

Does **not** own: model calls, embeddings, reranking, workflows, evals, observability export, compliance generation, graph extraction. Those run in async lanes.

See `agents/actant-kernel.md`, `specs/19-performance-architecture.md`, `specs/adr/0018-hot-kernel-async-lanes.md`.
