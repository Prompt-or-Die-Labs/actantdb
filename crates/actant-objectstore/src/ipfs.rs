//! IPFS [`BlobStore`] talking to Kubo's HTTP API.
//!
//! Compiled in only when the `ipfs` feature is enabled. Posts to
//! `<base_url>/api/v0/add` to write and fetches from `<base_url>/api/v0/cat`
//! to read. Returns `ipfs://<cid>` URIs.
//!
//! This backend treats the supplied `key` as a logical label only — IPFS
//! addresses by content, so the returned URI's CID is what subsequent reads
//! must use. The label is preserved as the `name` field in the Kubo `add`
//! request for human-readable listings.

use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use serde::Deserialize;
use url::Url;

use crate::{sha256_hex, BlobError, BlobRef, BlobResult, BlobStore};

/// Configuration for [`IpfsStore`].
#[derive(Debug, Clone)]
pub struct IpfsConfig {
    /// Kubo HTTP API base URL. Default: `http://localhost:5001`.
    pub base_url: String,
    /// Optional public gateway base for presign-style URL output (e.g.
    /// `https://ipfs.io/ipfs/`). When set, [`IpfsStore::presign_get`] returns
    /// a gateway URL.
    pub gateway: Option<String>,
}

impl Default for IpfsConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:5001".into(),
            gateway: None,
        }
    }
}

/// Blob store backed by a Kubo IPFS node.
#[derive(Debug, Clone)]
pub struct IpfsStore {
    config: IpfsConfig,
    client: reqwest::Client,
}

impl IpfsStore {
    /// Construct an [`IpfsStore`] with the supplied configuration.
    pub fn new(config: IpfsConfig) -> BlobResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| BlobError::Backend(format!("reqwest client: {e}")))?;
        Ok(Self { config, client })
    }

    /// Construct against the default `http://localhost:5001` Kubo endpoint.
    pub fn local() -> BlobResult<Self> {
        Self::new(IpfsConfig::default())
    }

    fn cid_from(&self, key_or_uri: &str) -> BlobResult<String> {
        if let Some(rest) = key_or_uri.strip_prefix("ipfs://") {
            // Trim a possible trailing slash or path.
            let cid = rest.split('/').next().unwrap_or("").to_string();
            if cid.is_empty() {
                return Err(BlobError::InvalidKey(format!(
                    "URI {key_or_uri:?} has no CID"
                )));
            }
            Ok(cid)
        } else {
            // Assume it's a bare CID.
            Ok(key_or_uri.to_string())
        }
    }
}

#[derive(Debug, Deserialize)]
struct AddResponse {
    #[serde(rename = "Hash")]
    hash: String,
}

#[async_trait]
impl BlobStore for IpfsStore {
    async fn put(&self, key: &str, body: Bytes) -> BlobResult<BlobRef> {
        let size = body.len() as u64;
        let content_hash = sha256_hex(&body);
        let base = self.config.base_url.trim_end_matches('/');
        let url = format!("{base}/api/v0/add");
        let part = reqwest::multipart::Part::bytes(body.to_vec()).file_name(key.to_string());
        let form = reqwest::multipart::Form::new().part("file", part);
        let res = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| BlobError::Backend(format!("kubo add (is the daemon running?): {e}")))?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(BlobError::Backend(format!(
                "kubo add returned {status}: {body}"
            )));
        }
        // Kubo streams one JSON object per added file; we send a single file.
        let body = res
            .text()
            .await
            .map_err(|e| BlobError::Backend(e.to_string()))?;
        // Take the first line as the response object.
        let first = body.lines().next().unwrap_or("");
        let parsed: AddResponse = serde_json::from_str(first)
            .map_err(|e| BlobError::Backend(format!("kubo response parse: {e} (body={body})")))?;
        let hash = parsed.hash;
        Ok(BlobRef {
            uri: format!("ipfs://{hash}"),
            size,
            content_hash,
        })
    }

    async fn get(&self, key_or_uri: &str) -> BlobResult<Bytes> {
        let cid = self.cid_from(key_or_uri)?;
        let base = self.config.base_url.trim_end_matches('/');
        let url = format!("{base}/api/v0/cat?arg={cid}");
        let res = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| BlobError::Backend(format!("kubo cat: {e}")))?;
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BlobError::NotFound(cid));
        }
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(BlobError::Backend(format!(
                "kubo cat returned {status}: {body}"
            )));
        }
        let bytes = res
            .bytes()
            .await
            .map_err(|e| BlobError::Backend(e.to_string()))?;
        Ok(bytes)
    }

    async fn delete(&self, key_or_uri: &str) -> BlobResult<()> {
        // IPFS does not "delete" content-addressed data; unpinning is the
        // closest analogue. We unpin the CID and ignore "not pinned" errors.
        let cid = self.cid_from(key_or_uri)?;
        let base = self.config.base_url.trim_end_matches('/');
        let url = format!("{base}/api/v0/pin/rm?arg={cid}");
        let _ = self.client.post(&url).send().await;
        Ok(())
    }

    async fn exists(&self, key_or_uri: &str) -> BlobResult<bool> {
        let cid = self.cid_from(key_or_uri)?;
        let base = self.config.base_url.trim_end_matches('/');
        let url = format!("{base}/api/v0/block/stat?arg={cid}");
        let res = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| BlobError::Backend(format!("kubo block/stat: {e}")))?;
        Ok(res.status().is_success())
    }

    async fn presign_get(&self, key_or_uri: &str, _ttl: Duration) -> BlobResult<Option<Url>> {
        let cid = self.cid_from(key_or_uri)?;
        if let Some(gw) = &self.config.gateway {
            let base = gw.trim_end_matches('/');
            let raw = format!("{base}/{cid}");
            return Url::parse(&raw)
                .map(Some)
                .map_err(|e| BlobError::Backend(format!("gateway URL: {e}")));
        }
        Ok(None)
    }
}
