# Where are we? — repository state

The earlier "freeze everything but two features" framing (2026-05-17, premortem-driven) was a hypothetical. It was lifted the same day. The substrate was unfrozen and built out. This file is the current state, not a plan.

## Where things actually are

ActantDB shipped two things in parallel:

1. **The v0.1 baseline** — `@actantdb/mastra` + Studio + replay — TypeScript-first, npm-installable, works on Mastra, LangGraph, and hand-rolled agents. Three runnable demos under `/examples/test-cleanup*` with recorded SQLite event ledgers.
2. **The substrate underneath** — the full multi-phase plan from `/specs/`: command engine, effect queue, governed memory, context firewall, workflows, replay engine, ActantIndex, MCP/A2A/AP2 protocols, observability, reliability primitives, hot kernel, six deployment modes. **All present in real Rust code** across ~49 crates.

The packages are what a developer installs. The substrate is what they get when they outgrow the basic install — same API, more behind it.

## What ships today

### npm packages (`/packages`, pnpm workspace)

| Package              | What it does                                                                |
| -------------------- | --------------------------------------------------------------------------- |
| `@actantdb/mastra`   | `withActant(agent, opts)` duck-typed wrapper; captures every tool call, runs Guard, supports approval flows, exposes the timeline to Studio. Works on Mastra, LangGraph, and hand-rolled agents. |
| `@actantdb/core`     | Ledger backed by `node:sqlite`, hash-chained events, monotonic ULIDs, idempotency. |
| `@actantdb/policy`   | Verdict builders + the alpha-demo policy with the `rm -rf …/dist` constrain hint. |
| `@actantdb/replay`   | Checkpoint / run / diff with memory + policy overrides.                     |
| `@actantdb/studio`   | Local HTTP server + vanilla-JS UI with timeline, detail panel, replay modal, side-by-side diff. CLI: `studio | approve | deny | replay create|run|diff | approvals`. |
| `@actantdb/types`    | Generated from `crates/actant-contracts` via the `codegen-ts` subcommand. Hand-edits forbidden. |
| `@actantdb/sdk`      | HTTP + WS client for the Rust server.                                       |
| `@actantdb/convex`   | Adapter for Convex's `handler(ctx, args)` tool shape.                       |

### Rust crates (`/crates`, Cargo workspace, Rust 1.88)

49 crates, ~15k lines, real implementations. Highlights:

- **Phase 1 core**: `actant-storage` (SQLite + Postgres backends), `actant-command` (alpha command set), `actant-policy`, `actant-server` (Axum HTTP + WebSocket), `actant-cli` (the `actantdb` binary), `actant-subscribe`.
- **Phase 2 effects**: `actant-effects`, `actant-worker-protocol`, `actant-worker-{shell,file,model,mcp,browser,email,slack,manager}`.
- **Phase 3 context + memory**: `actant-context` (manifest pipeline), `actant-memory` (lifecycle + conflict detection), `actant-embed` + `actant-embedders` (Hash + plug-in vector path), `actant-index` (dense cosine retrieval).
- **Phase 4 workflows**: `actant-flow` (runner state machine with approval-gate pause/resume), `actant-trigger` (Scheduler + cron).
- **Phase 5 replay**: `actant-replay` (recorded / model / policy / memory modes; experimental/tool/local_only return named-error deferrals).
- **Phase 6 cloud + team**: `actant-auth` (HS256 + OIDC), `actant-tenant`, `actant-audit-export` (purge + retention), `actant-sync`.
- **Reliability**: `actant-throttle`, `actant-circuit`, `actant-lock`, `actant-ingress`, `actant-compensation`, `actant-drift`.
- **AI-native**: `actant-protocol` (MCP/A2A/AP2 with spend-limit enforcement), `actant-prompts`, `actant-models`, `actant-cache`, `actant-trace`.
- **Contracts**: `actant-contracts` is the single source of truth for every public type. `cargo run -p actant-contracts -- codegen-ts` regenerates `packages/actant-types/src/generated/*`.
- **CLI + SDK**: `actant-schema-dsl`, `actant-sdk-codegen`, `actant-templates`, `actant-codegen-project`.
- **Hot path**: `actant-kernel` (synchronous in-process tool-call dispatcher).

### Demos

- `/examples/test-cleanup/` — the killer "Why did this agent delete the wrong file?" walkthrough on Mastra.
- `/examples/langgraph-router/` — same the codebase, LangGraph-shaped agent.
- `/examples/cli-only/` — pure-CLI variant for the keep-it-tiny crowd.

Each ships a `.actantdb/<project>/events.sqlite` with a recorded run so the demo opens in Studio without re-running.

### Deployment

