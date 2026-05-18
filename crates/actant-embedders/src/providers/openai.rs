//! Mockable OpenAI embeddings adapter.
//!
//! Gated behind the `openai` feature. Calls `POST {base_url}/v1/embeddings`
//! with the OpenAI request shape and parses the standard response. The base
//! URL is constructor-injected so tests can point at a local mock HTTP
//! server (see `tests/openai_mocked.rs`).

use actant_embed::{Embedder, Embedding};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// OpenAI-compatible embeddings client.
#[derive(Debug, Clone)]
pub struct OpenAiEmbedder {
    base_url: String,
    api_key: String,
    model: String,
    dim: usize,
    http: reqwest::Client,
}

impl OpenAiEmbedder {
    /// New client. `base_url` is the OpenAI host (no trailing slash). For
    /// real OpenAI pass `"https://api.openai.com"`.
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        dim: usize,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
            dim,
            http: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Serialize)]
struct EmbedRequest<'a> {
    input: &'a str,
    model: &'a str,
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedRow>,
}

#[derive(Debug, Deserialize)]
struct EmbedRow {
    embedding: Vec<f32>,
}

#[async_trait]
impl Embedder for OpenAiEmbedder {
    fn provider(&self) -> &'static str {
        "openai"
    }

    fn dimension(&self) -> usize {
        self.dim
    }

    async fn embed(&self, text: &str) -> anyhow::Result<Embedding> {
        let url = format!("{}/v1/embeddings", self.base_url);
        let req = EmbedRequest {
            input: text,
            model: &self.model,
        };
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("openai embeddings non-2xx: {status} {body}");
        }
        let mut parsed: EmbedResponse = resp.json().await?;
        let row = parsed
            .data
            .pop()
            .ok_or_else(|| anyhow::anyhow!("openai response had no data rows"))?;
        Ok(Embedding {
            provider: "openai".into(),
            model: self.model.clone(),
            vector: row.embedding,
        })
    }
}
