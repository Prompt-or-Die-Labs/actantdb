# ActantDB

**The event-sourced backend for autonomous agents.** Built-in replay, runtime authority gating, durable workflows, governed memory, hybrid retrieval, OpenTelemetry-compatible traces, and multi-tenant deployment — installable as an npm package, deployable as a Rust server.

It is also, in five words:

> **Your agent just called a tool. Do you know why?**

ActantDB records what the model saw, what action it requested, who approved it, what happened, and lets you replay the run from any decision point — without replacing Mastra, Convex, or LangGraph.

---

## What's here today

Concrete state of the repo at the most recent build. Not aspirational.

| Layer                | What ships                                                                                  | Evidence                                       |
| -------------------- | ------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| **npm packages**      | 8 TypeScript packages: `@actantdb/mastra`, `core`, `policy`, `replay`, `studio`, `types`, `sdk`, `convex`. ESM, Node ≥22.5. | `packages/*`, `.github/workflows/publish-npm.yml` |
| **Demos**            | 3 runnable demos with recorded SQLite event ledgers (Mastra, LangGraph, pure CLI).            | `examples/test-cleanup/`, `examples/langgraph-router/`, `examples/cli-only/` |
| **Rust workspace**   | 49 crates, ~15k lines, Rust 1.88. Full Phase 1–6 implementation.                            | `crates/`, `cargo metadata`                    |
| **HTTP server**      | Axum HTTP + WebSocket. TLS (rustls). `actantdb serve --tls-cert/--tls-key`.                   | `crates/actant-server`, `tests/tls.rs`         |
| **Storage**          | SQLite + Postgres backends. Migration runner. Hash-chained events. Idempotency records.       | `crates/actant-storage`, 3 migrations          |
| **Auth**             | JWT HS256 + OIDC discovery/JWKS with 1-hour cache.                                            | `crates/actant-auth`                           |
| **Multi-tenant**     | `actant-tenant` with cross-tenant guards. Role checks.                                        | `crates/actant-tenant`                         |
| **Workers**          | Shell, file, model (mock + OpenAI-compat), MCP (stdio JSON-RPC), browser (emulator + driver trait), email, slack, manager. | `crates/actant-worker-*`                       |
| **Workflows**        | Runner state machine with approval-gate pause/resume. Cron scheduler with watch-channel shutdown. | `crates/actant-flow`, `crates/actant-trigger`  |
| **Replay**           | 4 working modes (recorded / model / policy / memory) + 3 explicit deferrals (experimental / tool / local_only) returning named errors. | `crates/actant-replay`                         |
| **Reliability**      | Throttle (token bucket), circuit breaker, leases, locks, DLQ, compensations, drift detection.   | `crates/actant-{throttle,circuit,lock,compensation,drift}` |
| **AI-native**        | Context manifests with sensitivity ceiling. Memory candidate→approval→use lifecycle with conflict detection. Hybrid retrieval (dense cosine). MCP+A2A+AP2 protocol types. | `crates/actant-{context,memory,index,embed,protocol}` |
| **Observability**    | OTel GenAI + OpenInference traces. W3C-style trace+span ids. Single redaction chokepoint.      | `crates/actant-trace`                          |
| **Audit + retention**| Nightly export per workspace. `purge_by_policy` deletes events past retention.                  | `crates/actant-audit-export`                   |
| **Deployment**       | Multi-stage Dockerfile (rust:1.88 → distroless). Helm chart (Deployment + Service + optional Postgres + PVC + 3 health probes). | `deploy/docker/`, `deploy/helm/`               |
| **Docs site**        | mdbook materialized from `/specs` + ADRs + operations. 20 specs + 18 ADRs.                      | `docs/book.toml`, `docs/build.sh`              |
| **Python SDK**       | pip-installable, mirrors the TS SDK surface. Integration test passes against a real server.    | `sdks/python/`                                 |
| **Swift SDK**        | Two-tier (Swift 6.3, macOS 26 / iOS 26). `ActantDB` is a 1:1 HTTP+WS client; `ActantAgent` is the opinionated facade (Session / MemoryStore / Auditor / ApprovalCenter / ReplayClient / RelationshipStore / ActantDBSupervisor) that lets a consumer like Swoosh add ActantDB by one-line conformance extensions. | `sdks/swift/` |
| **Benchmarks**       | HTTP single-session: p50 **464 µs**, p95 **1.00 ms**, p99 **2.12 ms** (1.8k req/s). 10-concurrent: **3.9k req/s** aggregate. Replay 200-event run end-to-end: **3.4 ms**. RSS only +1.4 MB per 10k events. Full table in [`BENCHMARKS.md`](./BENCHMARKS.md). | `bench/`, [`BENCHMARKS.md`](./BENCHMARKS.md) |
| **Tests**            | **331 Rust + 25 TypeScript + 10 Python + 62 Swift + 1 workspace smoke = 429 passing.** 0 failed.            | `cargo test --workspace`, `pnpm -r test`, `pnpm smoke`, `swift test --package-path sdks/swift` |
| **Spec verification**| Every active spec has `tests/spec_NN_verification.rs`. The harness caught **8 real drifts** before they shipped (event-name drift, missing diff kinds, FK ordering, etc.). | `SPECS_STATUS.md`                              |
| **CI bundle**        | `fmt-check + clippy -D warnings + test + verify-specs + verify-agents`. Green.                  | `.github/workflows/ci.yml`, `justfile`         |

