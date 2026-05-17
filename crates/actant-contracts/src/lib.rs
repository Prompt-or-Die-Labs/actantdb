//! actant-contracts — the single source of truth for ActantDB public types.
//!
//! Every cross-package type lives here. Other ActantDB crates and SDKs
//! consume their types from here; hand-edits to generated outputs are
//! forbidden. See `/wedge/f2-f3-prevention.md`.
//!
//! v0.1 scope: only the types the killer demo emits (Guard Authority +
//! Chronicle Replay). Per anti-scope rule #2, nothing here exists without
//! a use site in the demo.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod events;
pub mod policy;
pub mod replay;
pub mod schema;

pub use events::*;
pub use policy::*;
pub use replay::*;
