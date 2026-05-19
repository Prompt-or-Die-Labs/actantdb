//! actant-core — shared types for the ActantDB v2 substrate.
//!
//! Defines IDs, actors, events, errors, and schema-shared structs used by
//! the other v2 crates (`actant-storage`, `actant-command`, `actant-policy`,
//! `actant-server`, ...). Mirrors the SQL schema in `/specs/02-data-model.sql`.
//!
//! See `/specs/01-architecture.md` for the layering and
//! `/specs/13-actant-contract.md` for the underlying type contracts.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod hash;
pub mod hlc;
pub mod ids;
pub mod model;
pub mod time_utils;

pub use error::ActantError;
pub use hash::{canonical_json, chain_hash, sha256_hex};
pub use hlc::{Hlc, HlcClock};
pub use ids::*;
pub use model::*;
pub use time_utils::now_rfc3339;
