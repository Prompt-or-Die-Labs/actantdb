# PIVOT — current substrate and validation gates

This repo is past the freeze-lift point: the core ActantDB substrate is the
active product, not a future wedge. The durable shape is an embedded-first
event ledger for agents, with a Rust server path when teams need multi-user
approval, OIDC, hosted deployment, or clustered sync.

## Current substrate

- `npm install @actantdb/all` is the default consumer install path.
- The workspace currently contains 19 package manifests under `packages/`:
  18 `@actantdb/*` packages plus `create-actantdb`.
- The local workspace version is `0.0.15`; the latest npm version verified
  for `@actantdb/all` and `@actantdb/mastra` is `0.0.12`.
- Rust lives in 37 crates under `crates/`, with 39 Cargo workspace members
  including the Rust SDK and bench package.
- `crates/actant-contracts/` remains the single source of truth for public
  cross-language types. Generated TypeScript belongs under
  `packages/actant-types/src/generated/` and is regenerated, not hand-edited.
- Server mode is opt-in. Embedded mode remains the first-run path for Node
  users through `node:sqlite`.

## Hard validation gates

### Gate 1 — MVP green

Target: 2026-06-30.

Acceptance criterion:

> `@actantdb/mastra` wraps a Mastra-shaped agent, captures tool calls and a
> context manifest, supports approval, and opens Studio with timeline and
> replay.

Repo status: implementation-complete. The remaining Gate 1 items are human
artifacts: a 90-second recording, a hero PNG, and three-platform external
install verification.

### Gate 2 — external adoption

Target: 2026-07-31.

Acceptance criterion:

> 10 non-Wes developers installed ActantDB; 5 used it on real projects; 3 kept
> it past one week; 2 became weekly-feedback design partners.

Repo status: artifact prerequisites mostly exist, but the current `0.0.15`
workspace packages are not yet published to npm. Closing the gate requires
publishing and human outreach.

### Gate 3 — shipped or staged

Target: 2026-08-17.

Acceptance criterion:

> 5 non-Wes developers shipped or staged with ActantDB; 2 public examples
> exist; 1 named design partner is publicly attributable.

Repo status: three public examples exist in `examples/`. The developer usage
and named partner thresholds are external events, not repo artifacts.

## Non-negotiables

- Do not add public types outside `crates/actant-contracts/`.
- Do not hand-edit generated TypeScript bindings.
- Do not make the consumer install path require Rust, Docker, or an exposed
  port.
- Do not weaken the hash chain. `prev_chain_hash` is load-bearing.
