//! Subcommand handlers.
//!
//! Each module owns one (or a small cluster of) `actantdb` subcommand. The
//! enum variants and arg parsing live in [`crate::main`]; the bodies live
//! here. Keeping them in submodules stops `main.rs` from ballooning to
//! 1500+ lines as new commands land.

pub mod doctor;
pub mod explain;
pub mod export_import;
pub mod init;
pub mod shell;
pub mod sql;
pub mod status;
pub mod tail;
pub mod upgrade;
pub mod watch;
pub mod watch_dev;
