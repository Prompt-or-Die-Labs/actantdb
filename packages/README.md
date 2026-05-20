# packages/

The product surface. TypeScript packages distributed via npm, ESM-only, TypeScript 5.4+ strict, pnpm workspace.

| Package              | What it does                                                                |
| -------------------- | --------------------------------------------------------------------------- |
| `@actantdb/mastra`   | `withActant(agent, opts)` duck-typed wrapper. Captures every tool call, runs Guard, supports approval flows, exposes the timeline to Studio. Works on Mastra, LangGraph, and hand-rolled agents. |
| `@actantdb/langgraph` | LangGraph package name for the same thin `withActant` wrapper.              |
| `@actantdb/inngest`  | Inngest handler wrapper that records each function invocation as an embedded ActantDB run. |
| `@actantdb/triggerdev` | Trigger.dev task wrapper that records each task invocation as an embedded ActantDB run. |
| `@actantdb/elizaos`  | elizaOS action/runtime/plugin wrapper that records action execution as embedded ActantDB runs. |
| `@actantdb/core`     | Local ledger backed by `node:sqlite`, hash-chained events, monotonic ULIDs, idempotency. |
| `@actantdb/policy`   | Verdict builders + the alpha-demo policy (`rm -rf …/dist` constrain hint).  |
| `@actantdb/replay`   | Checkpoint / run / diff with memory + policy overrides.                     |
| `@actantdb/studio`   | Local HTTP server + UI with timeline, detail panel, replay modal, side-by-side diff. CLI: `studio | approve | deny | replay create|run|diff | approvals`. |
| `@actantdb/types`    | Generated from `crates/actant-contracts` via `codegen-ts`. Hand-edits forbidden. |
| `@actantdb/sdk`      | HTTP + WS client for the Rust server. Use this when an embedded `@actantdb/core` outgrows the laptop. |
| `@actantdb/convex`   | Adapter for Convex's `handler(ctx, args)` tool shape.                       |

## Conventions

- ESM only. No CJS.
- All cross-package types come from `@actantdb/types`. Adding a public type to any other package is a PR rejection (see [`/CLAUDE.md`](../CLAUDE.md) §F3).
- `engines.node >= 22.5` for packages that import `node:sqlite`.
- Each package has `package.json`, `tsconfig.json`, `src/index.ts`, `README.md`, and a `src/*.test.ts` next to anything non-trivial.

## Build / test

```bash
pnpm install
pnpm -r build
pnpm -r test
pnpm smoke           # workspace E2E (must pass on every PR)
```

Per-package:

```bash
pnpm --filter @actantdb/<pkg> test
pnpm --filter @actantdb/<pkg> test -- <pattern>
```

## How this connects to the Rust crates

`@actantdb/core` is the embedded path: a TypeScript ledger on `node:sqlite` or Bun's `bun:sqlite`. When a workload outgrows it, `@actantdb/sdk` points at an `actantdb-server` (the Rust `actant-server` binary) and the TypeScript surface stays identical (`actant.command(...)`, `actant.approvals.pending()`, `actant.replay.fromEvent(...)`).

The NAPI-RS / WASM bridges that bundle the Rust kernel directly inside `@actantdb/core` are declared as `optionalDependencies` but not built yet — that's a named deferred item in [`/CHANGELOG.md`](../CHANGELOG.md). Today the embedded path is pure TypeScript; the server path is Rust.

## Demo packages (workspace members, not published)

| Package                            | Where                       | Shows                                                 |
| ---------------------------------- | --------------------------- | ----------------------------------------------------- |
| `actant-demo-test-cleanup`         | `/examples/test-cleanup/`              | The killer "deleted the wrong file" walkthrough.       |
| `actant-demo-langgraph-router`     | `/examples/langgraph-router/`    | The same demo on a LangGraph-shaped router.           |
| `actant-demo-cli`                  | `/examples/cli-only/`          | Pure-CLI variant.                                     |

`pnpm-workspace.yaml` includes both `packages/*` and `examples/test-cleanup*` so they share the workspace `@actantdb/*` versions.
