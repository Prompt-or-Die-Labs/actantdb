# ActantDB

**The local-first backend for agent state, authority, replay, memory, and audit.**

ActantDB answers the first operational questions every useful agent creates:
what did the model see, why was this tool allowed, who approved it, what changed,
and can we replay the run from the decision point?

> **Your agent just called a tool. Do you know why?**

ActantDB is not an agent runtime. Keep Mastra, LangGraph, elizaOS, OpenAI
Agents, AI SDK, LangChain, or your hand-rolled loop; ActantDB gives that agent
the backend it is missing. The golden path is a local embedded SQLite ledger via
npm. No cloud account, no daemon, no Docker, and no model API key are required
for the first run.

---

## Golden quickstart

Requires **Node ≥22.5**. Node 24 is recommended because `node:sqlite` is
unflagged there.

```bash
npm create actantdb@latest my-agent -- --template minimal --framework hand-rolled --language js --yes
cd my-agent
npm install
npm start
npm run studio
npm run doctor
```

What that proves:

- `npm start` records one hash-chained run under `./.actantdb`.
- `npm run studio` opens the local timeline for that run.
- `npm run doctor` checks the ledger and schema.

Try the browser-only walkthrough first: [`docs/src/playground.md`](./docs/src/playground.md).
The full quickstart is [`docs/src/golden-quickstart.md`](./docs/src/golden-quickstart.md).

---

## Why this matters

Agents now make decisions that touch files, tickets, email, money, and memory.
A trace tells you what happened. ActantDB stores the authority record that lets
you prove why it happened and replay the run with one variable changed.

That distinction matters because production agents fail in ways tracing alone
cannot answer:

- A model saw a stale memory and proposed a destructive command. ActantDB shows
  the memory that caused it and lets you replay the planner without that memory.
- A workflow stopped mid-run. ActantDB replays from a checkpoint with a stricter
  policy or a different model route.
- An approver accepted a long tool call. ActantDB stores the exact policy
  snapshot, requested action, constrained action, and approval scope.
- A local-first agent must prove private context never entered a forbidden route.
  ActantDB stores the context manifest, blocked items, and guard verdict.

---

## Where it fits

- Keep your existing Mastra, LangGraph, LangChain, elizaOS, Inngest, Trigger.dev, OpenAI Agents SDK, AI SDK, or hand-rolled agent.
- Add backend state for model calls, tool calls, approvals, memory decisions, context manifests, replay diffs, and audit exports.
- Keep the default backend path embedded and local.
- Add the Rust server only when you need shared approvals, HTTP/WebSocket access,
  tenancy, or deployment. SQLite is the default server backend; Postgres is
  wired for the core HTTP command/event/sync surface through
  `ACTANTDB_DATABASE_URL`.
- Export traces and audit rows when a downstream system needs them.

---

## What ships when you go deeper

Concrete state of the repo at the most recent build. Not aspirational.

