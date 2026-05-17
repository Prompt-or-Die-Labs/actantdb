# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository status — read before doing anything

**A pivot landed on 2026-05-17.** Most of this repo is now frozen. Read [PIVOT.md](PIVOT.md) and [wedge/README.md](wedge/README.md) before opening any other directory.

- **Active surface (v0.1 wedge):** `/wedge/`, `/packages/*`, and a small set of Rust crates: `crates/actant-contracts`, `crates/actant-kernel`, `crates/actant-napi`, `crates/actant-wasm`, `crates/actant-server`.
- **Frozen as v2 roadmap (do NOT pick up work):** `/specs/`, `/planning/`, `/agents/`, `/migrations/`, `/templates/`, `/examples/`, and all other crates under `/crates/`. Each has a `STATUS.md` marker explaining the freeze. The frozen crates compile to empty `lib.rs` stubs — they only reserve names. Filling them in requires the wedge to pass Gate 3 (Aug 17, 2026).
- The v0.1 product is two features: **Guard Authority** (runtime tool-call gate) and **Chronicle Replay** (causal event timeline + replay from any decision point). Anything outside those two features waits.

## Architecture

The product is shipped as npm packages; Rust is an invisible implementation detail.

```
packages/                      pnpm workspace, ESM, TS 5.4+ strict
  actant-mastra/   @actantdb/mastra  — wedge entry point (Mastra wrapper)
  actant-core/     @actantdb/core    — embedded TS runtime; bundles native + WASM
  actant-types/    @actantdb/types   — GENERATED from crates/actant-contracts
  actant-policy/   @actantdb/policy  — Guard verdict builders
  actant-replay/   @actantdb/replay  — replay engine
  actant-studio/   @actantdb/studio  — UI + `actantdb` CLI
  actant-convex/   @actantdb/convex  — Phase 2, conditional

crates/                        Cargo workspace, Rust 1.88, edition 2021
  actant-contracts/  SINGLE SOURCE OF TRUTH for every public type. Has a CLI
                     (`cargo run -p actant-contracts -- ...`) with subcommands
                     `diff`, `check-compat`, `codegen-ts`, `codegen-py`,
                     `codegen-swift`. TS types regenerate into
                     packages/actant-types/src/generated/*.
  actant-kernel/     Hot-path coordinator (Rust impl).
  actant-napi/       NAPI-RS native addon, bundled into @actantdb/core via
                     optionalDependencies (per platform).
  actant-wasm/       WASM fallback, bundled into @actantdb/core.
  actant-server/     Optional scale-out mode (later, not Phase 1).
  (everything else)  FROZEN v2 substrate — empty stubs, do not modify.
```

The TS API is identical between `mode: "embedded"` (default) and `mode: "server"` — choice is config, not migration. Default install is `npm install @actantdb/core`; no Rust toolchain, no Docker, no exposed port, no daemon.

### Binding rules (from [wedge/f2-f3-prevention.md](wedge/f2-f3-prevention.md))

These supersede anything else in the repo that contradicts them:

1. **No new public type outside `actant-contracts`.** If an interface is missing, stop and request a contract change — do not invent it elsewhere. Coding-agent work packages explicitly forbid inventing interfaces.
2. **Contract update protocol:** edit `actant-contracts` → `cargo run -p actant-contracts -- check-compat` → `... codegen-ts` → commit Rust + regenerated TS in the same PR. Hand-edits to `packages/actant-types/src/generated/*` are forbidden.
3. **TS-native default.** The first README line is `npm install`. Never add Rust toolchain steps, ports, or Docker to the default install path.
4. **Workspace smoke test must pass on every PR.** If `cargo build --workspace` or the smoke test stays red >24h, freeze new work until green.
5. **Three crates → five crates is the active growth path.** The 40-crate substrate stays frozen until the wedge proves out. By the time 5 active crates compile, if the daily build/smoke isn't green, collapse to a monolith — no discussion.

## Common commands

Rust side (via [justfile](justfile)):

```bash
just            # list recipes
just check      # cargo check --workspace --all-targets   (fast)
just build      # cargo build --workspace --all-targets
just test       # cargo test  --workspace --all-targets
just fmt        # cargo fmt --all
just fmt-check  # CI mode
just lint       # cargo clippy --workspace --all-targets -- -D warnings
just ci         # fmt-check + lint + test + verify-specs + verify-agents
```

Run a single Rust test: `cargo test -p <crate> <test_name>` (e.g. `cargo test -p actant-contracts check_compat`).

TypeScript side (pnpm 9, Node ≥22.5 — `node:sqlite` is required by `@actantdb/core`):

```bash
pnpm install
pnpm -r build         # build all packages
pnpm -r test          # run all package tests (vitest)
pnpm -r --parallel dev
pnpm -r lint          # tsc --noEmit per package
pnpm smoke            # workspace E2E smoke test (must pass on every PR)
pnpm ci               # build + test + smoke

# Workspace also contains:
#   wedge/demo/        the killer-demo rehearsal package (`pnpm --filter
#                      actant-demo-test-cleanup demo` to record a run;
#                      `pnpm --filter actant-demo-test-cleanup studio` to open it)
```

Run a single TS test: `pnpm --filter @actantdb/<pkg> test -- <pattern>` (vitest).

Regenerate TS types from contracts:

```bash
cargo run -p actant-contracts -- check-compat
cargo run -p actant-contracts -- codegen-ts
```

Repo-wide checks enforced by [.github/workflows/ci.yml](.github/workflows/ci.yml):

- `just verify-specs` — every `specs/*.md` must contain a `## Verification` section.
- `just verify-agents` — every `agents/actant-*.md` must contain `## Context`, `## Scope`, `## Specs to read first`, `## Acceptance criteria`, `## Do NOT`.

Both files are frozen as v2 roadmap, but the lints still run in CI — don't break them.

## Conventions

- SQL: `snake_case`, `TEXT` for IDs, ISO-8601 timestamps (SQLite alpha).
- Naming: crates are `actant_*`, types are `Actant*`, the product written as one word is lowercase `actantdb`.
- TypeScript packages: ESM only, no CJS. All cross-package types come from `@actantdb/types`.
- Markdown: 100-col soft wrap, ATX headers, fenced blocks with language hints.

## Things to actively avoid

- Adding work to any frozen crate or directory (see `STATUS.md` files).
- Introducing public types in any package/crate other than `actant-contracts`.
- Adding planning docs — anti-scope rule #1 in [PIVOT.md](PIVOT.md) forbids new planning files post-pivot.
- Adding empty stubs without a use site in the killer demo (anti-scope rule #2).
- Writing prose-only contracts: contracts are Rust types + JSON schemas, not markdown.
