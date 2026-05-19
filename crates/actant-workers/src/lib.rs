//! Bundled effect workers for ActantDB.
//!
//! Each worker is gated behind its own feature flag so consumers (and
//! the per-worker `cargo install`-able binaries) pay only for what they
//! enable. The `Worker` trait + message types live in `actant-worker-protocol`
//! and are intentionally kept out of this crate.
//!
//! ## Feature matrix
//!
//! | Feature   | Module                  | Effect types covered                            |
//! |-----------|-------------------------|-------------------------------------------------|
//! | `shell`   | [`shell`]               | `shell.run`                                     |
//! | `file`    | [`file`]                | `file.read`, `file.write`                       |
//! | `browser` | [`browser`]             | `browser.navigate`/`click`/`type`/`screenshot`  |
//! | `cdp`     | [`browser::cdp`]        | (extends `browser` with a real Chrome driver)   |
//! | `email`   | [`email`]               | `email.send`                                    |
//! | `mcp`     | [`mcp`]                 | `mcp.call`                                      |
//! | `slack`   | [`slack`]               | `slack.post`                                    |
//! | `model`   | [`model`]               | `model.call`                                    |
//! | `manager` | [`manager`]             | (host any subset of the above in one process)   |

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "shell")]
pub mod shell;

#[cfg(feature = "file")]
pub mod file;

#[cfg(feature = "browser")]
pub mod browser;

#[cfg(feature = "email")]
pub mod email;

#[cfg(feature = "mcp")]
pub mod mcp;

#[cfg(feature = "slack")]
pub mod slack;

#[cfg(feature = "model")]
pub mod model;

#[cfg(feature = "manager")]
pub mod manager;
