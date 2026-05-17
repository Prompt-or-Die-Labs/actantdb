# Work package: `sdks/rust` — `actant-client`

## Context

Rust SDK for *consumers* of ActantDB. Lives outside the workspace (separately published) so consumers don't need the whole repo.

## Specs to read first

- `/specs/09-sdk-design.md` §11.
- `/planning/sdk-rust.md`.

## Scope

### Layout

```
sdks/rust/
├── Cargo.toml                  (not a workspace member)
├── README.md
├── src/
│   ├── lib.rs
│   ├── client.rs
│   ├── transport/
│   │   ├── http.rs
│   │   └── ws.rs
│   ├── subscribe.rs
│   ├── errors.rs
│   ├── auth.rs
│   └── generated.rs            (codegen output)
└── tests/
```

### Tests

- `cargo test` integration tests against `actantdb-server` in CI.
- Stream cancellation: `drop` on the subscription handle sends `unsubscribe`.
- `cargo clippy -- -D warnings` clean.

## Acceptance criteria

- [ ] `cargo build --release` green.
- [ ] `cargo test` green.
- [ ] `cargo doc --no-deps` complete public docs.
- [ ] Crate publishes to crates.io with a single `actant-client` name.

## Do NOT

- Do NOT depend on `actant-storage` or any non-`actant-core` workspace crate.
- Do NOT have a feature flag for `async-std`. tokio only in Phase 1.
- Do NOT add unsafe.

## Hand-off

`cargo test`, plus running the alpha demo via this SDK from a separate cargo project.