| Layer                | What ships                                                                                  | Evidence                                       |
| -------------------- | ------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| **npm packages**      | 24 package manifests: 23 `@actantdb/*` packages plus `create-actantdb`. Local workspace version is `0.0.15`. ESM, Node ≥22.5 with Bun first-run smoke coverage. | `packages/*`, `.github/workflows/publish-npm.yml` |
| **Demos**            | 3 runnable demos with recorded SQLite event ledgers (Mastra, LangGraph, pure CLI).            | `examples/test-cleanup/`, `examples/langgraph-router/`, `examples/cli-only/` |
| **Rust workspace**   | 31 crates under `crates/`, 33 Cargo workspace packages including the Rust SDK and bench package. Rust 1.88. Full Phase 1–6 implementation. | `crates/`, `cargo metadata`                    |
| **HTTP server**      | Axum HTTP + WebSocket. TLS (rustls). `actantdb serve --tls-cert/--tls-key`.                   | `crates/actant-server`, `tests/tls.rs`         |
| **Storage**          | SQLite server backend; Postgres storage + command-engine + core HTTP backend. Migration runner. Hash-chained events. Idempotency records. | `crates/actant-storage`, `crates/actant-command`, `crates/actant-server`, `migrations/` |
| **Auth**             | JWT HS256 + OIDC discovery/JWKS with 1-hour cache.                                            | `crates/actant-auth`                           |
| **Multi-tenant**     | `actant-tenant` with cross-tenant guards. Role checks.                                        | `crates/actant-tenant`                         |
| **Workers**          | Shell, file, model (mock + OpenAI-compat), MCP (stdio JSON-RPC), browser (emulator + driver trait), email, slack, manager. | `crates/actant-worker-*`                       |
| **Workflows**        | Runner state machine with approval-gate pause/resume. Cron scheduler with watch-channel shutdown. | `crates/actant-flow`, `crates/actant-trigger`  |
| **Replay**           | 4 working modes (recorded / model / policy / memory) + 3 explicit deferrals (experimental / tool / local_only) returning named errors. | `crates/actant-replay`                         |
| **Reliability**      | Throttle (token bucket), circuit breaker, leases, locks, DLQ, compensations, drift detection.   | `crates/actant-reliability`, `crates/actant-{compensation,drift}` |
| **AI-native**        | Context manifests with sensitivity ceiling. Memory candidate→approval→use lifecycle with conflict detection. Hybrid retrieval (dense + sparse + graph + local rerank). MCP+A2A+AP2 protocol types. | `crates/actant-{context,memory,embed,core}` |
| **Observability**    | OTel GenAI + OpenInference schema columns. W3C-style trace+span ids. Single redaction chokepoint.      | `crates/actant-core`                          |
| **Audit + retention**| Nightly export per workspace. `purge_by_policy` deletes events past retention.                  | `crates/actant-audit-export`                   |
| **Deployment**       | Multi-stage Dockerfile (rust:1.88 → distroless). Docker Compose server stack with SQLite volume + Caddy + Mailpit. Helm chart exists; `ACTANTDB_DATABASE_URL` enables Postgres-backed core HTTP mode. | `deploy/`, `deploy/helm/` |
| **Docs site**        | mdbook materialized from `/specs` + ADRs + operations. 20 specs + 18 ADRs.                      | `docs/book.toml`, `docs/build.sh`              |
| **Python SDK**       | pip-installable sync + asyncio client, typed public errors, and dependency-free LangChain/CrewAI/AutoGen adapters. | `sdks/python/` |
| **Swift SDK**        | Two-tier (Swift 6.3, macOS 26 / iOS 26). `ActantDB` is a 1:1 HTTP+WS client; `ActantAgent` is the opinionated facade (Session / MemoryStore / Auditor / ApprovalCenter / ReplayClient / RelationshipStore / ActantDBSupervisor) that lets a consumer like Swoosh add ActantDB by one-line conformance extensions. | `sdks/swift/` |
| **Benchmarks**       | HTTP single-session: p50 **464 µs**, p95 **1.00 ms**, p99 **2.12 ms** (1.8k req/s). 10-concurrent: **3.9k req/s** aggregate. Replay 200-event run end-to-end: **3.4 ms**. RSS only +1.4 MB per 10k events. Full table in [`BENCHMARKS.md`](./BENCHMARKS.md). | `bench/`, [`BENCHMARKS.md`](./BENCHMARKS.md) |
| **Tests**            | CI runs TypeScript build/test, smoke, Rust fmt/clippy/tests, spec verification, and agent-doc verification. Local docs/build checks are documented per surface. | [`TESTING.md`](./TESTING.md), `.github/workflows/ci.yml` |
| **Spec verification**| Every active spec has `tests/spec_NN_verification.rs`. The harness caught **8 real drifts** before they shipped (event-name drift, missing diff kinds, FK ordering, etc.). | `SPECS_STATUS.md`                              |
| **CI bundle**        | `fmt-check + clippy -D warnings + test + verify-specs + verify-agents` are wired in CI; current local proof is listed in `TESTING.md`. | `.github/workflows/ci.yml`, `justfile`, [`TESTING.md`](./TESTING.md) |

The repo-verifiable quality gates are tracked in [`GATES.md`](./GATES.md); current local and CI coverage is listed in [`TESTING.md`](./TESTING.md). [`RELEASE_CHECKLIST.md`](./RELEASE_CHECKLIST.md) covers release operations.

---

## Integrations

ActantDB ships a **Model Context Protocol** server (`@actantdb/mcp-server`) that exposes the local ledger to any MCP client: Claude Desktop, Cursor, Continue, and Cline. The same binary speaks stdio and Streamable HTTP. Drop this block into `~/Library/Application Support/Claude/claude_desktop_config.json` and restart Claude Desktop to see `list_runs`, `query_predicate`, `replay`, `decide_approval`, and friends in the tool picker:

```json
{
  "mcpServers": {
    "actantdb": {
      "command": "npx",
      "args": ["-y", "@actantdb/mcp-server", "--stdio"],
      "env": { "ACTANTDB_STORE_DIR": "/Users/you/.actantdb" }
    }
  }
}
```

