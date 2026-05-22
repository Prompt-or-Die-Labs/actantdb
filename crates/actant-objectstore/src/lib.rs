//! actant-objectstore — pluggable blob/object storage for ActantDB.
//!
//! Today the substrate's `artifact.uri` column is a free-form string and the
//! single in-tree writer (`write_report_event_and_artifact` in `actant-server`)
//! synthesises an `actantdb://event/<id>` back-reference rather than storing
//! anything outside SQLite (see `STORAGE_AUDIT.md §3`). This crate ships the
//! abstraction the audit calls for: a [`BlobStore`] trait with a default
//! filesystem implementation, an opt-in S3 backend through `apache/arrow-rs`'s
//! `object_store`, an opt-in IPFS backend talking to Kubo's HTTP API, an
//! in-memory store for tests, and a [`Layered`] router that dispatches on URI
//! scheme.
//!
//! The default-feature build pulls only `tokio`, `bytes`, and `url` —
//! no AWS SDK, no IPFS client. Backends are gated behind the `s3` and `ipfs`
//! features.
//!
//! # Quick start
//!
//! ```no_run
//! use actant_objectstore::{BlobStore, FilesystemStore};
//! use bytes::Bytes;
//! # async fn ex() -> anyhow::Result<()> {
//! let store = FilesystemStore::new("/tmp/actantdb-blobs")?;
//! let r = store.put("hello.txt", Bytes::from_static(b"hi")).await?;
//! assert!(r.uri.starts_with("file://"));
//! let body = store.get(&r.uri).await?;
//! assert_eq!(&body[..], b"hi");
//! # Ok(()) }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use thiserror::Error;
use url::Url;

mod fs;
mod layered;
mod mem;

#[cfg(feature = "s3")]
mod s3;

#[cfg(feature = "ipfs")]
mod ipfs;

pub use fs::FilesystemStore;
pub use layered::Layered;
pub use mem::MemoryStore;

#[cfg(feature = "s3")]
pub use s3::{S3Config, S3Store};

#[cfg(feature = "ipfs")]
pub use ipfs::{IpfsConfig, IpfsStore};

/// Errors surfaced by [`BlobStore`] implementations.
///
/// Implementations may add provider-specific context inside [`BlobError::Backend`].
/// Callers should treat this as opaque other than the discriminants below.
#[derive(Debug, Error)]
pub enum BlobError {
    /// Object key (or URI) failed validation — empty, contained path traversal
    /// segments, or otherwise unsafe.
    #[error("invalid key: {0}")]
    InvalidKey(String),

    /// Object did not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// Underlying IO error (filesystem, network).
    #[error("io error: {0}")]
    Io(String),

    /// Backend rejected the request (HTTP error, S3 error, etc).
    #[error("backend error: {0}")]
    Backend(String),

    /// Feature was recognised but is not compiled in (e.g. `s3` feature off).
    #[error("backend not available: {0}")]
    Unavailable(String),
}

impl From<std::io::Error> for BlobError {
    fn from(value: std::io::Error) -> Self {
        match value.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound(value.to_string()),
            _ => Self::Io(value.to_string()),
        }
    }
}

/// Convenience result alias.
pub type BlobResult<T> = Result<T, BlobError>;

/// Reference to a stored object — the canonical URI plus integrity metadata.
///
/// Callers should treat [`BlobRef::uri`] as the opaque locator: it can be
/// passed back to [`BlobStore::get`] / [`BlobStore::delete`] without
/// understanding the scheme. The same URI is what gets persisted into
/// `artifact.uri`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRef {
    /// Canonical URI for the stored object (e.g. `file:///…`, `s3://bucket/key`,
    /// `ipfs://<cid>`, `mem://<store>/<key>`).
    pub uri: String,
    /// Size of the stored payload in bytes.
    pub size: u64,
    /// Hex-encoded SHA-256 of the stored payload.
    pub content_hash: String,
}

/// Trait implemented by every blob backend.
///
/// All methods are async and `Send` + `Sync` so the implementation can sit
/// inside `Arc` and be shared across the storage layer.
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// Store `body` under `key`. Returns a [`BlobRef`] whose `uri` is the
    /// canonical locator for the object.
    ///
    /// Keys are validated by the implementation; see [`is_safe_key`] for the
    /// rules the filesystem backend enforces.
    async fn put(&self, key: &str, body: Bytes) -> BlobResult<BlobRef>;

    /// Retrieve the object identified by `key_or_uri`. Implementations accept
    /// either the bare key that was passed to [`Self::put`] or the canonical
    /// URI returned by it.
    async fn get(&self, key_or_uri: &str) -> BlobResult<Bytes>;

    /// Delete the object. Idempotent — deleting an object that does not exist
    /// returns `Ok(())`.
    async fn delete(&self, key_or_uri: &str) -> BlobResult<()>;

    /// Whether the object exists.
    async fn exists(&self, key_or_uri: &str) -> BlobResult<bool>;

    /// Generate a presigned GET URL valid for `ttl`, if the backend supports
    /// it. Returns `Ok(None)` when presigning is not available (filesystem,
    /// IPFS — IPFS gateways are public by default, so the URI itself is the
    /// reachable URL).
    async fn presign_get(&self, key_or_uri: &str, ttl: Duration) -> BlobResult<Option<Url>>;
}

/// Validate that a key is safe to map onto a filesystem path. The filesystem
/// backend enforces this; in-memory and S3 backends accept the broader key
/// space their host systems permit.
///
/// Allowed characters: `[A-Za-z0-9_\-.]`. Disallows empty keys, `..`, leading
/// `/`, leading `.`, and embedded path separators. This is intentionally
/// restrictive: the canonical artifact key is a hex sha256, which always
/// satisfies the rule, and keeping the surface small forecloses path-traversal
/// CVEs.
pub fn is_safe_key(key: &str) -> bool {
    if key.is_empty() || key == "." || key == ".." || key.starts_with('.') {
        return false;
    }
    if key.contains("..") {
        return false;
    }
    key.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Compute the SHA-256 of `body` and return it as a lowercase hex string.
/// Exposed as a helper so callers (notably `actant-storage::Storage::put_artifact`)
/// don't pull `sha2` directly.
pub fn sha256_hex(body: &[u8]) -> String {
    actant_core::sha256_hex(body)
}
