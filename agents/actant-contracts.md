# Work package: `actant-contracts`

## Context

`actant-contracts` is the single source of truth for public contract types and generated SDK surfaces. Cross-crate/public shapes start here, and TypeScript/Python/Swift bindings are generated from this crate.

## Specs to read first

- `/specs/09-sdk-design.md`.
- `/specs/02-data-model.sql`.
- `/specs/03-command-spec.md`.

## Scope

Own the Rust contract definitions, compatibility checks, generated TypeScript bindings, and the `actant-sdk-codegen` binary/templates.

## Acceptance criteria

- [ ] `cargo build -p actant-contracts` zero warnings.
- [ ] `cargo test -p actant-contracts` passes.
- [ ] `cargo run -p actant-contracts --bin actant-contracts -- check-compat` passes.
- [ ] `cargo run -p actant-contracts --bin actant-contracts -- codegen-ts` produces no unstaged generated drift.

## Do NOT

- Do NOT add public API types in downstream crates first.
- Do NOT hand-edit `packages/actant-types/src/generated/*`.
- Do NOT emit SDK clients from live server metadata when the contract crate can be the source of truth.
