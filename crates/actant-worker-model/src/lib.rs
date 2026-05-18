//! Model-call worker. Adapts OpenAI-compatible HTTP providers and a mock
//! provider used by tests and the alpha-demo without keys.
//!
//! Phase 1 keeps the surface small: one `model.call` effect type with an
//! `input` schema of `{ prompt, model, route, role }`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::ActantError;
use actant_worker_protocol::{Handler, HandlerResult};
use async_trait::async_trait;

/// Provider for the model worker.
#[derive(Debug, Clone)]
pub enum Provider {
    /// Deterministic mock — echoes the prompt back as a "plan".
    Mock,
    /// OpenAI-compatible chat completion API.
    OpenAi {
        /// Base URL like `https://api.openai.com/v1`.
        base_url: String,
        /// Bearer token.
        api_key: String,
    },
    /// Local Ollama server. Ollama exposes an OpenAI-compatible chat-completions
    /// endpoint at `<base_url>/v1/chat/completions` and does not require an
    /// API key.
    Ollama {
        /// Base URL like `http://localhost:11434`.
        base_url: String,
    },
}

impl Provider {
    /// Constructor for [`Provider::Ollama`] pointing at the default
    /// `http://localhost:11434` endpoint.
    pub fn ollama() -> Self {
        Self::Ollama {
            base_url: "http://localhost:11434".to_string(),
        }
    }
}

/// Per-1k token rate table used by [`compute_cost_usd`].
#[derive(Debug, Clone, Copy)]
pub struct CostRates {
    /// Dollars per 1,000 input tokens.
    pub input_per_1k: f64,
    /// Dollars per 1,000 output tokens.
    pub output_per_1k: f64,
}

/// Linear cost formula: `(tokens_in/1000)*input_per_1k + (tokens_out/1000)*output_per_1k`.
///
/// Pure function over the inputs — no provider call, no allocation. Sealed
/// here so every worker that wants to charge a model_call uses the same math.
pub fn compute_cost_usd(tokens_in: u32, tokens_out: u32, rates: CostRates) -> f64 {
    let cin = (tokens_in as f64 / 1000.0) * rates.input_per_1k;
    let cout = (tokens_out as f64 / 1000.0) * rates.output_per_1k;
    cin + cout
}

/// Handler for `model.call` effects.
#[derive(Debug, Clone)]
pub struct ModelHandler {
    /// Active provider.
    pub provider: Provider,
}

impl ModelHandler {
    /// New mock handler.
    pub fn mock() -> Self {
        Self {
            provider: Provider::Mock,
        }
    }
}

#[async_trait]
impl Handler for ModelHandler {
    fn effect_type(&self) -> &'static str {
        "model.call"
    }

    async fn handle(&self, input: serde_json::Value) -> HandlerResult {
        let prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let model = input
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("mock")
            .to_string();
        match &self.provider {
            Provider::Mock => Ok(serde_json::json!({
                "model": model,
                "tokens_in": prompt.len() as u32 / 4,
                "tokens_out": (prompt.len() as u32 / 8).max(4),
                "summary": format!("[mock] received: {}", &prompt[..prompt.len().min(80)]),
                "raw": prompt,
            })),
            Provider::OpenAi { base_url, api_key } => {
                let client = reqwest::Client::new();
                let resp = client
                    .post(format!("{base_url}/chat/completions"))
                    .bearer_auth(api_key)
                    .json(&serde_json::json!({
                        "model": model,
                        "messages": [{"role":"user","content": prompt}],
                    }))
                    .send()
                    .await
                    .map_err(|e| ActantError::Internal(format!("openai: {e}")))?;
                let body: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| ActantError::Internal(format!("openai parse: {e}")))?;
                Ok(body)
            }
            Provider::Ollama { base_url } => {
                let client = reqwest::Client::new();
                let resp = client
                    .post(format!("{base_url}/v1/chat/completions"))
                    .json(&serde_json::json!({
                        "model": model,
                        "messages": [{"role":"user","content": prompt}],
                    }))
                    .send()
                    .await
                    .map_err(|e| ActantError::Internal(format!("ollama: {e}")))?;
                let body: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| ActantError::Internal(format!("ollama parse: {e}")))?;
                Ok(body)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_responds() {
        let h = ModelHandler::mock();
        let r = h
            .handle(serde_json::json!({"prompt":"hi","model":"mock"}))
            .await
            .unwrap();
        assert_eq!(r["model"], "mock");
    }
}
