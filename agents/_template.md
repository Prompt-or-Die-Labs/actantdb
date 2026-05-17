# Work package: `<crate-name>`

## Context

(One paragraph: where this crate sits in the ActantDB architecture, what depends on it, what it depends on. Reference `/specs/01-architecture.md`.)

## Specs to read first

(Bullet list of `/specs/*` files the agent MUST read before writing code. Order matters.)

- `/specs/01-architecture.md` §"..."
- `/specs/02-data-model.sql` (tables: `...`)
- `/specs/03-command-spec.md` §"..."

## Scope

(What this work package WILL produce. Be exhaustive — types, traits, functions, tests, examples. If something is out of scope for this phase, say so.)

### Public API surface

```rust
// pseudo-Rust signatures the agent must produce
```

### Internal modules

```
crates/<crate>/src/
├── lib.rs
├── ...
```

### Tests

- Unit tests for every public function.
- Integration tests under `tests/`.

## Acceptance criteria

(Binary. Each item is testable.)

- [ ] `cargo build -p <crate>` succeeds with zero warnings.
- [ ] `cargo test -p <crate>` passes.
- [ ] `cargo clippy -p <crate> -- -D warnings` passes.
- [ ] (Add crate-specific criteria, ideally tied to spec invariants.)

## Do NOT

(Constraints — what the agent must avoid.)

- Do NOT modify any other crate's source.
- Do NOT add dependencies not present in `Cargo.toml` workspace table.
- Do NOT add `unsafe` code.
- Do NOT contradict any spec. Open an issue if the spec is wrong.

## Hand-off

When done, run `just ci` from the workspace root and ensure all green.