New project? `npm create actantdb@latest my-app` scaffolds a runnable wrapper, the right `@actantdb/*` deps, `npm run studio`, and `npm run doctor`. Writing tests against an agent? `@actantdb/testing` gives you `createTestLedger() + expectEventEmitted() + expectGuardVerdict()` so consumer tests don't need to reach into the ledger by hand. See [`docs/recipes/`](./docs/recipes) for focused local how-tos.

---

## Install (the substrate)

Requires **Node ≥22.5** or **Bun ≥1.3** for embedded mode. `@actantdb/core` uses `node:sqlite` on Node, which is unflagged starting at Node 24 (Node 22 needs `--experimental-sqlite`), and `bun:sqlite` on Bun. Consumer apps can install with npm, pnpm, or Bun; the packages do not require the ActantDB repo's maintainer toolchain.

```bash
# Add backend capture to your agent:
npm install @actantdb/mastra

# LangGraph package name:
npm install @actantdb/langgraph

# Durable workflow package names:
npm install @actantdb/inngest @actantdb/triggerdev

# elizaOS package name:
npm install @actantdb/elizaos

# Studio CLI (dev-only — provides `npx actantdb`):
npm install --save-dev @actantdb/studio
```

`@actantdb/mastra` works with **any** agent that exposes a tools record — Mastra, LangGraph, OpenAI Agents SDK, hand-rolled. `@actantdb/langgraph` is the LangGraph-named package for the same thin wrapper. `@actantdb/elizaos` wraps elizaOS actions/plugin-shaped runtime objects without becoming the Eliza agent. `@actantdb/inngest` and `@actantdb/triggerdev` wrap handler/task functions without taking a framework dependency. If you're using Mastra, also `npm install @mastra/core`; for other framework-name packages no extra peer is needed.

Embedded storage boundary: embedded `@actantdb/core` needs Node or Bun SQLite plus a real filesystem. Python, Swift, Rust, Deno, Edge runtimes, and browsers should use the HTTP SDK path unless their runtime explicitly supports that embedded storage contract. See [`docs/RUNTIME_GUIDANCE.md`](./docs/RUNTIME_GUIDANCE.md).

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

The wrapper works on Mastra, LangGraph, and any agent that exposes a tools record. The named Inngest and Trigger.dev packages wrap handler-style workflows. You keep your framework. ActantDB adds the flight recorder and the authority gate.

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

// Scale-out — point at an actantdb-server. HTTP surface, SQLite-backed today.
const actant = await createActant({ mode: "server", url: process.env.ACTANTDB_URL });
```

When you cross the line where embedded isn't enough — shared approvals, HTTP/WebSocket access, or deployment behind TLS — run the Rust server. The default server persists to SQLite. Set `ACTANTDB_DATABASE_URL=postgres://...` to run the Postgres-backed core HTTP surface: health, metadata, typed commands, events, and sync pulls. SQLite-only Studio/admin routes return explicit 501 in Postgres mode.

---

## The Rust substrate (what's under the substrate)

```
crates/
├── actant-contracts/      SINGLE source of truth. Every public type, error,
│                          event, command, schema. `cargo run -p actant-contracts --bin actant-contracts --
│                          {diff,check-compat,codegen-ts,codegen-py,codegen-swift}`.
│                          Hand-edits to the generated TS are forbidden.
├── actant-core / actant-storage / actant-command / actant-policy
│                          Phase 1 core. 10 alpha commands.
├── actant-effects + actant-worker-protocol + actant-workers
│                          Phase 2. Lease-based effect queue + worker fleet.
├── actant-context / actant-memory / actant-embed / actant-embedders
│                          Phase 3. Context firewall, memory lifecycle, hybrid retrieval.
├── actant-flow / actant-trigger
│                          Phase 4. Durable workflows + cron/event triggers.
├── actant-replay
│                          Phase 5. Four working replay modes + 3 named deferrals.
├── actant-auth / actant-tenant / actant-audit-export / actant-sync
│                          Phase 6. JWT + OIDC, multi-tenant, audit export, cluster sync.
├── actant-server / actant-cli / actant-subscribe
│                          HTTP+WS server, `actantdb` CLI binary, hot-path kernel.
├── actant-reliability / actant-compensation / actant-drift
│                          Reliability primitives.
├── actant-schema-dsl / actant-templates
│                          Developer-tools surface.
├── actant-eval / actant-ffi
│                          Self-improvement primitives + embeddable FFI.
```