Gate 1 (MVP) is implementation-complete. Gates 2 + 3 are blocked on external adoption — `npm publish` + design-partner outreach. See [`GATES.md`](./GATES.md) for the punch list and [`RELEASE_CHECKLIST.md`](./RELEASE_CHECKLIST.md) for the 5-step path to close them.

---

## Why this matters

Agents in 2026 are moving out of chat boxes and into production: shipping commits, sending email, moving money, running for minutes or hours or days, calling tools that change real-world state. A trace tells you *what* happened. ActantDB tells you *why* — and lets you re-execute the run with one variable changed.

That distinction matters because production agents fail in ways tracing alone cannot answer:

- A model saw a stale memory and proposed a destructive command. A trace shows the command. ActantDB shows the *memory that caused it* and lets you replay the planner without that memory.
- A workflow ran for 14 hours and stopped mid-way. A trace shows the last span. ActantDB replays from any checkpoint with a different policy or model.
- An approver clicked "yes" on a long tool call. A trace shows the click. ActantDB shows the exact policy snapshot the approver was looking at and the constrained variant they accepted.
- A regulated workspace needs to prove no agent leaked HIPAA-class content to a cloud model. A trace shows the call. ActantDB shows the **context manifest**: what the model saw, what was blocked, and which capsule's sensitivity ceiling fired.

That gap is the product. The 2026 agent-tooling market has good frameworks (Mastra, LangGraph, OpenAI Agents SDK, CrewAI) and good backends (Convex, Temporal, Inngest). It does not have a governed accountability layer. ActantDB is that layer.

---

## What you can build with it

This isn't theoretical. Each of the things below is supported by code that compiles and tests that pass in this repo:

