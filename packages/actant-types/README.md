# @actantdb/types

Generated TypeScript bindings of [`crates/actant-contracts`](../../crates/actant-contracts) — the **single source of truth** for every public type, error, event, command, and schema in ActantDB.

**Hand-edits are forbidden.** Every file under `src/generated/` is regenerated from `actant-contracts` by `cargo run -p actant-contracts -- codegen-ts`.

## Why this exists

See [`/CLAUDE.md`](../../CLAUDE.md) §F3. Prose specs do not survive parallel coding-agent implementation across many crates: agents re-derive their own interpretations. The contract crate is the only place these types live; this package is the consumable TypeScript view.

## Update protocol

1. Modify `crates/actant-contracts` with the proposed change.
2. Run `cargo run -p actant-contracts -- check-compat` — fails if the change is backward-incompatible without an explicit version bump.
3. Run `cargo run -p actant-contracts -- codegen-ts` — regenerates `packages/actant-types/src/generated/*`.
4. Commit both the Rust + the regenerated TypeScript in the same PR.

No package may add a public type that crosses package boundaries without going through this protocol.
