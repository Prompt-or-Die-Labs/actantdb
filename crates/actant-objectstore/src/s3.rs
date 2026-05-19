//! S3 [`BlobStore`] backed by `apache/arrow-rs`'s `object_store` crate.
//!
//! Compiled in only when the `s3` feature is enabled. The backend talks to
//! AWS S3 and any S3-compatible endpoint (MinIO, Cloudflare R2,
//! DigitalOcean Spaces) — the underlying crate handles signing and retries.
//!
//! Canonical URI: `s3://<bucket>/<key>`.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::path::Path as ObjPath;
use object_store::signer::Signer;
use object_store::ObjectStore;
use url::Url;

use crate::{sha256_hex, BlobError, BlobRef, BlobResult, BlobStore};

/// Configuration for [`S3Store`]. Fields map onto `AmazonS3Builder`.
#[derive(Debug, Clone)]
pub struct S3Config {
    /// Bucket name.
    pub bucket: String,
    /// AWS region (e.g. `us-east-1`).
    pub region: Option<String>,
    /// Override endpoint for S3-compatible providers (`https://minio:9000`,
    /// `https://<account>.r2.cloudflarestorage.com`, etc).
    pub endpoint: Option<String>,
    /// Access key id. Falls back to the standard AWS env vars when `None`.
    pub access_key_id: Option<String>,
    /// Secret access key.
    pub secret_access_key: Option<String>,
    /// Allow plain HTTP (set for local MinIO).
    pub allow_http: bool,
}

impl S3Config {
    /// New config for `bucket` with defaults inherited from AWS env vars.
    pub fn new(bucket: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            region: None,
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            allow_http: false,
        }
    }
}

/// S3 / S3-compatible blob store.
///
/// Holds the concrete `AmazonS3` handle so [`Signer::signed_url`] is callable
/// for presign — `dyn ObjectStore` does not require `Signer`.
#[derive(Debug, Clone)]
pub struct S3Store {
    bucket: String,
    inner: Arc<AmazonS3>,
}

impl S3Store {
    /// Construct an [`S3Store`] from a [`S3Config`].
    pub fn from_config(config: S3Config) -> BlobResult<Self> {
        let mut b = AmazonS3Builder::new()
            .with_bucket_name(&config.bucket)
            .with_allow_http(config.allow_http);
        if let Some(r) = &config.region {
            b = b.with_region(r);
        }
        if let Some(e) = &config.endpoint {
            b = b.with_endpoint(e);
        }
        if let Some(k) = &config.access_key_id {
            b = b.with_access_key_id(k);
        }
        if let Some(k) = &config.secret_access_key {
            b = b.with_secret_access_key(k);
        }
        let inner = b
            .build()
            .map_err(|e| BlobError::Backend(format!("AmazonS3Builder: {e}")))?;
        Ok(Self {
            bucket: config.bucket,
            inner: Arc::new(inner),
        })
    }

    fn uri_for(&self, key: &str) -> String {
        format!("s3://{}/{}", self.bucket, key)
    }

    fn key_from<'a>(&self, key_or_uri: &'a str) -> BlobResult<&'a str> {
        if let Some(rest) = key_or_uri.strip_prefix("s3://") {
            let bucket = &self.bucket;
            let prefix = format!("{bucket}/");
            if let Some(k) = rest.strip_prefix(&prefix) {
                Ok(k)
            } else {
                Err(BlobError::InvalidKey(format!(
                    "URI {key_or_uri:?} is for a different bucket (this store is {bucket:?})"
                )))
            }
        } else {
            Ok(key_or_uri)
        }
    }
}

fn to_blob_err(e: object_store::Error) -> BlobError {
    match &e {
        object_store::Error::NotFound { path, .. } => BlobError::NotFound(path.clone()),
        _ => BlobError::Backend(e.to_string()),
    }
}

#[async_trait]
impl BlobStore for S3Store {
    async fn put(&self, key: &str, body: Bytes) -> BlobResult<BlobRef> {
        let size = body.len() as u64;
        let content_hash = sha256_hex(&body);
        let path = ObjPath::from(key);
        self.inner
            .put(&path, body.into())
            .await
            .map_err(to_blob_err)?;
        Ok(BlobRef {
            uri: self.uri_for(key),
            size,
            content_hash,
        })
    }

    async fn get(&self, key_or_uri: &str) -> BlobResult<Bytes> {
        let key = self.key_from(key_or_uri)?;
        let path = ObjPath::from(key);
        let got = self.inner.get(&path).await.map_err(to_blob_err)?;
        let bytes = got.bytes().await.map_err(to_blob_err)?;
        Ok(bytes)
    }

    async fn delete(&self, key_or_uri: &str) -> BlobResult<()> {
        let key = self.key_from(key_or_uri)?;
        let path = ObjPath::from(key);
        match self.inner.delete(&path).await {
            Ok(()) => Ok(()),
            Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(BlobError::Backend(e.to_string())),
        }
    }

    async fn exists(&self, key_or_uri: &str) -> BlobResult<bool> {
        let key = self.key_from(key_or_uri)?;
        let path = ObjPath::from(key);
        match self.inner.head(&path).await {
            Ok(_) => Ok(true),
            Err(object_store::Error::NotFound { .. }) => Ok(false),
            Err(e) => Err(BlobError::Backend(e.to_string())),
        }
    }

    async fn presign_get(&self, key_or_uri: &str, ttl: Duration) -> BlobResult<Option<Url>> {
        let key = self.key_from(key_or_uri)?;
        let path = ObjPath::from(key);
        // `Signer::signed_url` is the public presign entry point in
        // object_store 0.11. `AmazonS3` implements `Signer` directly.
        let url = Signer::signed_url(self.inner.as_ref(), reqwest::Method::GET, &path, ttl)
            .await
            .map_err(|e| BlobError::Backend(e.to_string()))?;
        Ok(Some(url))
    }
}