- **Production coding agents** with reviewable approval flows, replayable failures, and memory provenance you can audit. Demo: [`examples/test-cleanup/`](./examples/test-cleanup).
- **Multi-step research agents** that survive process restarts, pause for human approval for hours or days, retry against flaky providers, and resume cleanly. (`actant-flow` + `actant-trigger` + `actant-effects`.)
- **Customer-support agents** with reviewable memory candidates, conflict detection between contradictory memories, and a replay-on-complaint workflow that proves what the agent told the customer and why. (`actant-memory` + `actant-replay`.)
- **Regulated-industry agents** (healthcare, finance) with `actant-audit-export` nightly exports, `purge_by_policy` retention enforcement, AP2 mandate types with spend-limit enforcement, and a context firewall that refuses to send `Secret`-class content to cloud routes. (`actant-audit-export` + `actant-protocol` + `actant-context`.)
- **Multi-agent task boards** with delegation, per-agent budgets (cost / tokens / tool-call counts), trust profiles that update from observed behavior, and drift detection that flags off-mandate actions. (`actant-trust` + `actant-drift` + `actant-protocol`.)
- **Self-hosted agent backends** for teams: JWT or OIDC auth, multi-tenant boundary with cross-tenant guards, Postgres backend, Helm chart, TLS termination, three-tier health probes, graceful shutdown, per-endpoint rate limits. (`actant-auth` + `actant-tenant` + `actant-server` + `deploy/helm/`.)
- **Local-first personal agents** (Mac, Linux, Windows) with private memory, capsule-bound sensitivity, and replay that runs entirely on-device. `npm install @actantdb/mastra` + `npx actantdb studio` and nothing leaves the machine.
- **Agent-tool MCP servers** that get governed through ActantDB without changing the MCP server. The MCP wire protocol is implemented over stdio JSON-RPC; the worker claims `mcp.call` effects from the same queue as everything else. (`actant-worker-mcp`.)
- **Cross-framework portability.** The same `withActant()` wrapper runs on Mastra, LangGraph, and hand-rolled agents — proven by three public examples. (`@actantdb/mastra` + `examples/langgraph-router/`.)
- **Anything observable through OpenTelemetry + OpenInference.** Spans for model_call, tool_call, retrieval, reranker, embedding, agent, workflow, approval, replay; metrics for queue depth, model latency, token usage, retry counts, budget remaining, cache hit rate. Export to Phoenix, Arize, LangSmith, Datadog, Grafana, Honeycomb. (`actant-trace`.)

---

## Install (the substrate)

Requires **Node ≥22.5**. `@actantdb/core` uses `node:sqlite`, which is unflagged starting at Node 24 (Node 22 needs `--experimental-sqlite`).

```bash
# Wrap your agent (runtime dep):
npm install @actantdb/mastra

# Studio CLI (dev-only — provides `npx actantdb`):
npm install --save-dev @actantdb/studio
```

`@actantdb/mastra` works with **any** agent that exposes a tools record — Mastra, LangGraph, OpenAI Agents SDK, hand-rolled. If you're using Mastra, also `npm install @mastra/core`; for other frameworks no extra peer is needed.

Snippets below are valid in plain `.mjs` (no TypeScript needed). Save as `agent.mjs` and run `node agent.mjs`.

```js
// agent.mjs — wrap any agent, capture every tool call.
import { withActant } from "@actantdb/mastra";
import { demoPolicy } from "@actantdb/policy";

const wrapped = withActant(myAgent, {
  project: "prod-support-agent",
  policy: demoPolicy,
});

await wrapped.run({ message: "Clean up the test artifacts." });

// Direct ledger access — query events your wrapped agent captured:
const events = wrapped.actant.ledger.query({});
console.log(`captured ${events.length} events`);
```

If you just want the embedded ledger (no agent framework):

```js
// ledger.mjs — append-only hash-chained event ledger, no daemon.
import { openLedger, ulid } from "@actantdb/core";

const ledger = openLedger({ project: "demo" });
const runId = ulid();
ledger.append({ kind: "user_message", runId, payload: { text: "hello" } });
for (const e of ledger.query({ runId })) console.log(e.kind, e.chain_hash);
ledger.close();
```

```bash
npx actantdb studio --project prod-support-agent
```

Open the timeline. Click any tool call. See the model context, the approval scope, the tool result. Click **Replay** to rerun from before that decision — with or without a memory, with a stricter policy, with a different model.

The wrapper works on Mastra, LangGraph, and any agent that exposes a tools record. You keep your framework. ActantDB adds the flight recorder and the authority gate.

---

## The hero diff

What Studio shows when you replay the killer-demo run with `mem_42_dist` excluded and a stricter policy:

```
event              original (recorded)            replay (under overrides)
─────────────────────────────────────────────────────────────────────────────
context_build      3 included, 0 blocked          2 included, 1 blocked  ◀ mem_42 blocked
model_call         "rm -rf build dist"            "rm -rf build"
guard_verdict      require_approval (constrain)   allow
tool_call          shell.run "rm -rf build"       shell.run "rm -rf build"
effect_completed   exit=0                         exit=0  (recorded reuse)
```

> Without `mem_42`, the planner would have proposed the safe command directly. The memory caused the risky proposal; Guard caught it; replay proves the causal link.