Every active spec under `/specs` (20 specs + 18 ADRs) has a corresponding `tests/spec_NN_verification.rs` regression gate. Breaking a spec's `## Verification` clause fails CI.

Rust import migration after the crate consolidation:

```rust
use actantdb::policy::{ActantCapsule, ActantTrustProfile, MemoryAllowed};
use actantdb::command::{
    dispatch_tool_call, ActantCache, ActantHotToolCall, ActantModelRegistry,
    ActantPromptTemplate,
};
use actantdb::core::{
    new_span_id, new_trace_id, ActantA2aCard, ActantAp2Mandate, ActantMcpServer,
};
use actantdb::memory::{ActantIndex, ActantSearchOptions};
use actantdb::contracts::sdk_codegen;
```

---

## What ActantDB is NOT

- Not a Mastra/LangGraph/CrewAI replacement — `@actantdb/mastra` wraps the agent you already have.
- Not a Convex/Temporal/Inngest replacement — the substrate ships durable workflows, but you can keep your existing backend and use ActantDB only for replay + authority.
- Not a new agent framework, language, or runtime.
- Not a hosted service. Everything in this repo runs on your machine, on your server, or in your cluster.

---

## Reproduce

```bash
# Rust fast workspace check
cargo check --workspace --all-targets
just check           # same fast check when just is installed
just ci              # fmt-check + clippy -D warnings + test + verify-specs + verify-agents

# Focused Rust tests used locally; CI owns the full matrix.
cargo test -p actant-storage
cargo test -p actant-sync
cargo test -p actant-replay
cargo test -p actantdb-client
cargo test -p actant-server --lib
cargo test -p actant-subscribe --lib

# TypeScript
pnpm install
pnpm -r build
pnpm -r test
pnpm smoke           # workspace E2E

# Bun lane
bun install --frozen-lockfile
bun run build:bun
bun run test:bun
bun run smoke:bun
bun run smoke:bun-create:bun

# Python SDK (14 local tests + 1 server integration skip unless ACTANTDB_TEST_URL is set)
(cd sdks/python && python3 -m unittest discover -s tests)

# The killer demo end-to-end
pnpm --filter actant-demo-test-cleanup demo
ACTANTDB_STORE_DIR=./examples/test-cleanup/.actantdb \
  npx actantdb studio --project demo-test-cleanup

# Regenerate TS types from Rust contracts
cargo run -p actant-contracts --bin actant-contracts -- check-compat
cargo run -p actant-contracts --bin actant-contracts -- codegen-ts

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

- **Gate 1 — agent substrate:** green. Wrapping, capture, approval, Studio, and replay are implemented and tested.
- **Gate 2 — self-host backend:** mostly green. Embedded, SQLite server, auth, workers, workflows, replay, deployment, CLI diagnostics, and Postgres core HTTP mode are in tree; remaining Postgres work is the SQLite-only Studio/admin route layer.
- **Gate 3 — compatibility and release discipline:** green. Contracts, generated SDKs, spec verification, agent docs verification, and CI are reproducible from the repo.

[`GATES.md`](./GATES.md) is the artifact gate ledger. [`RELEASE_CHECKLIST.md`](./RELEASE_CHECKLIST.md) covers release operations. [`CHANGELOG.md`](./CHANGELOG.md) enumerates what landed. [`SPECS_STATUS.md`](./SPECS_STATUS.md) maps every spec to its verifier test. [`PIVOT.md`](./PIVOT.md) captures the freeze-lift decision and the current substrate shape.

---

## Repository layout

```
/
├── README.md                       — you are here
├── PIVOT.md                        — current state (post freeze-lift)
├── CHANGELOG.md                    — what landed this session
├── SPECS_STATUS.md                 — per-spec verification (216 tests)
├── GATES.md                        — artifact quality gates
├── RELEASE_CHECKLIST.md            — package and binary release checklist
├── CLAUDE.md                       — guidance for Claude Code sessions
├── packages/                       — 19 package manifests (TypeScript first)
├── crates/                         — Cargo workspace, 31 crates, Rust 1.88
├── migrations/                     — SQLite + Postgres migrations
├── specs/                          — 20 specs + 18 ADRs
├── agents/                         — per-crate implementation briefs
├── planning/                       — phase plans, perf budgets, deployment playbook, Studio/SDK design
├── examples/                          — 3 runnable end-to-end demos
├── sdks/                           — sdks/python/, sdks/rust/, sdks/swift/
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
