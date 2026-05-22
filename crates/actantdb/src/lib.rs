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
//! - `core`, `storage`, `policy`, `command`, `replay`, `subscribe`, `auth`
//! - `reliability` (throttle / circuit / lock / ingress, each its own sub-feature inside)
//! - `workers` (shell / file / browser / email / mcp / slack / model, gated inside)
//! - `objectstore` (filesystem default; s3 / ipfs / gcs / azure feature-gated)
//! - `sync`, `memory`, `effects`, `trigger`, `eval`, `embed`,
//!   `templates`, `audit-export`, `tenant`, `drift`, `compensation`,
//!   `contracts`
//!
//! This crate is *only* re-exports. Every type you see here is identical
//! to the one in the underlying `actant-*` crate; consumers who already
//! depend on those crates directly do not need to migrate.
//!
//! ## Consolidated import paths
//!
//! The retired `capsule`, `kernel`, `runtime`, and `trust` umbrella modules
//! were folded into their owning crates. Use these replacements:
//!
//! ```ignore
//! use actantdb::policy::{ActantCapsule, ActantTrustProfile, MemoryAllowed};
//! use actantdb::command::{
//!     dispatch_tool_call, ActantCache, ActantHotToolCall, ActantModelRegistry,
//!     ActantPromptTemplate,
//! };
//! use actantdb::core::{
//!     new_span_id, new_trace_id, ActantA2aCard, ActantAp2Mandate, ActantMcpServer,
//! };
//! use actantdb::memory::{ActantIndex, ActantSearchOptions};
//! use actantdb::contracts::sdk_codegen;
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "audit-export")]
pub use actant_audit_export as audit_export;
#[cfg(feature = "auth")]
pub use actant_auth as auth;
#[cfg(feature = "command")]
pub use actant_command as command;
#[cfg(feature = "compensation")]
pub use actant_compensation as compensation;
#[cfg(feature = "contracts")]
pub use actant_contracts as contracts;
#[cfg(feature = "core")]
pub use actant_core as core;
#[cfg(feature = "drift")]
pub use actant_drift as drift;
#[cfg(feature = "effects")]
pub use actant_effects as effects;
#[cfg(feature = "embed")]
pub use actant_embed as embed;
#[cfg(feature = "eval")]
pub use actant_eval as eval;
#[cfg(feature = "memory")]
pub use actant_memory as memory;
#[cfg(feature = "objectstore")]
pub use actant_objectstore as objectstore;
#[cfg(feature = "policy")]
pub use actant_policy as policy;
#[cfg(feature = "reliability")]
pub use actant_reliability as reliability;
#[cfg(feature = "replay")]
pub use actant_replay as replay;
#[cfg(feature = "storage")]
pub use actant_storage as storage;
#[cfg(feature = "subscribe")]
pub use actant_subscribe as subscribe;
#[cfg(feature = "sync")]
pub use actant_sync as sync;
#[cfg(feature = "templates")]
pub use actant_templates as templates;
#[cfg(feature = "tenant")]
pub use actant_tenant as tenant;
#[cfg(feature = "trigger")]
pub use actant_trigger as trigger;
#[cfg(feature = "workers")]
pub use actant_workers as workers;
