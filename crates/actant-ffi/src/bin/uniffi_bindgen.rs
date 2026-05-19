//! Thin host for `uniffi-bindgen` so the Swift / Kotlin glue is generated
//! by the same uniffi minor the runtime crate was compiled against.
//!
//! Usage (from the repo root, once `cargo build --release -p actant-ffi`
//! has produced the cdylib):
//!
//! ```sh
//! cargo run --bin uniffi-bindgen -- \
//!     generate \
//!     --library target/release/libactant_ffi.dylib \
//!     --language swift \
//!     --out-dir crates/actant-ffi/bindings/swift
//! ```
//!
//! See `crates/actant-ffi/README.md` for the full Swift / iOS recipe.
fn main() {
    uniffi::uniffi_bindgen_main()
}
