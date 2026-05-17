# Work package: `actant-sdk-codegen`

## Context

`actant-sdk-codegen` reads `GET /v1/metadata/commands` and `GET /v1/metadata/tables` from a running server and emits typed clients per language. Phase 1 emits TypeScript, Python, and Rust. The SDK packages themselves (`sdks/ts`, `sdks/python`, `sdks/rust`) are scaffolded in separate work packages once codegen produces output.

## Specs to read first

- `/specs/09-sdk-design.md` §7 — codegen pipeline.
- `/specs/08-api-spec.md` §8 — metadata endpoints.

## Scope (Phase 1)

### CLI

```
actant-sdk-codegen --target ts    --server URL  --out PATH
actant-sdk-codegen --target py    --server URL  --out PATH
actant-sdk-codegen --target rust  --server URL  --out PATH
actant-sdk-codegen --target ts    --metadata-file FILE --out PATH    # offline mode
```

### Internal modules

```
crates/actant-sdk-codegen/src/
├── main.rs
├── lib.rs                 # public API for tests
├── metadata.rs            # fetch + parse server metadata
├── targets/
│   ├── mod.rs
│   ├── ts.rs              # TypeScript emitter
│   ├── py.rs              # Pydantic emitter
│   └── rust.rs            # serde emitter
└── format.rs              # post-process: prettier / black / rustfmt invocation
```

### Tests

- A canned metadata fixture produces deterministic output for each target.
- TypeScript output type-checks under `tsc --strict` (via a small harness in `tests/`).
- Python output passes `mypy --strict` against the generated file.
- Rust output compiles standalone.

## Acceptance criteria

- [ ] `cargo build -p actant-sdk-codegen` zero warnings.
- [ ] `cargo test -p actant-sdk-codegen` passes (including the type-check harnesses).
- [ ] `cargo clippy -p actant-sdk-codegen -- -D warnings` passes.
- [ ] Output for every alpha command in `/specs/03-command-spec.md` includes a method with input type, output type, and the documented errors.

## Do NOT

- Do NOT hand-edit generated files in any SDK. Codegen overwrites.
- Do NOT introduce template-engine libraries beyond `handlebars` or equivalent. Phase 1 keeps emitters explicit.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
