# ActantDB — rules for AI coding assistants

This file teaches Cursor, Windsurf, Claude Code, GitHub Copilot, and Google Antigravity about the shape of this repository. Keep the following files in sync when making changes to rules:
- `.cursorrules` (Cursor)
- `.windsurfrules` (Windsurf)
- `.github/copilot-instructions.md` (GitHub Copilot)
- `AGENTS.md` (Cross-tool foundation / Claude Code)
- `GEMINI.md` (Google Antigravity-specific rules)
- `.antigravityrules` (Antigravity boundaries)

## What ActantDB is

ActantDB is a hash-chained event ledger for AI agents. Every action an
agent takes — a model call, a tool call, a memory write, a guard
verdict — lands as a typed, append-only event in SQLite (default) or
Postgres. Consumers wrap their existing agent framework (Mastra,
LangGraph, raw SDK calls) with `withActant()` from `@actantdb/mastra`
and get a complete, replayable trace for free.

## Workspace shape

- **TypeScript packages** live in `packages/`. Each package publishes
  as `@actantdb/<name>` and uses ESM only. Node ≥ 22.5 (we use
  `node:sqlite`).
- **Rust crates** live in `crates/`. The HTTP+WS server binary is
  `actant-server` (built as `actantdb-server`); the CLI binary is
  `actant-cli` (built as `actantdb`).
- **The single source of truth for every public type** is
  `crates/actant-contracts/`. Generated TypeScript bindings land in
  `packages/actant-types/src/generated/`.
- **Specs** live in `specs/`. Every active spec has a `## Verification`
  section enforced by `tests/spec_NN_verification.rs` in the relevant
  crate.
- **Agent Guidelines** live in `agents/`. Every agent markdown file must contain all required layout sections.

## Binding rules — do not break these

1. **No new public type outside `actant-contracts`.** If you need to
   add a struct that crosses a crate boundary or appears in the public
   API, edit `crates/actant-contracts/src/lib.rs` first.
2. **Never hand-edit `packages/actant-types/src/generated/*`.** Those
   files are produced by `cargo run -p actant-contracts -- codegen-ts`.
   If the generated TypeScript is wrong, fix the Rust contract and
   regenerate.
3. **The default install path is `npm install @actantdb/all`.** Do
   not add Rust toolchain steps, Docker, or exposed ports to
   consumer-facing READMEs. Server mode is opt-in; embedded mode runs
   in Node out of the box.
4. **Every agent action is a typed event in a hash-chained ledger.**
   When you're unsure about an event payload, look at
   `@actantdb/types` first, then `crates/actant-contracts/src/lib.rs`.
   The `prev_chain_hash` field on every row is load-bearing — don't
   skip it.

## Surface vocabulary

- The product written as one word is lowercase **`actantdb`**.
- The CLI binary is **`actantdb`**.
- The umbrella npm package is **`@actantdb/all`**.
- Crates are named `actant-*` (kebab-case), Rust types are `Actant*`.
- SQL identifiers use `snake_case`, IDs are `TEXT`, timestamps are
  ISO-8601 strings.

## Graphify Rules
- For codebase questions, first run `graphify query "<question>"` when `graphify-out/graph.json` exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts.
- If `graphify-out/wiki/index.md` exists, use it for broad navigation instead of raw source browsing.
- Read `graphify-out/GRAPH_REPORT.md` only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).

## Assistant tooling

- Codex hooks live in `.codex/hooks.json` and delegate to `.codex/hooks/actantdb-context.sh`.
- Project skills live in `.agent/skills/`; workflows live in `.agent/workflows/`.
- Git hooks live in `.githooks/` and should be active through `git config core.hooksPath .githooks`.

## Common commands

- **Rust check (fast):** `just check` or `cargo check -p <crate> --all-targets`
- **Rust tests (per crate):** `cargo test -p <crate> <test_name>`
- **TypeScript build:** `pnpm -r build`
- **TypeScript tests:** `pnpm -r test` or `pnpm --filter @actantdb/<pkg> test`
- **Regenerate TS types from contracts:** `cargo run -p actant-contracts -- codegen-ts`
- **Smoke test (required green on every PR):** `pnpm smoke`
- **Verify Specs compliance:** `just verify-specs`
- **Verify Agent compliance:** `just verify-agents`
- **Full CI check:** `just ci`

## Do NOT

- Do **not** run `cargo test --workspace` locally — the build artefacts
  can crash low-disk machines. Use `cargo test -p <crate>` instead.
- Do **not** add Rust toolchain steps to the default install
  instructions in any README a consumer reads.
- Do **not** introduce a new public type without first editing
  `actant-contracts` and regenerating bindings in the same PR.
- Do **not** hand-edit anything under `packages/actant-types/src/generated/`.
- Do **not** treat the wedge framing as v1 and the substrate as
  future — both are present and active. See `PIVOT.md`.

## Where to look

- Repo orientation: `CLAUDE.md`, `PIVOT.md`, `README.md`.
- What landed when: `CHANGELOG.md`.
- Outstanding work: `GAPS.md`, `DEVX_GAPS.md`, `GATES.md`.
- Per-spec verification status: `SPECS_STATUS.md`.
- Architecture: `specs/00-overview.md` and friends.