Try it:

```bash
pnpm install
pnpm --filter actant-demo-test-cleanup demo
ACTANTDB_STORE_DIR=./examples/test-cleanup/.actantdb \
  npx actantdb studio --project demo-test-cleanup
```

---

## Embedded today, scale-out when you need it

Same TypeScript API, two modes:

```ts
// Embedded — npm install only. Local SQLite ledger via node:sqlite. No daemon.
const actant = await createActant({ mode: "embedded", project: "p1" });

// Scale-out — point at an actantdb-server. Identical surface.
const actant = await createActant({ mode: "server", url: process.env.ACTANTDB_URL });
```

When you cross the line where embedded isn't enough — multi-user approvals, regulated audit, cluster sync, OIDC SSO — the Rust server is the same backend you've been running locally. Cargo build it, drop it in Helm, point your existing `@actantdb/mastra` wrappers at it. The TS API doesn't change.

---

## The Rust substrate (what's under the substrate)

```
crates/
├── actant-contracts/      SINGLE source of truth. Every public type, error,
│                          event, command, schema. `cargo run -p actant-contracts --
│                          {diff,check-compat,codegen-ts,codegen-py,codegen-swift}`.
│                          Hand-edits to the generated TS are forbidden.
├── actant-core / actant-storage / actant-command / actant-policy
│                          Phase 1 core. 10 alpha commands.
├── actant-effects + actant-worker-{protocol,shell,file,model,mcp,browser,email,slack,manager}
│                          Phase 2. Lease-based effect queue + worker fleet.
├── actant-context / actant-memory / actant-embed / actant-embedders / actant-index
│                          Phase 3. Context firewall, memory lifecycle, hybrid retrieval.
├── actant-flow / actant-trigger
│                          Phase 4. Durable workflows + cron/event triggers.
├── actant-replay
│                          Phase 5. Four working replay modes + 3 named deferrals.
├── actant-auth / actant-tenant / actant-audit-export / actant-sync
│                          Phase 6. JWT + OIDC, multi-tenant, audit export, cluster sync.
├── actant-server / actant-cli / actant-kernel / actant-subscribe
│                          HTTP+WS server, `actantdb` CLI binary, hot-path kernel.
├── actant-throttle / actant-circuit / actant-lock / actant-ingress / actant-compensation / actant-drift
│                          Reliability primitives.
├── actant-protocol / actant-prompts / actant-models / actant-cache / actant-trace
│                          AI-native primitives.
├── actant-schema-dsl / actant-sdk-codegen / actant-templates / actant-codegen-project
│                          Developer-tools surface.
├── actant-eval / actant-capsule / actant-trust
│                          Self-improvement primitives.
└── actant-napi / actant-wasm
                          Bridge stubs (declared via @actantdb/core optionalDependencies;
                          NAPI/WASM build is a named deferral in CHANGELOG.md).
```

Every active spec under `/specs` (20 specs + 18 ADRs) has a corresponding `tests/spec_NN_verification.rs` regression gate. Breaking a spec's `## Verification` clause fails CI.

---

## What ActantDB is NOT

- Not a Mastra/LangGraph/CrewAI replacement — `@actantdb/mastra` wraps the agent you already have.
- Not a Convex/Temporal/Inngest replacement — the substrate ships durable workflows, but you can keep your existing backend and use ActantDB only for replay + authority.
- Not a new agent framework, language, or runtime.
- Not a hosted service. Everything in this repo runs on your machine, on your server, or in your cluster.

---

## Reproduce

```bash
# Rust (186 tests, ~49 crates)
cargo test --workspace
just check           # cargo check --workspace --all-targets (fast)
just ci              # fmt-check + clippy -D warnings + test + verify-specs + verify-agents

# TypeScript (25 vitest tests)
pnpm install
pnpm -r build
pnpm -r test
pnpm smoke           # workspace E2E

# Python SDK (4 tests; integration test runs only with ACTANTDB_TEST_URL set)
(cd sdks/python && python3 -m unittest discover -s tests)

# The killer demo end-to-end
pnpm --filter actant-demo-test-cleanup demo
ACTANTDB_STORE_DIR=./examples/test-cleanup/.actantdb \
  npx actantdb studio --project demo-test-cleanup

# Regenerate TS types from Rust contracts
cargo run -p actant-contracts -- check-compat
cargo run -p actant-contracts -- codegen-ts

# Benchmark HTTP /v1/command
cargo bench -p actant-bench --bench http_command -- --sample-size 20
```

