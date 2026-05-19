//! # actantdb
//!
//! Single-crate entry point for every ActantDB primitive. Pick everything
//! with one `cargo add`, or feature-prune to just what your app needs.
//!
//! ## All-in (most common)
//!
//! ```toml
//! [dependencies]
//! actantdb = "0.0"
//! ```
//!
//! ```ignore
//! use actantdb::storage::Storage;
//! use actantdb::policy::evaluate;
//! ```
//!
//! ## Just storage
//!
//! ```toml
//! [dependencies]
//! actantdb = { version = "0.0", default-features = false, features = ["storage"] }
//! ```
//!
//! ```ignore
//! use actantdb::storage::{Storage, StorageConfig};
//! ```
//!
//! ## Mix and match
//!
//! Pick any subset of the features below:
//!
//! - `storage`, `policy`, `command`, `replay`, `subscribe`, `auth`
//! - `reliability` (throttle / circuit / lock / ingress, each its own sub-feature inside)
//! - `runtime` (trace / cache / prompts / models / protocol)
//! - `workers` (shell / file / browser / email / mcp / slack / model, gated inside)
//! - `objectstore` (filesystem default; s3 / ipfs / gcs / azure feature-gated)
//! - `sync`, `memory`, `effects`, `trigger`, `eval`, `embed`, `capsule`,
//!   `trust`, `templates`, `audit-export`, `tenant`, `drift`,
//!   `compensation`, `kernel`, `contracts`
//!
//! This crate is *only* re-exports. Every type you see here is identical
//! to the one in the underlying `actant-*` crate; consumers who already
//! depend on those crates directly do not need to migrate.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "storage")]      pub use actant_storage      as storage;
#[cfg(feature = "policy")]       pub use actant_policy       as policy;
#[cfg(feature = "command")]      pub use actant_command      as command;
#[cfg(feature = "replay")]       pub use actant_replay       as replay;
#[cfg(feature = "subscribe")]    pub use actant_subscribe    as subscribe;
#[cfg(feature = "auth")]         pub use actant_auth         as auth;
#[cfg(feature = "reliability")]  pub use actant_reliability  as reliability;
#[cfg(feature = "runtime")]      pub use actant_runtime      as runtime;
#[cfg(feature = "objectstore")]  pub use actant_objectstore  as objectstore;
#[cfg(feature = "sync")]         pub use actant_sync         as sync;
#[cfg(feature = "workers")]      pub use actant_workers      as workers;
#[cfg(feature = "memory")]       pub use actant_memory       as memory;
#[cfg(feature = "effects")]      pub use actant_effects      as effects;
#[cfg(feature = "trigger")]      pub use actant_trigger      as trigger;
#[cfg(feature = "eval")]         pub use actant_eval         as eval;
#[cfg(feature = "embed")]        pub use actant_embed        as embed;
#[cfg(feature = "capsule")]      pub use actant_capsule      as capsule;
#[cfg(feature = "trust")]        pub use actant_trust        as trust;
#[cfg(feature = "templates")]    pub use actant_templates    as templates;
#[cfg(feature = "audit-export")] pub use actant_audit_export as audit_export;
#[cfg(feature = "tenant")]       pub use actant_tenant       as tenant;
#[cfg(feature = "drift")]        pub use actant_drift        as drift;
#[cfg(feature = "compensation")] pub use actant_compensation as compensation;
#[cfg(feature = "kernel")]       pub use actant_kernel       as kernel;
#[cfg(feature = "contracts")]    pub use actant_contracts    as contracts;