- `deploy/docker/` — multi-stage Dockerfile (rust:1.88 → distroless), compose with Postgres sidecar.
- `deploy/helm/actantdb/` — Helm chart with Deployment, Service, optional Postgres StatefulSet, PVC for SQLite mode, readiness / liveness probes.
- `docs/book/` — mdbook site materialized from `/specs` + ADRs.
- `bench/` — Criterion benchmarks (`storage_append_event ≈ 60 µs`, `command_append_user_message ≈ 116 µs`, HTTP `/v1/command` median **341 µs**).

### Test coverage (current snapshot)

```
cargo test --workspace         186 Rust passing
pnpm -r test                    25 TS  passing
pnpm smoke                       1 workspace E2E passing
python -m unittest sdks/python   4 passing (1 integration skipped without ACTANTDB_TEST_URL)
                              ────
                              216 tests, 0 failed
```

CI bundle `fmt-check + clippy -D warnings + test + verify-specs + verify-agents` is green.

## Active design constraints (kept from F2/F3 prevention)

These survived the freeze lift because they were the right call regardless:

1. **No new public type outside `actant-contracts`.** If an interface is missing, edit the contract crate and regenerate. Hand-edits to `packages/actant-types/src/generated/*` are forbidden.
2. **Contract update protocol**: edit `actant-contracts` → `cargo run -p actant-contracts -- check-compat` → `… codegen-ts` → commit Rust + regenerated TS in the same PR.
3. **TS-native default install path.** First README line is `npm install`. No Rust toolchain, no Docker, no exposed port required.
4. **Workspace smoke test on every PR** (`pnpm smoke`). If red >24h, freeze new feature work until green.
5. **TS API is identical between `mode: "embedded"` and `mode: "server"`.** Choice is config, not migration.

## Gates and the road from here

[`GATES.md`](./GATES.md) tracks artifact gates only. Status as of the most recent build:

- **Gate 1 — agent substrate:** green. Wrapping, capture, approval, Studio, and replay are implemented and tested.
- **Gate 2 — self-host backend:** green. Embedded, server, storage, auth, tenants, workers, workflows, replay, deployment, and CLI diagnostics are in tree.
- **Gate 3 — compatibility and release discipline:** green. Contracts, generated SDKs, spec verification, agent docs verification, and CI are reproducible from the repo.

[`RELEASE_CHECKLIST.md`](./RELEASE_CHECKLIST.md) covers package and binary release operations.

## Explicit named gaps (not bugs)

From [`CHANGELOG.md`](./CHANGELOG.md) "Deferred":

- **NAPI / WASM bridges** for `@actantdb/core` — declared via `optionalDependencies` but not built.
- **Real cloud-model inference** — `Provider::OpenAi` exists; untested in CI; needs `ACTANTDB_MODEL_API_KEY`.
- **MCP wire transport** — `actant-worker-mcp` returns an envelope by default; the stdio JSON-RPC path is implemented and tested when `python3` is available.
- **Real browser driver** — `actant-worker-browser`'s `EmulatorDriver` is deterministic; a WebDriver/CDP impl is a one-file swap.
- **OIDC RSA signature verification** — discovery + JWKS fetch are real; signature verification delegates to a future `jsonwebtoken` integration.
- **Postgres command-engine plumbing** — `PgStorage` exists with schema; the command engine still hardcodes `SqlitePool` paths.
- **Studio dashboard polish** — React Studio exists; future work is feature depth and panel-level polish.

These are tracked, not silent.

## Reading order

1. [`README.md`](./README.md) — the install pitch.
2. [`CHANGELOG.md`](./CHANGELOG.md) — what landed this session (Phases 1–6 + production-readiness round + the v0.1 baseline).
3. [`SPECS_STATUS.md`](./SPECS_STATUS.md) — per-spec verification (every active spec has a `tests/spec_NN_verification.rs`).
4. [`GATES.md`](./GATES.md) — artifact quality gates.
5. [`RELEASE_CHECKLIST.md`](./RELEASE_CHECKLIST.md) — package and binary release checklist.
6. [`examples/`](./examples) — three runnable end-to-end demos.
7. [`/specs`](./specs), [`/specs/adr`](./specs/adr) — design documents (the substrate that the code implements).
8. [`/agents`](./agents) — the per-crate implementation briefs (still useful as orienting documents).
9. [`/planning`](./planning) — phase plans, lane catalog, performance budgets, SDK plans, Studio design, worker fleet, test strategy, eval catalog, deployment playbook.

The premortem files (`premortem-report-20260517-133422.html`, `premortem-transcript-20260517-133422.md`) stay at the root as reference. The decision to lift the freeze is recorded here.

## What the premortem still teaches

The premortem named real distribution and execution risks. Even with the substrate built, those risks matter once the artifacts are released:

- Mastra still owns the TypeScript volume audience.
- Convex still has the closest direct-competitor architecture.
- Real usage will still reveal problems tests miss, but it is not a repo quality gate.

The substrate ships as both the demo flow and the full backend. Same API, additional capabilities behind it.
