# CLAUDE.md

This file guides Claude Code (claude.ai/code) when working in this repository.

## Repository status — read before doing anything

The 2026-05-17 premortem proposed freezing everything except a 2-feature wedge. The freeze was **lifted the same day**. The substrate was built out: ~49 Rust crates, 8 npm packages (now published as `@actantdb/*@0.0.6`), three runnable demos, **429 passing tests** (331 Rust + 25 TS + 10 Python + 62 Swift + 1 smoke), full Phase 1–6 implementation. The wedge frame is still useful for *external* developer narrative; internally, everything is active.

Read [PIVOT.md](PIVOT.md) for the current state, [CHANGELOG.md](CHANGELOG.md) for what landed, [GATES.md](GATES.md) for outstanding gate work, and [SPECS_STATUS.md](SPECS_STATUS.md) for per-spec verification.

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
actant-napi, actant-wasm  (declared via @actantdb/core optionalDependencies; bridge build deferred)
```

The TypeScript API is identical between `mode: "embedded"` (`@actantdb/core` against `node:sqlite`) and `mode: "server"` (`@actantdb/sdk` against `actantdb-server`). Choice is config, not migration.

## Binding rules (kept from F2/F3 prevention — `/wedge/f2-f3-prevention.md`)

These survived the freeze lift because they're the right design regardless:

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
ACTANTDB_STORE_DIR=./wedge/demo/.actantdb \
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
- Treating the wedge as v1 and the substrate as future — both are present; both have tests; both are active.
- Treating the original STATUS.md "freeze v2 roadmap" framing as authoritative — it's been lifted (see PIVOT.md).