---

## Compared to the competitive landscape

See [`COMPARISON.md`](./COMPARISON.md) for a 13-row feature matrix vs. 14 competitors (Temporal, Inngest, Restate, DBOS, Mastra, LangGraph, OpenAI Agents, CrewAI, Langfuse, LangSmith, Phoenix, Helicone, Mem0, Zep) — with sourced vendor docs, honest "where competitors win" notes, and routing guidance for when to use each.

Three places ActantDB is genuinely uncontested in 2026:

1. **Hash-chained tamper-evident ledger** — every event carries `payload_hash + chain_hash`. No competitor in the surveyed set provides this.
2. **Runtime Guard verdict as a typed ledger event** — the policy snapshot, decision, and reason are first-class rows you can replay against. Closest analog (LangGraph interrupts) is per-node, not policy-typed.
3. **Capsule-bound context with sensitivity ceiling** — context is sealed by capsule + enforced before model dispatch, not just labeled in traces.

---

## Status

- **Gate 1 — MVP** (target 2026-06-30): implementation-complete. Three runnable demos. Human leftovers: 90-second screencast, hero PNG, three-platform install verification.
- **Gate 2 — external adoption** (target 2026-07-31): blocked on first `npm publish` + outreach. The repo ships a manual-trigger publish workflow at [`.github/workflows/publish-npm.yml`](./.github/workflows/publish-npm.yml) that builds, tests, and publishes every `@actantdb/*` package under the `shadow` dist-tag.
- **Gate 3 — shipped/staged** (target 2026-08-17): blocked on landing 5 non-Wes developers + 1 named design partner.

[`GATES.md`](./GATES.md) is the punch list. [`RELEASE_CHECKLIST.md`](./RELEASE_CHECKLIST.md) is the 5-step sequence to close Gates 2 + 3. [`CHANGELOG.md`](./CHANGELOG.md) enumerates what landed. [`SPECS_STATUS.md`](./SPECS_STATUS.md) maps every spec to its verifier test. [`PIVOT.md`](./PIVOT.md) captures the freeze-lift decision and the current current substrate shape.

---

## Repository layout

```
/
├── README.md                       — you are here
├── PIVOT.md                        — current state (post freeze-lift)
├── CHANGELOG.md                    — what landed this session
├── SPECS_STATUS.md                 — per-spec verification (216 tests)
├── GATES.md                        — Gate 1/2/3 punch list
├── RELEASE_CHECKLIST.md            — 5 steps to close Gates 2 + 3
├── CLAUDE.md                       — guidance for Claude Code sessions
├── packages/                       — 8 npm packages (TypeScript first)
├── crates/                         — Cargo workspace, ~49 crates, Rust 1.88
├── migrations/                     — 3 SQL migrations, ~80 tables
├── specs/                          — 20 specs + 18 ADRs
├── agents/                         — per-crate implementation briefs
├── planning/                       — phase plans, perf budgets, deployment playbook, Studio/SDK design
├── examples/                          — 3 runnable end-to-end demos
├── sdks/                           — sdks/python/
├── bench/                          — Criterion + HTTP load benches
├── deploy/                         — docker/ + helm/
├── docs/                           — mdbook site materialized from /specs
├── scripts/                        — smoke.mjs and friends
└── premortem-{report,transcript}-20260517-*.{html,md}
```

---

## Sources / prior art

Mastra 1.0, Convex Agents + Workflow component, OpenInference + OpenTelemetry GenAI conventions, SpacetimeDB reducers, Temporal durable execution, NAPI-RS, FastEmbed. The premortem at the repo root (2026-05-17) cites every claim about the May 2026 market landscape.

---

## License

Apache 2.0. See [LICENSE](./LICENSE).
