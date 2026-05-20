# GATES — artifact verification status

This file tracks only things this repository can prove with code, docs, tests,
or CI. Market-facing outcomes are outside this score.

Source of truth for gate definitions: [PIVOT.md §Hard validation gates](./PIVOT.md).

## Gate 1 — Agent substrate

Acceptance criterion:

> An agent can be wrapped, every relevant decision is captured, authority is
> gated, and the run can be inspected and replayed locally.

| Item | Status | Evidence |
| --- | --- | --- |
| `@actantdb/mastra` wraps a Mastra-shaped agent | ✅ | [`packages/actant-mastra/src/index.ts`](./packages/actant-mastra/src/index.ts) accepts any agent with `tools: Record<string, {execute}>`. |
| Captures tool calls | ✅ | `tool_call_requested` / `tool_call_started` / `tool_call_completed`; round-trip covered in [`packages/actant-mastra/src/index.test.ts`](./packages/actant-mastra/src/index.test.ts). |
| Captures context manifest | ✅ | `context_build` event + `buildContextManifest()` in [`packages/actant-core/src/runtime.ts`](./packages/actant-core/src/runtime.ts). |
| Supports approval and constraint | ✅ | Guard verdicts (`allow`, `constrain`, `require_approval`, `block`, `halt`) in [`packages/actant-policy/src/index.ts`](./packages/actant-policy/src/index.ts); constrain rewrite covered by [`scripts/smoke.mjs`](./scripts/smoke.mjs). |
| Opens Studio with timeline + replay | ✅ | `actantdb studio` in [`packages/actant-studio/src/cli.ts`](./packages/actant-studio/src/cli.ts); `/api/replay` renders side-by-side diffs. |
| Runnable examples exist | ✅ | [`examples/test-cleanup/`](./examples/test-cleanup), [`examples/langgraph-router/`](./examples/langgraph-router), [`examples/cli-only/`](./examples/cli-only). |
| Workspace smoke covers the story | ✅ | `pnpm smoke` covers session → message → manifest → tool request → verdict → approval → constrained execution → completion → checkpoint → headless Studio render. |

**Gate 1: green.**

## Gate 2 — Self-host backend

Acceptance criterion:

> ActantDB can run locally or as a server, persist a hash-chained ledger, expose
> agent-native APIs, and give operators enough tools to diagnose and recover.

| Item | Status | Evidence |
| --- | --- | --- |
| Embedded local ledger | ✅ | `@actantdb/core` uses SQLite via `node:sqlite`; Node >= 22.5 path documented in [`README.md`](./README.md). |
| Rust HTTP + WS server | ✅ | [`crates/actant-server/`](./crates/actant-server) with health probes, TLS support, and OpenAPI coverage. |
| SQLite + Postgres storage | ✅ | [`crates/actant-storage/`](./crates/actant-storage), SQLite/PG migration parity in CI. |
| Hash-chain and idempotency | ✅ | `agent_event.prev_chain_hash` and idempotency records covered by spec verification. |
| Auth + tenant boundary | ✅ | [`crates/actant-auth/`](./crates/actant-auth), [`crates/actant-tenant/`](./crates/actant-tenant). |
| Effect workers | ✅ | Shell, file, model, MCP, browser, email, slack, and manager workers under `crates/actant-workers/`. |
| Durable workflows | ✅ | [`crates/actant-flow/`](./crates/actant-flow), [`packages/actant-workflow/`](./packages/actant-workflow). |
| Replay modes | ✅ | Rust and TS replay packages cover recorded/model/policy/memory/tool/local_only/experimental modes. |
| CLI diagnostics | ✅ | `actantdb doctor`, `status`, `tail`, `watch`, `shell`, `explain`, `sql`, `export`, `import`, `backup`, `restore`. |
| Deployment recipes | ✅ | [`deploy/docker-compose.yml`](./deploy/docker-compose.yml), [`deploy/Dockerfile`](./deploy/Dockerfile), [`deploy/helm/`](./deploy/helm). |
| MCP surface | ✅ | [`packages/actant-mcp-server/`](./packages/actant-mcp-server) exposes tools and resources over stdio + HTTP. |

**Gate 2: green.**

## Gate 3 — Compatibility and release discipline

Acceptance criterion:

> Public types, generated SDKs, migrations, docs, and CI stay reproducible from
> the repository.

| Item | Status | Evidence |
| --- | --- | --- |
| Contracts are source of truth | ✅ | Public cross-language types live in [`crates/actant-contracts/`](./crates/actant-contracts). |
| Generated TypeScript is reproducible | ✅ | `cargo run -p actant-contracts -- codegen-ts` writes [`packages/actant-types/src/generated/`](./packages/actant-types/src/generated). |
| Schema compatibility is checked | ✅ | `cargo run -p actant-contracts -- check-compat` compares current schemas against the generated baseline and fails on removed types, removed properties, required-field drift, and enum shrinkage. |
| Spec verification is wired | ✅ | Every active spec has a `tests/spec_NN_verification.rs` verifier; see [`SPECS_STATUS.md`](./SPECS_STATUS.md). |
| Agent docs verification is wired | ✅ | `just verify-agents` enforces the required agent-work-package sections. |
| Formatting/lint/test CI is wired | ✅ | [`.github/workflows/ci.yml`](./.github/workflows/ci.yml) runs format, lint, tests, spec verification, and agent verification. |
| Publish workflow exists | ✅ | [`.github/workflows/publish-npm.yml`](./.github/workflows/publish-npm.yml) builds, tests, smokes, dry-runs, publishes, and mirrors tags when manually triggered. |
| Binary release workflow exists | ✅ | [`.github/workflows/release-binaries.yml`](./.github/workflows/release-binaries.yml) builds release binaries from tags or manual dispatch. |

**Gate 3: green.**

## Summary

The repo-verifiable quality gates are green. Market-facing outcomes and
publication timing are operations, not quality gates.
