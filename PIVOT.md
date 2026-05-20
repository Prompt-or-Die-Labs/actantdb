# PIVOT — current substrate and artifact gates

This repo is past the freeze-lift point: the core ActantDB substrate is the
active product, not a future wedge. The durable shape is an embedded-first
event ledger for agents, with a Rust server path when teams need multi-user
approval, OIDC, hosted deployment, or clustered sync.

## Current substrate

- `npm install @actantdb/all` is the default consumer install path.
- The workspace currently contains 19 package manifests under `packages/`:
  18 `@actantdb/*` packages plus `create-actantdb`.
- The local workspace version is `0.0.15`.
- Rust lives in 37 crates under `crates/`, with 39 Cargo workspace members
  including the Rust SDK and bench package.
- `crates/actant-contracts/` remains the single source of truth for public
  cross-language types. Generated TypeScript belongs under
  `packages/actant-types/src/generated/` and is regenerated, not hand-edited.
- Server mode is opt-in. Embedded mode remains the first-run path for Node
  users through `node:sqlite`.

## Hard validation gates

Only repository-verifiable gates count here. Market-facing outcomes are not
validation gates because this repo cannot prove them.

### Gate 1 — agent substrate

Acceptance criterion:

> `@actantdb/mastra` wraps a Mastra-shaped agent, captures tool calls and a
> context manifest, supports approval, and opens Studio with timeline and
> replay.

Repo status: green. Covered by package tests, `pnpm smoke`, and Studio replay
routes.

### Gate 2 — self-host backend

Acceptance criterion:

> ActantDB runs embedded or as a Rust server, persists a hash-chained ledger,
> exposes agent-native APIs, and gives operators enough CLI/deployment tooling
> to diagnose, recover, and self-host.

Repo status: green. SQLite, Postgres, auth, tenants, workers, workflow,
replay, backup/restore, MCP, Docker Compose, Helm, and CLI diagnostics are in
tree and covered by tests or CI.

### Gate 3 — compatibility and release discipline

Acceptance criterion:

> Public contracts, generated SDKs, migrations, docs, and CI stay reproducible
> from the repository.

Repo status: green. `actant-contracts check-compat` compares current schemas
against the generated baseline, `codegen-ts` regenerates TS types, every active
spec has a verifier, agent docs have a verifier, and CI wires those checks.

## Non-negotiables

- Do not add public types outside `crates/actant-contracts/`.
- Do not hand-edit generated TypeScript bindings.
- Do not make the consumer install path require Rust, Docker, or an exposed
  port.
- Do not weaken the hash chain. `prev_chain_hash` is load-bearing.
