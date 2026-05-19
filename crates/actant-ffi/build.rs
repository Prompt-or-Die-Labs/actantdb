//! Build script for `actant-ffi`.
//!
//! Proc-macro uniffi (the `#[uniffi::export]` flavour we use here) does
//! **not** require a UDL file or codegen step at compile time — the
//! `uniffi::setup_scaffolding!()` macro in `src/lib.rs` emits everything
//! the FFI ABI needs inline.
//!
//! What `build.rs` actually does:
//! 1. Trigger a rebuild whenever the FFI surface in `src/lib.rs` changes,
//!    so any consumer that links the cdylib picks up signature changes.
//! 2. Document — by being present — that the bindings-generation step is
//!    a *post-build* invocation of the `uniffi-bindgen` bin (see
//!    `src/bin/uniffi_bindgen.rs` and the README). Doing it from `build.rs`
//!    would be circular: `uniffi-bindgen --library` needs the compiled
//!    cdylib as input.
fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/bin/uniffi_bindgen.rs");
}
