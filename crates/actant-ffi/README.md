# actant-ffi

Embeddable ActantDB surface exposed via [uniffi-rs](https://mozilla.github.io/uniffi-rs/).

Closes [`GAPS.md`](../../GAPS.md) row **#39**. This is the Rust side of the
iOS embedded-mode plan documented in
[`docs/IOS_EMBEDDING.md`](../../docs/IOS_EMBEDDING.md) §1.

## What ships

| Symbol           | Kind              | Role                                                       |
|------------------|-------------------|------------------------------------------------------------|
| `ActantHandle`   | `uniffi::Object`  | Opaque handle: `open`, `dispatch`, `events_since`, `ingest`, `close`. |
| `CommandOutcome` | `uniffi::Record`  | Flat mirror of `actant_command::CommandOutcome`.            |
| `EventRow`       | `uniffi::Record`  | Flat mirror of `agent_event` with replication fields.        |
| `IngestReport`   | `uniffi::Record`  | accepted / skipped / rejected counts.                       |
| `FfiError`       | `uniffi::Error`   | FFI-safe flattening of `actant_core::ActantError`.          |

The `[lib]` table builds `cdylib`, `staticlib`, and `rlib` — the first two
feed the iOS XCFramework workflow (GAPS row #41); the `rlib` lets
`cargo test -p actant-ffi` run in-tree.

## Building

```sh
# Standard host build (gives you target/release/libactant_ffi.{dylib,a})
cargo build --release -p actant-ffi
```

## Generating Swift bindings

The crate publishes a `uniffi-bindgen` bin that wraps `uniffi::uniffi_bindgen_main()`.
Run it **after** the cdylib is built; uniffi reads the symbol table out of
the compiled library, so the codegen step is not allowed to happen inside
`build.rs` (would be circular).

```sh
# Build the cdylib first.
cargo build --release -p actant-ffi

# Then generate Swift glue against it.
cargo run --bin uniffi-bindgen -p actant-ffi -- \
    generate \
    --library target/release/libactant_ffi.dylib \
    --language swift \
    --out-dir crates/actant-ffi/bindings/swift
```

That writes `actant_ffi.swift` + `actant_ffiFFI.h` + `actant_ffiFFI.modulemap`
into `bindings/swift/`. The SwiftPM package keeps the generated Swift glue at
`sdks/swift/Sources/ActantFFI/actant_ffi.swift`; the XCFramework workflow
diffs its freshly generated copy against that committed source before packaging
the headers and `lipo`-fattened static archive into `ActantFFI.xcframework.zip`.

For local Swift SDK validation before a tagged release artifact exists:

```sh
bash sdks/swift/scripts/build-local-actantffi-xcframework.sh
ACTANTDB_LOCAL_FFI_XCFRAMEWORK=".actantffi/ActantFFI.xcframework" \
  swift test --package-path sdks/swift --filter embeddedRoundTrip
```

The release workflow writes the SwiftPM-compatible checksum with:

```sh
swift package compute-checksum ActantFFI.xcframework.zip > ActantFFI.checksum
```

## Open dependencies (cross-agent)

- **GAPS row #42** — HLC clock + `device_id` columns in `agent_event`.
  Until that migration lands, `events_since` returns `"_legacy_"` / `0` /
  `0` for the replication fields. The SQL projection in `src/lib.rs` is
  the single place that changes.
- **GAPS row #43** — `Storage::ingest_events()` is wired. `ingest` accepts
  content-derived ids and reports idempotent skips for duplicates.

## Local verification

```sh
cargo check    -p actant-ffi --all-targets
cargo build    -p actant-ffi --release
cargo clippy   -p actant-ffi --all-targets -- -D warnings
cargo test     -p actant-ffi
cargo fmt --all -- --check
```

(`cargo test --workspace` is banned per the CLAUDE.md disk-crash note.)
