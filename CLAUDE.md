# CLAUDE.md

This file guides Claude Code (claude.ai/code) when working in this repository.

## Repository status — read before doing anything

Workspace: **36 Rust crates** (post-slim refactor, was 53), **9 npm packages** (incl. new `actantdb` umbrella), three runnable end-to-end demos under `examples/`, full Phase 1–6 implementation. Packages published as `@actantdb/*@0.0.10`. Everything is active; there is no "wedge mode" or freeze.

Read [CHANGELOG.md](CHANGELOG.md) for what landed, [GATES.md](GATES.md) for outstanding gate work, [GAPS.md](GAPS.md) for implementation gaps, [SPECS_STATUS.md](SPECS_STATUS.md) for per-spec verification, [STORAGE_AUDIT.md](STORAGE_AUDIT.md) + [BENCHMARKS.md](BENCHMARKS.md) + [TESTING.md](TESTING.md) for system audits.

## Architecture

The product ships as **npm packages**; the Rust workspace is the substrate.

### TypeScript packages (`/packages`, pnpm workspace, ESM, Node ≥22.5)

```
@actantdb/mastra    withActant() wrapper for Mastra / LangGraph / hand-rolled agents
@actantdb/core      embedded ledger on node:sqlite
@actantdb/types     generated from crates/actant-contracts (hand-edits forbidden)
@actantdb/policy    Guard verdict builders
@actantdb/replay    checkpoint / run / diff
@actantdb/studio    UI + actantdb CLI
@actantdb/sdk       HTTP + WS client for the Rust server
@actantdb/convex    Convex adapter
```

### Swift SDK (`/sdks/swift`, SwiftPM, Swift 6.3, macOS 26 / iOS 26)

Two-tier consumer surface — the consumer (Swoosh and friends) plugs
ActantDB in by declaring conformances on the high-level types, not by
writing adapters:

```
ActantDB     low-level HTTP+WS client; covers every actant-server endpoint
ActantAgent  opinionated facade — AgentBackend, Session<Message>,
             MemoryStore, Auditor<Record>, ApprovalCenter, ReplayClient,
             RelationshipStore, ActantDBSupervisor (spawn/lifecycle the
             actantdb Rust subprocess for local-first mode)
```

ActantAgent's API shape is the consumer contract; methods are chosen so a
consumer satisfies their own protocols via one-line extensions. When in
doubt about a new high-level method, ask whether the consumer can express
it as `extension ActantAgent.X: ConsumerProtocol {}` — if not, push more
work down into ActantAgent.

### Rust crates (`/crates`, Cargo workspace, Rust 1.88)

```
actant-contracts      SINGLE SOURCE OF TRUTH for every public type.
                      CLI: cargo run -p actant-contracts -- {diff,check-compat,codegen-ts,codegen-py,codegen-swift}
actant-core, actant-storage, actant-command, actant-policy
actant-effects, actant-worker-{protocol,shell,file,model,mcp,browser,email,slack,manager}
actant-context, actant-memory, actant-embed, actant-embedders, actant-index
actant-flow, actant-trigger, actant-replay, actant-eval
actant-server, actant-cli, actant-kernel, actant-subscribe
actant-auth, actant-tenant, actant-audit-export, actant-sync
actant-throttle, actant-circuit, actant-lock, actant-ingress, actant-compensation, actant-drift
actant-protocol, actant-prompts, actant-models, actant-cache, actant-trace
actant-schema-dsl, actant-sdk-codegen, actant-templates, actant-codegen-project
actant-capsule, actant-trust
```

The TypeScript API is identical between `mode: "embedded"` (`@actantdb/core` against `node:sqlite`) and `mode: "server"` (`@actantdb/sdk` against `actantdb-server`). Choice is config, not migration.

## Binding rules

The contract-first build discipline:

1. **No new public type outside `actant-contracts`.** Missing interface? Edit the contract crate and regenerate. Hand-edits to `packages/actant-types/src/generated/*` are forbidden.
2. **Contract update protocol**: edit `actant-contracts` → `cargo run -p actant-contracts -- check-compat` → `… codegen-ts` → commit Rust + regenerated TS in the same PR.
3. **TS-native default install path.** First README line is `npm install`. Never add Rust toolchain steps, Docker, or exposed ports to the default install path.
4. **Workspace smoke test must pass on every PR.** If `cargo build --workspace` or `pnpm smoke` stays red >24h, freeze new feature work until green.
5. **Every active spec has a `tests/spec_NN_verification.rs` regression gate.** Breaking a spec's `## Verification` clause fails CI.

## Common commands

### Rust (via [justfile](justfile))

```bash
just            # list recipes
just check      # cargo check --workspace --all-targets   (fast)
just build      # cargo build --workspace --all-targets
just test       # cargo test  --workspace --all-targets   (331 passing)
just fmt        # cargo fmt --all
just fmt-check  # CI mode
just lint       # cargo clippy --workspace --all-targets -- -D warnings
just ci         # fmt-check + lint + test + verify-specs + verify-agents
```

Single Rust test: `cargo test -p <crate> <test_name>`.

### TypeScript (pnpm 9, Node ≥22.5)

```bash
pnpm install
pnpm -r build         # build all packages
pnpm -r test          # 25 vitest tests across the @actantdb/* packages
pnpm -r --parallel dev
pnpm -r lint          # tsc --noEmit per package
pnpm smoke            # workspace E2E (must pass on every PR)
pnpm ci               # build + test + smoke
```

Single TS test: `pnpm --filter @actantdb/<pkg> test -- <pattern>`.

### Demos

```bash
pnpm --filter actant-demo-test-cleanup demo       # record a run
ACTANTDB_STORE_DIR=./examples/test-cleanup/.actantdb \
  npx actantdb studio --project demo-test-cleanup # open it
```

### Regenerate TS types from contracts

```bash
cargo run -p actant-contracts -- check-compat
cargo run -p actant-contracts -- codegen-ts
```

### Python SDK

```bash
(cd sdks/python && python3 -m unittest discover -s tests)
# Integration test runs only when ACTANTDB_TEST_URL is set.
```

## Repo-wide lints enforced by `.github/workflows/ci.yml`

- `just verify-specs` — every `specs/*.md` must contain a `## Verification` section.
- `just verify-agents` — every `agents/actant-*.md` must contain `## Context`, `## Scope`, `## Specs to read first`, `## Acceptance criteria`, `## Do NOT`.

## Conventions

- SQL: `snake_case`, `TEXT` for IDs, ISO-8601 timestamps (SQLite alpha; Postgres backend exists).
- Naming: crates are `actant_*`, types are `Actant*`, the product written as one word is lowercase `actantdb`.
- TypeScript: ESM only. All cross-package types come from `@actantdb/types`.
- Markdown: 100-col soft wrap, ATX headers, fenced blocks with language hints.

## Things to actively avoid

- Introducing public types outside `actant-contracts`.
- Hand-editing `packages/actant-types/src/generated/*`.
- Adding Rust toolchain steps, Docker, or exposed ports to the default install README.
- Reintroducing "wedge" framing or freeze-mode language. Every part of the substrate is active and supported.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
