# actant-contracts

The contract crate. Every cross-package type lives here exactly once.

## Why this exists

`/wedge/f2-f3-prevention.md` §F3 names the failure mode: prose specs do not survive parallel coding-agent implementation across many crates. Agents re-derive their own interpretations of `AccountableAction`, `GovernanceContext`, the audit schema, the error hierarchy, the event taxonomy. Each derivation compiles locally; the workspace-level build fails in hundreds of places three months later.

The fix: machine-checked contracts. Every public type, error, event name, command name, schema, wire shape is defined here once. `cargo check` is the source of truth.

## Rules

1. If a type crosses crate or package boundaries, it lives here.
2. If an error crosses crate or package boundaries, it lives here.
3. If a command can be called by an SDK, its input + output + error types live here.
4. If an event can be replayed, its payload shape lives here.
5. No other crate redefines these types under a different name.
6. Hand-edits to `packages/actant-types/src/generated/*` are forbidden — they are regenerated.

## Update protocol

1. Modify `src/` in this crate.
2. Run `cargo run -p actant-contracts -- check-compat`. Backward-incompatible changes without an explicit version bump fail.
3. Run `cargo run -p actant-contracts -- codegen-ts`. Regenerates `packages/actant-types/src/generated/*`.
4. Commit Rust + regenerated TS in the same PR.
5. Reviewer prompt: "Does this PR add a public type outside this crate? If yes, reject."

## What this prevents

Each of these symptoms is invalidated by structure rather than vigilance:

- Same concept under five names across crates.
- Error types forked four ways.
- Trait signatures with incompatible shapes.
- Schema fields that appear in one crate's writer and not the matching reader.
- Workspace builds that succeed locally per-crate and fail catastrophically on integration.

See [`/wedge/f2-f3-prevention.md`](../../wedge/f2-f3-prevention.md).
