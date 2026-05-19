//! Filesystem-backed [`BlobStore`]. The default backend; no external deps.
//!
//! Objects are laid out as `<root>/<aa>/<key>` where `aa` is the first two
//! characters of the key. This avoids the "millions of files in one directory"
//! issue on common filesystems (ext4, APFS).
//!
//! Canonical URI is `file://<absolute-path>`.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use bytes::Bytes;
use tokio::fs;
use tokio::io::AsyncReadExt;
use url::Url;

use crate::{is_safe_key, sha256_hex, BlobError, BlobRef, BlobResult, BlobStore};

/// Filesystem-backed object store. Cheap to clone (just paths).
#[derive(Debug, Clone)]
pub struct FilesystemStore {
    root: PathBuf,
}

impl FilesystemStore {
    /// Open a filesystem store rooted at `root`. Creates the directory if
    /// missing.
    pub fn new(root: impl AsRef<Path>) -> std::io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        Ok(Self {
            root: root.canonicalize()?,
        })
    }

    /// Root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Map a key to the on-disk path. Validates the key first.
    fn path_for(&self, key: &str) -> BlobResult<PathBuf> {
        if !is_safe_key(key) {
            return Err(BlobError::InvalidKey(format!(
                "key {key:?} contains characters outside [A-Za-z0-9_-.]"
            )));
        }
        let prefix = if key.len() >= 2 { &key[..2] } else { "__" };
        Ok(self.root.join(prefix).join(key))
    }

    /// Extract a key from a `file://…` URI rooted under `self.root`, or pass
    /// a bare key through unchanged.
    fn key_from(&self, key_or_uri: &str) -> BlobResult<String> {
        if let Some(rest) = key_or_uri.strip_prefix("file://") {
            // Path is absolute; we only care about the basename, which is the key.
            let p = Path::new(rest);
            let name = p.file_name().and_then(|s| s.to_str()).ok_or_else(|| {
                BlobError::InvalidKey(format!("URI {key_or_uri:?} has no basename"))
            })?;
            Ok(name.to_string())
        } else {
            Ok(key_or_uri.to_string())
        }
    }

    fn uri_for(&self, path: &Path) -> String {
        // Url::from_file_path requires absolute paths; `path_for` produced an
        // absolute path because `root` was canonicalized.
        match Url::from_file_path(path) {
            Ok(u) => u.to_string(),
            // Fall back to a manually-built file:// scheme. This path is
            // exercised by tests on platforms where canonicalize tweaks the
            // path representation; the URI is purely informational.
            Err(_) => format!("file://{}", path.display()),
        }
    }
}

#[async_trait]
impl BlobStore for FilesystemStore {
    async fn put(&self, key: &str, body: Bytes) -> BlobResult<BlobRef> {
        let path = self.path_for(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let size = body.len() as u64;
        let content_hash = sha256_hex(&body);
        // Atomic write: write to a `.tmp` sibling then rename. Avoids torn
        // reads if a process is also calling `get` while `put` is in flight.
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, &body[..]).await?;
        fs::rename(&tmp, &path).await?;
        Ok(BlobRef {
            uri: self.uri_for(&path),
            size,
            content_hash,
        })
    }

    async fn get(&self, key_or_uri: &str) -> BlobResult<Bytes> {
        let key = self.key_from(key_or_uri)?;
        let path = self.path_for(&key)?;
        let mut file = fs::File::open(&path).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => BlobError::NotFound(format!("{key}: {e}")),
            _ => BlobError::Io(e.to_string()),
        })?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await?;
        Ok(Bytes::from(buf))
    }

    async fn delete(&self, key_or_uri: &str) -> BlobResult<()> {
        let key = self.key_from(key_or_uri)?;
        let path = self.path_for(&key)?;
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(BlobError::Io(e.to_string())),
        }
    }

    async fn exists(&self, key_or_uri: &str) -> BlobResult<bool> {
        let key = self.key_from(key_or_uri)?;
        let path = self.path_for(&key)?;
        match fs::metadata(&path).await {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(BlobError::Io(e.to_string())),
        }
    }

    async fn presign_get(
        &self,
        _key_or_uri: &str,
        _ttl: std::time::Duration,
    ) -> BlobResult<Option<Url>> {
        // Filesystem URIs are not presignable; the caller has direct access.
        Ok(None)
    }
}
